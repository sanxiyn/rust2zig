use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use syn::visit::Visit;

mod call;
mod expr;
mod flow;
mod integer;
mod item;
mod mac;
mod order;
mod pat;
mod result;
mod stmt;
mod string;
mod ty;
mod vec;

use crate::ast::ml::{Expression, Longident, Pattern, Structure};
use crate::scip::{Kind, Range, Scip};
use crate::translate::moniker;
use crate::translate::name::{escape_ml, screaming_to_snake};
use crate::translate::ty::{expr_type, peel_ref};

const CORE_VARIANTS: &[&str] = &[
    "core::option::Option::Some",
    "core::option::Option::None",
    "core::result::Result::Ok",
    "core::result::Result::Err",
];

pub fn apply(name: &str, args: Vec<Expression>) -> Expression {
    let func = Expression::Ident(Longident::Lident(name.to_string()));
    Expression::Apply(Box::new(func), args)
}

pub fn qualified(module: &str, name: &str) -> Expression {
    let qualified = Longident::Ldot(Box::new(Longident::Lident(module.to_string())), name.to_string());
    Expression::Ident(qualified)
}

pub fn raise(name: &str) -> Expression {
    apply("raise", vec![Expression::Construct(Longident::Lident(name.to_string()), None)])
}

pub fn todo(what: &str) -> Expression {
    Expression::Todo(what.to_string())
}

pub fn unit() -> Expression {
    Expression::Construct(Longident::Lident("()".to_string()), None)
}

pub fn unit_pat() -> Pattern {
    Pattern::Construct(Longident::Lident("()".to_string()), None)
}

pub struct TypeItem {
    pub module: String,
    pub impls: Vec<syn::ItemImpl>,
}

pub struct Translator {
    pub types: HashMap<String, TypeItem>,
    pub variants: HashMap<String, String>,
    pub ref_vars: HashSet<String>,
    pub mut_fields: HashSet<(String, String)>,
    pub escalated: HashSet<String>,
    pub current_module: RefCell<Option<String>>,
    pub scip: Scip,
}

impl Translator {
    pub fn new(scip: Scip) -> Self {
        Translator {
            types: Default::default(),
            variants: Default::default(),
            ref_vars: Default::default(),
            mut_fields: Default::default(),
            escalated: Default::default(),
            current_module: Default::default(),
            scip,
        }
    }

    pub fn check_moniker(&self, path: &syn::Path, expected: &str) -> bool {
        let ident = &path.segments.last().unwrap().ident;
        self.check_moniker_ident(ident, expected)
    }

    pub fn check_moniker_ident(&self, ident: &syn::Ident, expected: &str) -> bool {
        self.check_moniker_at(&ident.span().into(), expected)
    }

    pub fn check_moniker_at(&self, range: &Range, expected: &str) -> bool {
        let Some(symbol) = self.scip.symbol_at(range) else { return false };
        let Some(suffix) = moniker::suffix(expected) else { return false };
        symbol.ends_with(suffix)
    }

    pub fn is_variant(&self, path: &syn::Path) -> bool {
        let ident = &path.segments.last().unwrap().ident;
        if self.scip.kind_at(&ident.span().into()) == Some(Kind::EnumMember) {
            return true;
        }
        CORE_VARIANTS.iter().any(|moniker| self.check_moniker(path, moniker))
    }

    pub fn analyze(&mut self, file: &syn::File) {
        self.collect_types(file);
        self.collect_refs(file);
        self.collect_widths(file);
    }

    fn collect_types(&mut self, file: &syn::File) {
        for item in &file.items {
            let ident = match item {
                syn::Item::Enum(e) => &e.ident,
                syn::Item::Struct(s) => &s.ident,
                _ => continue,
            };
            let Some(symbol) = self.scip.symbol_at(&ident.span().into()) else { continue };
            let module = ident.to_string();
            if let syn::Item::Enum(e) = item {
                for variant in &e.variants {
                    if let Some(symbol) = self.scip.symbol_at(&variant.ident.span().into()) {
                        self.variants.insert(symbol.to_string(), module.clone());
                    }
                }
            }
            let type_item = TypeItem { module, impls: Default::default() };
            self.types.insert(symbol.to_string(), type_item);
        }
        for item in &file.items {
            let syn::Item::Impl(i) = item else { continue };
            let Some(symbol) = self.type_symbol(&i.self_ty) else { continue };
            if let Some(type_item) = self.types.get_mut(&symbol) {
                type_item.impls.push(i.clone());
            }
        }
    }

    pub fn type_symbol(&self, ty: &syn::Type) -> Option<String> {
        let syn::Type::Path(tp) = peel_ref(ty) else { return None };
        let ident = &tp.path.segments.last()?.ident;
        Some(self.scip.symbol_at(&ident.span().into())?.to_string())
    }

    pub fn type_item(&self, ident: &syn::Ident) -> Option<&TypeItem> {
        let symbol = self.scip.symbol_at(&ident.span().into())?;
        self.types.get(symbol)
    }

    pub fn type_module(&self, ident: &syn::Ident) -> Option<&str> {
        Some(&self.type_item(ident)?.module)
    }

    pub fn variant_module(&self, ident: &syn::Ident) -> Option<&str> {
        let symbol = self.scip.symbol_at(&ident.span().into())?;
        Some(self.variants.get(symbol)?)
    }

    pub fn expr_module(&self, expr: &syn::Expr) -> Option<&str> {
        let ty = expr_type(&self.scip, expr)?;
        let syn::Type::Path(tp) = peel_ref(&ty) else { return None };
        let name = tp.path.segments.last()?.ident.to_string();
        self.types.values().find(|t| t.module == name).map(|t| t.module.as_str())
    }

    pub fn path_module(&self, path: &syn::Path) -> Option<&str> {
        let qualifier = path.segments.iter().nth_back(1)?;
        self.type_module(&qualifier.ident)
    }

    pub fn variant_name(&self, path: &syn::Path) -> Longident {
        let ident = &path.segments.last().unwrap().ident;
        let name = if self.check_moniker_ident(ident, "core::result::Result::Err") {
            "Error".to_string()
        } else {
            ident.to_string()
        };
        self.qualify(self.variant_module(ident), name)
    }

    pub fn qualify(&self, module: Option<&str>, name: String) -> Longident {
        match module {
            Some(module) if self.current_module.borrow().as_deref() != Some(module) => {
                Longident::Ldot(Box::new(Longident::Lident(module.to_string())), name)
            }
            _ => Longident::Lident(name),
        }
    }

    fn collect_refs(&mut self, file: &syn::File) {
        struct Collect<'a> {
            scip: &'a Scip,
            mut_syms: HashSet<String>,
            assigned: HashSet<String>,
            mut_fields: HashSet<(String, String)>,
        }

        impl Collect<'_> {
            fn base_struct(&self, base: &syn::Expr) -> Option<String> {
                let syn::Expr::Path(ep) = base else { return None };
                let ident = &ep.path.segments.last()?.ident;
                let ty = self.scip.type_at(&ident.span().into())?;
                let syn::Type::Path(tp) = peel_ref(&ty) else { return None };
                Some(tp.path.segments.last()?.ident.to_string())
            }
        }

        impl<'ast> Visit<'ast> for Collect<'_> {
            fn visit_pat_ident(&mut self, pi: &'ast syn::PatIdent) {
                if pi.mutability.is_some() {
                    if let Some(symbol) = self.scip.symbol_at(&pi.ident.span().into()) {
                        self.mut_syms.insert(symbol.to_string());
                    }
                }
                syn::visit::visit_pat_ident(self, pi);
            }

            fn visit_expr_assign(&mut self, ea: &'ast syn::ExprAssign) {
                match &*ea.left {
                    syn::Expr::Path(ep) => {
                        let ident = &ep.path.segments.last().unwrap().ident;
                        if let Some(symbol) = self.scip.symbol_at(&ident.span().into()) {
                            self.assigned.insert(symbol.to_string());
                        }
                    }
                    syn::Expr::Field(ef) => {
                        if let (Some(owner), syn::Member::Named(field)) = (self.base_struct(&ef.base), &ef.member) {
                            self.mut_fields.insert((owner, field.to_string()));
                        }
                    }
                    _ => {}
                }
                syn::visit::visit_expr_assign(self, ea);
            }
        }

        let mut collector = Collect {
            scip: &self.scip,
            mut_syms: Default::default(),
            assigned: Default::default(),
            mut_fields: Default::default(),
        };
        collector.visit_file(file);
        self.ref_vars = collector.mut_syms.intersection(&collector.assigned).cloned().collect();
        self.mut_fields = collector.mut_fields;
    }

    pub fn translate_file(&self, file: &syn::File) -> (Structure, Structure) {
        let mut defs = vec![];
        let mut test = vec![];
        for item in &file.items {
            if let syn::Item::Fn(f) = item {
                if f.attrs.iter().any(|a| a.path().is_ident("test")) {
                    test.push(self.translate_test(f));
                    continue;
                }
            }
            defs.extend(self.item_defs(item));
        }
        (order::order(defs), test)
    }

    pub fn translate_path(&self, path: &syn::Path) -> Expression {
        let ident = &path.segments.last().unwrap().ident;
        let kind = self.scip.kind_at(&ident.span().into());
        if self.is_variant(path) {
            return Expression::Construct(self.variant_name(path), None);
        }
        if let Some(module) = self.path_module(path) {
            return Expression::Ident(self.qualify(Some(module), escape_ml(&ident.to_string())));
        }
        if matches!(kind, Some(Kind::Parameter) | Some(Kind::Variable) | Some(Kind::SelfParameter)) {
            let name = escape_ml(&ident.to_string());
            if let Some(symbol) = self.scip.symbol_at(&ident.span().into()) {
                if self.ref_vars.contains(symbol) {
                    return apply("!", vec![Expression::Ident(Longident::Lident(name))]);
                }
            }
            Expression::Ident(Longident::Lident(name))
        } else if matches!(kind, Some(Kind::Constant) | Some(Kind::StaticVariable)) {
            Expression::Ident(Longident::Lident(escape_ml(&screaming_to_snake(&ident.to_string()))))
        } else {
            Expression::Ident(Longident::Lident(escape_ml(&ident.to_string())))
        }
    }
}
