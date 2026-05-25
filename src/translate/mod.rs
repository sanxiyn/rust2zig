use std::collections::{HashMap, HashSet};
use std::fmt::Write;

mod call;
mod expr;
mod flow;
mod generic;
mod item;
mod mac;
mod name;
mod pat;
mod rename;
mod stmt;
mod ty;

use crate::scip::{Kind, Range, Scip};
use generic::GenericFn;
use name::{camel_to_snake, snake_to_camel};

const INDENT_SIZE: usize = 4;

pub enum PathMode {
    Normal,
    EnumVariant,
}

pub struct Struct {
    pub impls: Vec<syn::ItemImpl>,
}

pub struct Enum {
    pub has_data: bool,
    pub is_generic: bool,
    pub impls: Vec<syn::ItemImpl>,
}

pub struct Rust2Zig {
    pub structs: HashMap<String, Struct>,
    pub enums: HashMap<String, Enum>,
    pub generic_fns: HashMap<String, GenericFn>,
    pub renames: HashMap<String, String>,
    pub capture_stack: Vec<HashMap<String, String>>,
    pub scip: Scip,
    out: String,
    indent: usize,
}

impl Rust2Zig {
    pub fn new(scip: Scip) -> Self {
        Rust2Zig {
            structs: Default::default(),
            enums: Default::default(),
            generic_fns: Default::default(),
            renames: Default::default(),
            capture_stack: Default::default(),
            scip,
            out: Default::default(),
            indent: Default::default(),
        }
    }

    pub fn check_moniker(&self, path: &syn::Path, expected: &str) -> bool {
        let ident = &path.segments.last().unwrap().ident;
        self.check_moniker_ident(ident, expected)
    }

    pub fn check_moniker_ident(&self, ident: &syn::Ident, expected: &str) -> bool {
        let range = ident.span().into();
        let Some(symbol) = self.scip.symbol_at(&range) else { return false };
        let suffix = match expected {
            "core::macros::assert_eq" => "macros/assert_eq!",
            "core::option::Option" => "option/Option#",
            "core::option::Option::Some" => "option/Option#Some#",
            "core::option::Option::None" => "option/Option#None#",
            "core::slice::len" => "slice/impl#[`[T]`]len().",
            "std::macros::panic" => "macros/panic!",
            "std::macros::println" => "macros/println!",
            _ => return false,
        };
        symbol.ends_with(suffix)
    }

    fn indent(&mut self) {
        self.indent += INDENT_SIZE;
    }

    fn dedent(&mut self) {
        self.indent -= INDENT_SIZE;
    }

    fn pad(&self) -> String {
        " ".repeat(self.indent)
    }

    pub fn analyze(&mut self, file: &syn::File) {
        for item in &file.items {
            match item {
                syn::Item::Enum(e) => {
                    let Some(symbol) = self.scip.symbol_at(&e.ident.span().into()) else { continue };
                    let symbol = symbol.to_string();
                    let has_data = e.variants.iter().any(|v| !v.fields.is_empty());
                    let is_generic = !e.generics.params.is_empty();
                    self.enums.insert(symbol, Enum { has_data, is_generic, impls: Default::default() });
                }
                syn::Item::Fn(f) => {
                    self.register_generic(&f.sig);
                }
                syn::Item::Struct(s) => {
                    let Some(symbol) = self.scip.symbol_at(&s.ident.span().into()) else { continue };
                    let symbol = symbol.to_string();
                    self.structs.insert(symbol, Struct { impls: Default::default() });
                }
                _ => {}
            }
        }

        for item in &file.items {
            if let syn::Item::Impl(i) = item {
                if let syn::Type::Path(tp) = &*i.self_ty {
                    let ident = &tp.path.segments.last().unwrap().ident;
                    if let Some(symbol) = self.scip.symbol_at(&ident.span().into()) {
                        if let Some(s) = self.structs.get_mut(symbol) {
                            s.impls.push(i.clone());
                        } else if let Some(e) = self.enums.get_mut(symbol) {
                            e.impls.push(i.clone());
                        }
                    }
                    for ii in &i.items {
                        if let syn::ImplItem::Fn(m) = ii {
                            self.register_generic(&m.sig);
                        }
                    }
                }
            }
        }

        self.collect_renames(file);
    }

    pub fn collect_captures(&self, ec: &syn::ExprClosure) -> Vec<(syn::Ident, syn::Type)> {
        use syn::spanned::Spanned;
        use syn::visit::Visit;

        struct Visitor<'a> {
            scip: &'a Scip,
            span: Range,
            seen: HashSet<String>,
            captures: Vec<(syn::Ident, syn::Type)>,
        }

        impl<'a, 'ast> Visit<'ast> for Visitor<'a> {
            fn visit_ident(&mut self, ident: &'ast syn::Ident) {
                let range: Range = ident.span().into();
                let Some(symbol) = self.scip.symbol_at(&range) else { return };
                let Some(info) = self.scip.symbol_info(symbol) else { return };
                if !matches!(info.kind, Kind::Variable | Kind::Parameter) {
                    return;
                }
                let Some(def) = info.range.as_ref() else { return };
                if self.span.contains(def) {
                    return;
                }
                if self.seen.insert(symbol.to_string()) {
                    if let Some(ty) = self.scip.type_at(&range) {
                        self.captures.push((ident.clone(), ty));
                    }
                }
            }
        }

        let span: Range = ec.span().into();
        let mut visitor = Visitor {
            scip: &self.scip,
            span,
            seen: HashSet::new(),
            captures: Vec::new(),
        };
        visitor.visit_expr(&ec.body);
        visitor.captures
    }

    pub fn is_closure_type(&self, ty: &syn::Type) -> bool {
        let syn::Type::ImplTrait(it) = ty else { return false };
        it.bounds.iter().any(|bound| {
            let syn::TypeParamBound::Trait(tb) = bound else { return false };
            let Some(last) = tb.path.segments.last() else { return false };
            matches!(last.ident.to_string().as_str(), "Fn" | "FnMut" | "FnOnce")
        })
    }

    pub fn closure_return_type(&self, ident: &syn::Ident) -> Option<syn::Type> {
        let ty = self.scip.type_at(&ident.span().into())?;
        let syn::Type::ImplTrait(it) = ty else { return None };
        for bound in it.bounds {
            let syn::TypeParamBound::Trait(tb) = bound else { continue };
            let Some(last) = tb.path.segments.last() else { continue };
            let syn::PathArguments::Parenthesized(p) = &last.arguments else { continue };
            if let syn::ReturnType::Type(_, t) = &p.output {
                return Some((**t).clone());
            }
        }
        None
    }

    pub fn path_mode(&self, path: &syn::Path) -> PathMode {
        let ident = &path.segments.last().unwrap().ident;
        let range = ident.span().into();
        if self.scip.kind_at(&range) == Some(Kind::EnumMember) {
            PathMode::EnumVariant
        } else {
            PathMode::Normal
        }
    }

    pub fn translate_file(&mut self, file: &syn::File) -> String {
        self.out.clear();
        writeln!(self.out, "const std = @import(\"std\");").unwrap();
        writeln!(self.out).unwrap();
        for item in &file.items {
            self.translate_item(item);
        }
        std::mem::take(&mut self.out)
    }

    pub fn translate_path(&mut self, path: &syn::Path, mode: PathMode) {
        let ident = &path.segments.last().unwrap().ident;
        match mode {
            PathMode::Normal => {
                let kind = self.scip.kind_at(&ident.span().into());
                if matches!(kind, Some(Kind::Variable) | Some(Kind::Parameter)) {
                    let name = self.rename_ident(ident);
                    if let Some(map) = self.capture_stack.last() {
                        if let Some(symbol) = self.scip.symbol_at(&ident.span().into()) {
                            if let Some(field) = map.get(symbol) {
                                write!(self.out, "self.{}", field).unwrap();
                                return;
                            }
                        }
                    }
                    write!(self.out, "{}", name).unwrap();
                } else {
                    write!(self.out, "{}", snake_to_camel(&ident.to_string())).unwrap();
                }
            }
            PathMode::EnumVariant => {
                write!(self.out, ".{}", camel_to_snake(&ident.to_string())).unwrap();
            }
        }
    }

}
