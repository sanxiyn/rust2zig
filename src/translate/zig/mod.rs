use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

mod call;
mod cell;
mod closure;
mod drop;
mod expr;
mod flow;
mod generic;
mod item;
mod mac;
mod pat;
mod result;
mod stmt;
mod ty;

use crate::ast::zig::{Node, Var};
use crate::scip::{Kind, Range, Scip};
use crate::translate::name::{camel_to_snake, screaming_to_camel, snake_to_camel};
use drop::DropInfo;
use generic::GenericFn;

pub enum PathMode {
    Normal,
    EnumVariant,
}

pub struct Struct {
    pub has_fields: bool,
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
    pub error_types: HashSet<String>,
    pub error_scope: RefCell<Option<String>>,
    pub renames: HashMap<String, String>,
    pub cell_types: HashSet<String>,
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
            error_types: Default::default(),
            error_scope: Default::default(),
            renames: Default::default(),
            cell_types: Default::default(),
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
        self.check_moniker_at(&ident.span().into(), expected)
    }

    pub fn check_moniker_at(&self, range: &Range, expected: &str) -> bool {
        let Some(symbol) = self.scip.symbol_at(range) else { return false };
        let suffix = match expected {
            "core::cell::Cell" => "cell/Cell#",
            "core::cell::Cell::get" => "cell/impl#[`Cell<T>`]get().",
            "core::cell::Cell::new" => "cell/impl#[`Cell<T>`]new().",
            "core::cell::Cell::set" => "cell/impl#[`Cell<T>`]set().",
            "core::iter::Iterator::enumerate" => "iter/traits/iterator/Iterator#enumerate().",
            "core::macros::assert_eq" => "macros/assert_eq!",
            "core::mem::drop" => "mem/drop().",
            "core::num::rotate_right" => "]rotate_right().",
            "core::num::wrapping_add" => "]wrapping_add().",
            "core::num::wrapping_shl" => "]wrapping_shl().",
            "core::num::wrapping_mul" => "]wrapping_mul().",
            "core::num::wrapping_sub" => "]wrapping_sub().",
            "core::ops::drop::Drop" => "ops/drop/Drop#",
            "core::option::Option" => "option/Option#",
            "core::option::Option::Some" => "option/Option#Some#",
            "core::option::Option::None" => "option/Option#None#",
            "core::option::branch" => "option/impl#[`Option<T>`][Try]branch().",
            "core::result::Result" => "result/Result#",
            "core::result::Result::Ok" => "result/Result#Ok#",
            "core::result::Result::Err" => "result/Result#Err#",
            "core::result::Result::unwrap" => "result/impl#[`Result<T, E>`]unwrap().",
            "core::result::branch" => "result/impl#[`Result<T, E>`][Try]branch().",
            "core::slice::iter" => "slice/impl#[`[T]`]iter().",
            "core::slice::len" => "slice/impl#[`[T]`]len().",
            "core::str::as_bytes" => "str/impl#[str]as_bytes().",
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
                    let has_fields = !s.fields.is_empty();
                    self.structs.insert(symbol, Struct { has_fields, impls: Default::default() });
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

        self.collect_error_types(file);
        self.collect_cell_types(file);
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
                    let name = ident.to_string();
                    Node::Identifier(name)
                } else if matches!(kind, Some(Kind::Method) | Some(Kind::StaticMethod))
                    && path.segments.len() > 1
                {
                    let ty = path.segments[path.segments.len() - 2].ident.to_string();
                    let method = snake_to_camel(&ident.to_string());
                    Node::FieldAccess(Box::new(Node::Identifier(ty)), method)
                } else if matches!(kind, Some(Kind::Constant) | Some(Kind::StaticVariable)) {
                    Node::Identifier(screaming_to_camel(&ident.to_string()))
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

pub fn dotted_name(name: &str) -> Node {
    let mut parts = name.split('.');
    let mut node = Node::Identifier(parts.next().unwrap().to_string());
    for part in parts {
        node = Node::FieldAccess(Box::new(node), part.to_string());
    }
    node
}
