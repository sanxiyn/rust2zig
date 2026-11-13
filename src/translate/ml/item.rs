use std::collections::HashSet;

use syn::visit::Visit;

use crate::ast::ml::{Case, ConstructorArguments, ConstructorDeclaration, Expression, LabelDeclaration, Longident, ModuleBinding, ModuleExpr, MutableFlag, Pattern, RecFlag, StructureItem, TypeDeclaration, TypeKind, ValueBinding};
use crate::scip::Scip;
use crate::translate::name::{camel_to_snake, escape_ml, screaming_to_snake};
use super::{Translator, apply, unit_pat};
use super::integer::integer_type_name;
use super::order::{Definition, order};

#[derive(Clone, Copy, PartialEq)]
enum Exit {
    Continue,
    Return,
}

fn body_has_exit(block: &syn::Block, exit: Exit) -> bool {
    struct Find {
        exit: Exit,
        found: bool,
    }
    impl<'ast> Visit<'ast> for Find {
        fn visit_expr_continue(&mut self, _: &'ast syn::ExprContinue) {
            self.found |= self.exit == Exit::Continue;
        }
        fn visit_expr_return(&mut self, _: &'ast syn::ExprReturn) {
            self.found |= self.exit == Exit::Return;
        }
        fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
        fn visit_item_fn(&mut self, _: &'ast syn::ItemFn) {}
    }
    let mut finder = Find { exit, found: false };
    finder.visit_block(block);
    finder.found
}

fn field_bases(scip: &Scip, block: &syn::Block) -> HashSet<String> {
    struct Collect<'a> {
        scip: &'a Scip,
        out: HashSet<String>,
    }
    impl<'ast> Visit<'ast> for Collect<'_> {
        fn visit_expr_field(&mut self, ef: &'ast syn::ExprField) {
            if let syn::Expr::Path(ep) = &*ef.base
                && let Some(ident) = ep.path.get_ident()
                && let Some(symbol) = self.scip.symbol_at(&ident.span().into())
            {
                self.out.insert(symbol.to_string());
            }
            syn::visit::visit_expr_field(self, ef);
        }
    }
    let mut collector = Collect { scip, out: Default::default() };
    collector.visit_block(block);
    collector.out
}

impl Translator {
    pub fn translate_item(&self, item: &syn::Item) -> Vec<Definition> {
        match item {
            syn::Item::Const(c) => self.translate_const(c),
            syn::Item::Enum(e) => self.module_defs(item, &e.ident),
            syn::Item::Struct(s) => self.module_defs(item, &s.ident),
            syn::Item::Fn(f) => self.translate_fn(f),
            syn::Item::Impl(i) => {
                match self.type_symbol(&i.self_ty) {
                    Some(symbol) if self.types.contains_key(&symbol) => vec![],
                    _ => self.translate_impl_items(i),
                }
            }
            _ => vec![],
        }
    }

    fn translate_const(&self, c: &syn::ItemConst) -> Vec<Definition> {
        let ident = &c.ident;
        let ty = &c.ty;
        let value = &c.expr;
        let name = escape_ml(&screaming_to_snake(&ident.to_string()));
        let expr = match integer_type_name(ty) {
            Some(ty) => self.translate_int_operand(value, self.name_repr(&ty)),
            None => self.translate_expr(value),
        };
        let item = StructureItem::Value(RecFlag::Nonrecursive, vec![ValueBinding {
            pat: Pattern::Var(name),
            params: vec![],
            expr,
        }]);
        let def = Definition {
            symbols: self.def_symbol(ident).into_iter().collect(),
            references: self.refs(|v| v.visit_expr(value)),
            item,
        };
        vec![def]
    }

    fn module_defs(&self, item: &syn::Item, ident: &syn::Ident) -> Vec<Definition> {
        let Some(type_item) = self.type_item(ident) else {
            let decl = self.translate_type_decl(item);
            return decl.into_iter().map(|item| Definition {
                symbols: self.def_symbol(ident).into_iter().collect(),
                references: Default::default(),
                item,
            }).collect();
        };
        *self.current_module.borrow_mut() = Some(type_item.module.clone());
        let mut body = self.translate_type_decl(item);
        let mut defs = vec![];
        for i in &type_item.impls {
            defs.extend(self.translate_impl_items(i));
        }
        *self.current_module.borrow_mut() = None;
        let mut symbols: HashSet<String> = self.def_symbol(ident).into_iter().collect();
        let mut references: HashSet<String> = Default::default();
        for def in &defs {
            symbols.extend(def.symbols.iter().cloned());
            references.extend(def.references.iter().cloned());
        }
        references.retain(|symbol| !symbols.contains(symbol));
        body.extend(order(defs));
        let binding = ModuleBinding {
            name: type_item.module.clone(),
            expr: ModuleExpr::Structure(body),
        };
        vec![Definition { symbols, references, item: StructureItem::Module(binding) }]
    }

    fn translate_type_decl(&self, item: &syn::Item) -> Vec<StructureItem> {
        match item {
            syn::Item::Enum(e) => vec![self.translate_enum(e)],
            syn::Item::Struct(s) => vec![self.translate_struct(s)],
            _ => vec![],
        }
    }

    fn translate_enum(&self, e: &syn::ItemEnum) -> StructureItem {
        let name = self.type_decl_name(&e.ident);
        let params = self.type_params(&e.generics);
        let constructors = e.variants.iter().map(|variant| ConstructorDeclaration {
            name: variant.ident.to_string(),
            args: self.constructor_args(&variant.fields),
        }).collect();
        let decl = TypeDeclaration { name, params, kind: TypeKind::Variant(constructors) };
        StructureItem::Type(vec![decl])
    }

    fn translate_struct(&self, s: &syn::ItemStruct) -> StructureItem {
        let name = self.type_decl_name(&s.ident);
        let params = self.type_params(&s.generics);
        let owner = s.ident.to_string();
        let labels = match &s.fields {
            syn::Fields::Named(named) => self.labels(&named.named, Some(&owner)),
            _ => vec![],
        };
        let decl = TypeDeclaration { name, params, kind: TypeKind::Record(labels) };
        StructureItem::Type(vec![decl])
    }

    fn type_decl_name(&self, ident: &syn::Ident) -> String {
        match self.type_item(ident) {
            Some(_) => "t".to_string(),
            None => escape_ml(&camel_to_snake(&ident.to_string())),
        }
    }

    fn type_params(&self, generics: &syn::Generics) -> Vec<String> {
        generics.params.iter().filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string().to_lowercase())
            } else {
                None
            }
        }).collect()
    }

    fn constructor_args(&self, fields: &syn::Fields) -> ConstructorArguments {
        match fields {
            syn::Fields::Named(named) => ConstructorArguments::Record(self.labels(&named.named, None)),
            syn::Fields::Unit => ConstructorArguments::Tuple(vec![]),
            syn::Fields::Unnamed(unnamed) => ConstructorArguments::Tuple(
                unnamed.unnamed.iter().map(|field| self.translate_type(&field.ty)).collect(),
            ),
        }
    }

    fn labels(&self, fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>, owner: Option<&str>) -> Vec<LabelDeclaration> {
        fields.iter().map(|field| {
            let name = field.ident.as_ref().unwrap().to_string();
            let mutable = if owner.is_some_and(|owner| self.mut_fields.contains(&(owner.to_string(), name.clone()))) { MutableFlag::Mutable } else { MutableFlag::Immutable };
            LabelDeclaration { name: escape_ml(&name), mutable, ty: self.translate_type(&field.ty) }
        }).collect()
    }

    fn translate_impl_items(&self, i: &syn::ItemImpl) -> Vec<Definition> {
        let mut defs = vec![];
        for ii in &i.items {
            match ii {
                syn::ImplItem::Const(c) => {
                    let def = self.translate_associated_const(c);
                    defs.push(def);
                }
                syn::ImplItem::Fn(f) => {
                    let def = self.translate_method(f);
                    defs.push(def);
                }
                _ => (),
            }
        }
        defs
    }

    fn translate_associated_const(&self, c: &syn::ImplItemConst) -> Definition {
        let ident = &c.ident;
        let ty = &c.ty;
        let value = &c.expr;
        let name = escape_ml(&screaming_to_snake(&ident.to_string()));
        let expr = match integer_type_name(ty) {
            Some(ty) => self.translate_int_operand(value, self.name_repr(&ty)),
            None => self.translate_expr(value),
        };
        let item = StructureItem::Value(RecFlag::Nonrecursive, vec![ValueBinding {
            pat: Pattern::Var(name),
            params: vec![],
            expr,
        }]);
        Definition {
            symbols: self.def_symbol(ident).into_iter().collect(),
            references: self.refs(|v| v.visit_expr(value)),
            item,
        }
    }

    fn translate_method(&self, f: &syn::ImplItemFn) -> Definition {
        Definition {
            symbols: self.def_symbol(&f.sig.ident).into_iter().collect(),
            references: self.refs(|v| v.visit_block(&f.block)),
            item: self.translate_callable(&f.sig, &f.block),
        }
    }

    fn translate_fn(&self, f: &syn::ItemFn) -> Vec<Definition> {
        let def = Definition {
            symbols: self.def_symbol(&f.sig.ident).into_iter().collect(),
            references: self.refs(|v| v.visit_block(&f.block)),
            item: self.translate_callable(&f.sig, &f.block),
        };
        vec![def]
    }

    fn translate_callable(&self, signature: &syn::Signature, block: &syn::Block) -> StructureItem {
        let field_bases = field_bases(&self.scip, block);
        let mut mut_params: Vec<String> = Default::default();
        let mut params: Vec<Pattern> = Default::default();
        for arg in &signature.inputs {
            match arg {
                syn::FnArg::Receiver(_) => params.push(Pattern::Var("self".to_string())),
                syn::FnArg::Typed(pt) => {
                    if let syn::Pat::Ident(pi) = &*pt.pat {
                        let name = escape_ml(&pi.ident.to_string());
                        let pat = if pi.mutability.is_some() {
                            mut_params.push(name.clone());
                            Pattern::Var(format!("_{name}"))
                        } else {
                            Pattern::Var(name)
                        };
                        let pat = if self.needs_constraint(pi, &pt.ty, &field_bases) {
                            Pattern::Constraint(Box::new(pat), self.translate_type(&pt.ty))
                        } else {
                            pat
                        };
                        params.push(pat);
                    }
                }
            }
        }
        if params.is_empty() {
            params.push(unit_pat());
        }

        let mut body = self.translate_block(block);
        body = self.wrap_let_op(block, body);
        if body_has_exit(block, Exit::Return) {
            if let syn::ReturnType::Type(_, ty) = &signature.output {
                let handler = Case {
                    lhs: Pattern::Construct(
                        Longident::Lident("Return".to_string()),
                        Some(Box::new(Pattern::Var("r".to_string()))),
                    ),
                    guard: None,
                    rhs: Expression::Ident(Longident::Lident("r".to_string())),
                };
                body = Expression::Try(Box::new(body), vec![handler]);
                body = Expression::LetException("Return".to_string(), Some(self.translate_type(ty)), Box::new(body));
            }
        }
        if body_has_exit(block, Exit::Continue) {
            body = Expression::LetException("Continue".to_string(), None, Box::new(body));
        }

        for name in mut_params.iter().rev() {
            let binding = ValueBinding {
                pat: Pattern::Var(name.clone()),
                params: Default::default(),
                expr: apply("ref", vec![Expression::Ident(Longident::Lident(format!("_{name}")))]),
            };
            body = Expression::Let(RecFlag::Nonrecursive, vec![binding], Box::new(body));
        }
        StructureItem::Value(RecFlag::Nonrecursive, vec![ValueBinding {
            pat: Pattern::Var(escape_ml(&signature.ident.to_string())),
            params,
            expr: body,
        }])
    }

    fn needs_constraint(&self, pi: &syn::PatIdent, ty: &syn::Type, field_bases: &HashSet<String>) -> bool {
        let Some(symbol) = self.scip.symbol_at(&pi.ident.span().into()) else { return false };
        if !field_bases.contains(symbol) {
            return false;
        }
        let Some(module) = self.type_symbol(ty).and_then(|symbol| self.types.get(&symbol)) else {
            return false;
        };
        self.current_module.borrow().as_deref() != Some(module.module.as_str())
    }

    pub fn translate_test(&self, f: &syn::ItemFn) -> StructureItem {
        let body = self.translate_block(&f.block);
        StructureItem::Value(RecFlag::Nonrecursive, vec![ValueBinding {
            pat: unit_pat(),
            params: Default::default(),
            expr: body,
        }])
    }
}
