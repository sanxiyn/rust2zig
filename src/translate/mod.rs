use std::collections::{HashMap, HashSet};
use std::fmt::Write;

mod call;
mod expr;
mod flow;
mod item;
mod mac;
mod name;
mod pat;
mod stmt;
mod ty;

use crate::scip::{Kind, Range, Scip};
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

pub struct GenericArgRef {
    pub arg: usize,
    pub path: Vec<usize>,
}

pub struct GenericFn {
    pub type_params: Vec<String>,
    pub param_arg_index: Vec<GenericArgRef>,
}

pub fn find_type_param(ty: &syn::Type, name: &str) -> Option<Vec<usize>> {
    let syn::Type::Path(tp) = ty else { return None };
    if tp.path.is_ident(name) {
        return Some(Vec::new());
    }
    let last = tp.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(ab) = &last.arguments else { return None };
    for (i, ga) in ab.args.iter().enumerate() {
        let syn::GenericArgument::Type(inner) = ga else { continue };
        if let Some(mut sub) = find_type_param(inner, name) {
            let mut path = vec![i];
            path.append(&mut sub);
            return Some(path);
        }
    }
    None
}

pub fn peel_type<'a>(mut ty: &'a syn::Type, path: &[usize]) -> Option<&'a syn::Type> {
    for &idx in path {
        let syn::Type::Path(tp) = ty else { return None };
        let last = tp.path.segments.last()?;
        let syn::PathArguments::AngleBracketed(ab) = &last.arguments else { return None };
        let ga = ab.args.iter().nth(idx)?;
        let syn::GenericArgument::Type(inner) = ga else { return None };
        ty = inner;
    }
    Some(ty)
}

pub struct Rust2Zig {
    pub structs: HashMap<String, Struct>,
    pub enums: HashMap<String, Enum>,
    pub generic_fns: HashMap<String, GenericFn>,
    pub renames: HashMap<String, String>,
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
            scip,
            out: Default::default(),
            indent: Default::default(),
        }
    }

    pub fn rename_ident(&self, ident: &syn::Ident) -> String {
        let range: Range = ident.span().into();
        if let Some(symbol) = self.scip.symbol_at(&range) {
            if let Some(name) = self.renames.get(symbol) {
                return name.clone();
            }
        }
        ident.to_string()
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

    fn collect_renames(&mut self, file: &syn::File) {
        use syn::visit::{self, Visit};

        struct Collector<'a> {
            scip: &'a Scip,
            renames: HashMap<String, String>,
            stack: Vec<HashSet<String>>,
        }

        impl Collector<'_> {
            fn bind_ident(&mut self, ident: &syn::Ident) {
                let original = ident.to_string();
                let range: Range = ident.span().into();
                let Some(symbol) = self.scip.symbol_at(&range) else { return };
                let symbol = symbol.to_string();
                let mut name = original.clone();
                let mut n = 2;
                while self.stack.iter().any(|s| s.contains(&name)) {
                    name = format!("{original}{n}");
                    n += 1;
                }
                self.stack.last_mut().unwrap().insert(name.clone());
                if name != original {
                    self.renames.insert(symbol, name);
                }
            }

            fn bind_pat(&mut self, pat: &syn::Pat) {
                match pat {
                    syn::Pat::Ident(pi) => self.bind_ident(&pi.ident),
                    syn::Pat::Reference(pr) => self.bind_pat(&pr.pat),
                    syn::Pat::Struct(ps) => {
                        for f in &ps.fields {
                            self.bind_pat(&f.pat);
                        }
                    }
                    syn::Pat::Tuple(pt) => {
                        for e in &pt.elems {
                            self.bind_pat(e);
                        }
                    }
                    syn::Pat::TupleStruct(pts) => {
                        for e in &pts.elems {
                            self.bind_pat(e);
                        }
                    }
                    syn::Pat::Type(pt) => self.bind_pat(&pt.pat),
                    _ => {}
                }
            }
        }

        impl<'ast> Visit<'ast> for Collector<'_> {
            fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
                self.stack.push(HashSet::new());
                for arg in &f.sig.inputs {
                    if let syn::FnArg::Typed(pt) = arg {
                        self.bind_pat(&pt.pat);
                    }
                }
                visit::visit_block(self, &f.block);
                self.stack.pop();
            }

            fn visit_impl_item_fn(&mut self, m: &'ast syn::ImplItemFn) {
                self.stack.push(HashSet::new());
                for arg in &m.sig.inputs {
                    if let syn::FnArg::Typed(pt) = arg {
                        self.bind_pat(&pt.pat);
                    }
                }
                visit::visit_block(self, &m.block);
                self.stack.pop();
            }

            fn visit_block(&mut self, b: &'ast syn::Block) {
                self.stack.push(HashSet::new());
                visit::visit_block(self, b);
                self.stack.pop();
            }

            fn visit_local(&mut self, local: &'ast syn::Local) {
                if let Some(init) = &local.init {
                    visit::visit_expr(self, &init.expr);
                }
                self.bind_pat(&local.pat);
            }

            fn visit_expr_for_loop(&mut self, efl: &'ast syn::ExprForLoop) {
                visit::visit_expr(self, &efl.expr);
                self.stack.push(HashSet::new());
                self.bind_pat(&efl.pat);
                visit::visit_block(self, &efl.body);
                self.stack.pop();
            }

            fn visit_expr_closure(&mut self, ec: &'ast syn::ExprClosure) {
                self.stack.push(HashSet::new());
                for input in &ec.inputs {
                    self.bind_pat(input);
                }
                visit::visit_expr(self, &ec.body);
                self.stack.pop();
            }

            fn visit_expr_match(&mut self, em: &'ast syn::ExprMatch) {
                visit::visit_expr(self, &em.expr);
                for arm in &em.arms {
                    self.stack.push(HashSet::new());
                    self.bind_pat(&arm.pat);
                    if let Some((_, guard)) = &arm.guard {
                        visit::visit_expr(self, guard);
                    }
                    visit::visit_expr(self, &arm.body);
                    self.stack.pop();
                }
            }

            fn visit_expr_if(&mut self, ei: &'ast syn::ExprIf) {
                self.stack.push(HashSet::new());
                if let syn::Expr::Let(el) = &*ei.cond {
                    visit::visit_expr(self, &el.expr);
                    self.bind_pat(&el.pat);
                } else {
                    visit::visit_expr(self, &ei.cond);
                }
                visit::visit_block(self, &ei.then_branch);
                self.stack.pop();
                if let Some((_, else_expr)) = &ei.else_branch {
                    visit::visit_expr(self, else_expr);
                }
            }
        }

        let mut collector = Collector {
            scip: &self.scip,
            renames: HashMap::new(),
            stack: Vec::new(),
        };
        collector.visit_file(file);
        self.renames = collector.renames;
    }

    fn register_generic(&mut self, sig: &syn::Signature) {
        let type_params: Vec<String> = sig.generics.params.iter().filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string())
            } else {
                None
            }
        }).collect();
        if type_params.is_empty() {
            return;
        }
        let mut param_arg_index = Vec::with_capacity(type_params.len());
        for tp in &type_params {
            let mut found: Option<GenericArgRef> = None;
            for (i, arg) in sig.inputs.iter().filter(|x| matches!(x, syn::FnArg::Typed(_))).enumerate() {
                let syn::FnArg::Typed(pt) = arg else { continue };
                if let Some(path) = find_type_param(&pt.ty, tp) {
                    found = Some(GenericArgRef { arg: i, path });
                    break;
                }
            }
            let Some(r) = found else { return };
            param_arg_index.push(r);
        }
        let range: Range = sig.ident.span().into();
        let Some(symbol) = self.scip.symbol_at(&range) else { return };
        self.generic_fns.insert(symbol.to_string(), GenericFn { type_params, param_arg_index });
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
                let kind = self.scip.kind_at(&ident.span().into());
                if matches!(kind, Some(Kind::Variable) | Some(Kind::Parameter)) {
                    write!(self.out, "{}", self.rename_ident(ident)).unwrap();
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
