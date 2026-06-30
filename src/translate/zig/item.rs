use crate::ast::zig::{EnumVariant, Field, Node, Param};
use crate::translate::name::{camel_to_snake, escape_zig, snake_to_camel};
use super::Translator;

impl Translator {
    pub fn translate_item(&self, item: &syn::Item) -> Option<Node> {
        match item {
            syn::Item::Enum(e) => Some(self.translate_enum(e)),
            syn::Item::Struct(s) => Some(self.translate_struct(s)),
            syn::Item::Fn(f) => Some(self.translate_fn(f)),
            _ => None,
        }
    }

    fn translate_enum(&self, e: &syn::ItemEnum) -> Node {
        let name = e.ident.to_string();
        let symbol = self.scip.symbol_at(&e.ident.span().into()).unwrap().to_string();
        let enum_ = &self.enums[&symbol];
        let has_data = enum_.has_data;
        let impls = &enum_.impls;

        let type_params = e.generics.params.iter().filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p { Some(tp.ident.to_string()) } else { None }
        }).collect();
        let variants = e.variants.iter().map(|variant| EnumVariant {
            name: camel_to_snake(&variant.ident.to_string()),
            payload: self.translate_variant_payload(&variant.fields),
        }).collect();
        let methods = self.translate_methods(impls);
        Node::EnumDecl { name, type_params, is_union: has_data, variants, methods }
    }

    fn translate_struct(&self, s: &syn::ItemStruct) -> Node {
        let name = s.ident.to_string();
        let symbol = self.scip.symbol_at(&s.ident.span().into()).unwrap().to_string();
        let struct_ = &self.structs[&symbol];
        let impls = &struct_.impls;

        let fields = s.fields.iter().map(|field| Field {
            name: field.ident.as_ref().unwrap().to_string(),
            ty: self.translate_type(&field.ty),
        }).collect();
        let methods = self.translate_methods(impls);
        Node::StructDecl { name, fields, methods }
    }

    fn translate_methods(&self, impls: &[syn::ItemImpl]) -> Vec<Node> {
        let mut impl_items = vec![];
        for i in impls {
            for ii in &i.items {
                match ii {
                    syn::ImplItem::Fn(method) => {
                        let method = self.translate_method(method);
                        impl_items.push(method);
                    }
                    _ => {
                        let impl_item = Node::Todo("impl item".to_string());
                        impl_items.push(impl_item);
                    }
                }
            }
        }
        impl_items
    }

    fn translate_method(&self, method: &syn::ImplItemFn) -> Node {
        let name = escape_zig(&snake_to_camel(&method.sig.ident.to_string()));
        let receiver = method.sig.inputs.iter()
            .filter(|arg| matches!(arg, syn::FnArg::Receiver(_)))
            .filter_map(|arg| self.translate_fn_arg(arg, &[]));
        let typed = method.sig.inputs.iter()
            .filter(|arg| matches!(arg, syn::FnArg::Typed(_)))
            .filter_map(|arg| self.translate_fn_arg(arg, &[]));
        let params = receiver
            .chain(self.comptime_params(&method.sig.ident))
            .chain(typed)
            .collect();
        let return_type = Box::new(self.translate_return_type(&method.sig.output));
        let body = Box::new(self.translate_block(&method.block));
        Node::FnDecl { name, params, return_type, body }
    }

    fn translate_variant_payload(&self, fields: &syn::Fields) -> Option<Node> {
        match fields {
            syn::Fields::Unit => None,
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                Some(self.translate_type(&fields.unnamed[0].ty))
            }
            syn::Fields::Unnamed(fields) => Some(Node::TupleType(
                fields.unnamed.iter().map(|field| self.translate_type(&field.ty)).collect(),
            )),
            syn::Fields::Named(fields) => Some(Node::StructType(
                fields.named.iter().map(|field| Field {
                    name: field.ident.as_ref().unwrap().to_string(),
                    ty: self.translate_type(&field.ty),
                }).collect(),
            )),
        }
    }

    fn translate_fn(&self, f: &syn::ItemFn) -> Node {
        let is_test = f.attrs.iter().any(|a| a.path().is_ident("test"));
        if is_test {
            return self.translate_test(f);
        }

        let mut mut_params: Vec<String> = Default::default();
        for arg in &f.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pi) = &*pat_type.pat {
                    if pi.mutability.is_some() {
                        mut_params.push(self.rename_ident(&pi.ident));
                    }
                }
            }
        }

        let name = escape_zig(&snake_to_camel(&f.sig.ident.to_string()));
        let mut params = self.comptime_params(&f.sig.ident);
        params.extend(f.sig.inputs.iter().filter_map(|arg| self.translate_fn_arg(arg, &mut_params)));
        let return_type = Box::new(self.translate_return_type(&f.sig.output));
        let preamble: Vec<Node> = mut_params.iter().map(|name| Node::SimpleVarDecl {
            is_const: false,
            name: name.clone(),
            ty: None,
            expr: Some(Box::new(Node::Identifier(format!("_{name}")))),
        }).collect();
        let body = Box::new(self.translate_block_with_preamble(&f.block, preamble));
        Node::FnDecl { name, params, return_type, body }
    }

    fn translate_test(&self, f: &syn::ItemFn) -> Node {
        let name = f.sig.ident.to_string();
        let test_name = name.strip_prefix("test_").unwrap_or(&name).to_string();
        let body = Box::new(self.translate_block(&f.block));
        Node::TestDecl(Some(test_name), body)
    }

    fn translate_fn_arg(&self, arg: &syn::FnArg, mut_params: &[String]) -> Option<Param> {
        match arg {
            syn::FnArg::Receiver(receiver) => {
                let self_ty = Node::Identifier("Self".to_string());
                let ty = if receiver.reference.is_some() {
                    let is_const = receiver.mutability.is_none();
                    Node::PtrType { is_const, ty: Box::new(self_ty) }
                } else {
                    self_ty
                };
                Some(Param { comptime: false, name: "self".to_string(), ty })
            }
            syn::FnArg::Typed(pat_type) => {
                let name = if let syn::Pat::Ident(pi) = &*pat_type.pat {
                    let name = self.rename_ident(&pi.ident);
                    if mut_params.contains(&name) {
                        format!("_{}", name)
                    } else {
                        name
                    }
                } else {
                    return None
                };
                let ty = self.translate_type(&pat_type.ty);
                Some(Param { comptime: false, name, ty })
            }
        }
    }

    fn comptime_params(&self, ident: &syn::Ident) -> Vec<Param> {
        self.type_params(ident).into_iter().map(|name| Param {
            comptime: true,
            name,
            ty: Node::Identifier("type".to_string()),
        }).collect()
    }
}
