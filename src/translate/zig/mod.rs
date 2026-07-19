use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

mod call;
mod closure;
mod drop;
mod expr;
mod flow;
mod generic;
mod item;
mod mac;
mod pat;
mod rename;
mod stmt;
mod ty;

use crate::ast::zig::{Node, Var};
use crate::scip::{Kind, Scip};
use crate::translate::name::{camel_to_snake, snake_to_camel};
use drop::DropInfo;
use generic::GenericFn;

pub enum PathMode {
    Normal,
    EnumVariant,
}

pub struct Struct {
    pub impls: Vec<syn::ItemImpl>,
}

pub struct Enum {
    pub has_data: bool,
    pub impls: Vec<syn::ItemImpl>,
}

pub struct Translator {
    pub structs: HashMap<String, Struct>,
    pub enums: HashMap<String, Enum>,
    pub generic_fns: HashMap<String, GenericFn>,
    pub renames: HashMap<String, String>,
    pub drop_types: HashSet<String>,
    pub drop_infos: HashMap<String, DropInfo>,
    pub capture_stack: RefCell<Vec<HashMap<String, String>>>,
    pub scip: Scip,
}

impl Translator {
    pub fn new(scip: Scip) -> Self {
        Translator {
            structs: Default::default(),
            enums: Default::default(),
            generic_fns: Default::default(),
            renames: Default::default(),
            drop_types: Default::default(),
            drop_infos: Default::default(),
            capture_stack: Default::default(),
            scip,
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
            "core::iter::Iterator::enumerate" => "iter/traits/iterator/Iterator#enumerate().",
            "core::macros::assert_eq" => "macros/assert_eq!",
            "core::mem::drop" => "mem/drop().",
            "core::ops::drop::Drop" => "ops/drop/Drop#",
            "core::option::Option" => "option/Option#",
            "core::option::Option::Some" => "option/Option#Some#",
            "core::option::Option::None" => "option/Option#None#",
            "core::slice::iter" => "slice/impl#[`[T]`]iter().",
            "core::slice::len" => "slice/impl#[`[T]`]len().",
            "std::iter::zip" => "iter/adapters/zip/zip().",
            "std::macros::assert" => "macros/builtin/assert!",
            "std::macros::panic" => "macros/panic!",
            "std::macros::println" => "macros/println!",
            _ => return false,
        };
        symbol.ends_with(suffix)
    }

    pub fn analyze(&mut self, file: &syn::File) {
        for item in &file.items {
            match item {
                syn::Item::Enum(e) => {
                    let Some(symbol) = self.scip.symbol_at(&e.ident.span().into()) else { continue };
                    let symbol = symbol.to_string();
                    let has_data = e.variants.iter().any(|v| !v.fields.is_empty());
                    self.enums.insert(symbol, Enum { has_data, impls: Default::default() });
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
                        if let Some((_, path, _)) = &i.trait_ {
                            if self.check_moniker(path, "core::ops::drop::Drop") {
                                self.drop_types.insert(symbol.to_string());
                            }
                        }
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
        self.collect_drop_infos(file);
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

    pub fn translate_file(&self, file: &syn::File) -> Node {
        let mut items = vec![Node::SimpleVarDecl {
            var: Var { is_const: true, name: "std".to_string(), ty: None },
            expr: Some(Box::new(Node::BuiltinCall(
                "import".to_string(),
                vec![Node::StringLiteral("std".to_string())],
            ))),
        }];
        for item in &file.items {
            if let Some(node) = self.translate_item(item) {
                items.push(node);
            }
        }
        Node::Root(items)
    }

    pub fn translate_path(&self, path: &syn::Path, mode: PathMode) -> Node {
        let ident = &path.segments.last().unwrap().ident;
        match mode {
            PathMode::Normal => {
                let kind = self.scip.kind_at(&ident.span().into());
                if matches!(kind, Some(Kind::Parameter) | Some(Kind::Variable)) {
                    if let Some(map) = self.capture_stack.borrow().last() {
                        if let Some(symbol) = self.scip.symbol_at(&ident.span().into()) {
                            if let Some(field) = map.get(symbol) {
                                return Node::FieldAccess(Box::new(Node::Identifier("self".to_string())), field.clone());
                            }
                        }
                    }
                    let name = self.rename_ident(ident);
                    Node::Identifier(name)
                } else if matches!(kind, Some(Kind::Method) | Some(Kind::StaticMethod))
                    && path.segments.len() > 1
                {
                    let ty = path.segments[path.segments.len() - 2].ident.to_string();
                    let method = snake_to_camel(&ident.to_string());
                    Node::FieldAccess(Box::new(Node::Identifier(ty)), method)
                } else if matches!(kind, Some(Kind::StaticVariable)) {
                    Node::Identifier(snake_to_camel(&ident.to_string().to_ascii_lowercase()))
                } else {
                    Node::Identifier(snake_to_camel(&ident.to_string()))
                }
            }
            PathMode::EnumVariant => {
                Node::EnumLiteral(camel_to_snake(&ident.to_string()))
            }
        }
    }

    pub fn drop_call(&self, name: &str) -> Node {
        Node::Call(
            Box::new(Node::FieldAccess(
                Box::new(Node::Identifier(name.to_string())),
                "drop".to_string(),
            )),
            vec![],
        )
    }
}
