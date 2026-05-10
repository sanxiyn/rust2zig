mod expr;
mod item;
mod mac;
mod pat;
mod stmt;
mod ty;

use std::collections::HashMap;
use std::fmt::Write;

use crate::scip::{Kind, Range, Scip};

const INDENT_SIZE: usize = 4;

pub fn camel_to_snake(s: &str) -> String {
    let mut result: String = Default::default();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

pub fn snake_to_camel(s: &str) -> String {
    let mut result: String = Default::default();
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

pub enum PathMode {
    Normal,
    EnumVariant,
}

pub struct Enum {
    pub has_data: bool,
    pub is_generic: bool,
    pub impls: Vec<syn::ItemImpl>,
}

pub struct Struct {
    pub impls: Vec<syn::ItemImpl>,
}

pub struct Rust2Zig {
    pub enums: HashMap<String, Enum>,
    pub structs: HashMap<String, Struct>,
    pub scip: Scip,
    out: String,
    indent: usize,
}

impl Rust2Zig {
    pub fn new(scip: Scip) -> Self {
        Rust2Zig {
            enums: Default::default(),
            structs: Default::default(),
            scip,
            out: Default::default(),
            indent: Default::default(),
        }
    }

    pub fn check_moniker(&self, path: &syn::Path, expected: &str) -> bool {
        let ident = &path.segments.last().unwrap().ident;
        let range = ident.span().into();
        let Some(symbol) = self.scip.symbol_at(&range) else { return false };
        let suffix = match expected {
            "core::macros::assert_eq" => "macros/assert_eq!",
            "core::option::Option" => "option/Option#",
            "core::option::Option::Some" => "option/Option#Some#",
            "core::option::Option::None" => "option/Option#None#",
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
                    let name = e.ident.to_string();
                    let has_data = e.variants.iter().any(|v| !v.fields.is_empty());
                    let is_generic = !e.generics.params.is_empty();
                    self.enums.insert(name, Enum { has_data, is_generic, impls: Default::default() });
                }
                syn::Item::Struct(s) => {
                    let name = s.ident.to_string();
                    self.structs.insert(name, Struct { impls: Default::default() });
                }
                _ => {}
            }
        }

        for item in &file.items {
            if let syn::Item::Impl(i) = item {
                if let syn::Type::Path(tp) = &*i.self_ty {
                    let name = tp.path.segments.last().unwrap().ident.to_string();
                    if let Some(e) = self.enums.get_mut(&name) {
                        e.impls.push(i.clone());
                    } else if let Some(s) = self.structs.get_mut(&name) {
                        s.impls.push(i.clone());
                    }
                }
            }
        }
    }

    pub fn has_capture(&self, ec: &syn::ExprClosure) -> bool {
        use syn::spanned::Spanned;
        use syn::visit::Visit;

        struct Visitor<'a> {
            scip: &'a Scip,
            span: Range,
            found: bool,
        }

        impl<'a, 'ast> Visit<'ast> for Visitor<'a> {
            fn visit_ident(&mut self, ident: &'ast syn::Ident) {
                if self.found {
                    return;
                }
                let range: Range = ident.span().into();
                let Some(symbol) = self.scip.symbol_at(&range) else { return };
                let Some(info) = self.scip.symbol_info(symbol) else { return };
                if !matches!(info.kind, Kind::Variable | Kind::Parameter) {
                    return;
                }
                let Some(def) = info.range.as_ref() else { return };
                if !self.span.contains(def) {
                    self.found = true;
                }
            }
        }

        let span: Range = ec.span().into();
        let mut visitor = Visitor { scip: &self.scip, span, found: false };
        visitor.visit_expr(&ec.body);
        visitor.found
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
                write!(self.out, "{}", snake_to_camel(&ident.to_string())).unwrap();
            }
            PathMode::EnumVariant => {
                write!(self.out, ".{}", camel_to_snake(&ident.to_string())).unwrap();
            }
        }
    }

}
