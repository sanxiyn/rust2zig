use crate::ast::zig::Node;
use crate::translate::ty::type_argument;
use super::{Translator, todo};

impl Translator {
    pub fn translate_type(&self, ty: &syn::Type) -> Node {
        if let Some((ok, error)) = self.result_types(ty) {
            if !self.is_error_set(&error) {
                return todo("type");
            }
            let error = self.translate_type(&error);
            let ok = self.translate_type(&ok);
            return Node::ErrorUnion(Box::new(error), Box::new(ok));
        }
        match ty {
            syn::Type::Array(ta) => {
                let len = self.translate_expr(&ta.len);
                let ty = self.translate_type(&ta.elem);
                Node::ArrayType(Box::new(len), Box::new(ty))
            }
            syn::Type::Path(tp) => {
                let segment = tp.path.segments.last().unwrap();
                let name = segment.ident.to_string();
                match name.as_str() {
                    "bool"
                    | "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
                    | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => {
                        Node::Identifier(name)
                    }
                    _ if self.check_moniker(&tp.path, "core::cell::Cell") => {
                        if let Some(inner_ty) = type_argument(segment) {
                            self.translate_type(inner_ty)
                        } else {
                            todo("type")
                        }
                    }
                    _ if self.check_moniker(&tp.path, "core::option::Option") => {
                        if let Some(inner_ty) = type_argument(segment) {
                            let ty = self.translate_type(inner_ty);
                            Node::OptionalType(Box::new(ty))
                        } else {
                            todo("type")
                        }
                    }
                    _ => {
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            let mut type_args = vec![];
                            for arg in &args.args {
                                if let syn::GenericArgument::Type(arg_ty) = arg {
                                    let type_arg = self.translate_type(arg_ty);
                                    type_args.push(type_arg);
                                }
                            }
                            if type_args.is_empty() {
                                Node::Identifier(name)
                            } else {
                                let type_constructor = Node::Identifier(name);
                                Node::Call(Box::new(type_constructor), type_args)
                            }
                        } else {
                            Node::Identifier(name)
                        }
                    }
                }
            }
            syn::Type::Reference(tr) => {
                if let syn::Type::Slice(ts) = &*tr.elem {
                    let ty = self.translate_type(&ts.elem);
                    Node::SliceType(Box::new(ty))
                } else if is_str(&tr.elem) {
                    Node::SliceType(Box::new(Node::Identifier("u8".to_string())))
                } else {
                    let ty = self.translate_type(&tr.elem);
                    let is_const = tr.mutability.is_none()
                        && !self.is_cell_bearing(&tr.elem);
                    Node::PtrType {
                        is_const,
                        ty: Box::new(ty),
                    }
                }
            }
            syn::Type::Tuple(tt) => {
                let mut elements = vec![];
                for elem in &tt.elems {
                    let element = self.translate_type(elem);
                    elements.push(element);
                }
                Node::TupleType(elements)
            }
            _ => todo("type")
        }
    }

    pub fn translate_return_type(&self, output: &syn::ReturnType) -> Node {
        match output {
            syn::ReturnType::Default => Node::Identifier("void".to_string()),
            syn::ReturnType::Type(_, ty) => self.translate_type(ty),
        }
    }

    pub fn use_meta_eql(&self, ty: &syn::Type) -> bool {
        self.is_aggregate(ty) && self.is_pointer_free(ty, 0)
    }

    fn is_aggregate(&self, ty: &syn::Type) -> bool {
        match ty {
            syn::Type::Array(_) => true,
            syn::Type::Path(tp) => {
                let name = tp.path.segments.last().unwrap().ident.to_string();
                self.aggregates.contains_key(&name)
            }
            _ => false,
        }
    }

    fn is_pointer_free(&self, ty: &syn::Type, depth: usize) -> bool {
        const MAX_DEPTH: usize = 8;
        if depth > MAX_DEPTH {
            return false;
        }
        match ty {
            syn::Type::Array(ta) => self.is_pointer_free(&ta.elem, depth + 1),
            syn::Type::Path(tp) => {
                let segment = tp.path.segments.last().unwrap();
                let name = segment.ident.to_string();
                match name.as_str() {
                    "bool"
                    | "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
                    | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => true,
                    "Option" => match type_argument(segment) {
                        Some(inner_ty) => self.is_pointer_free(inner_ty, depth + 1),
                        None => false,
                    },
                    _ if self.plain_enums.contains(&name) => true,
                    _ => match self.aggregates.get(&name) {
                        Some(aggregate) => aggregate.fields.iter()
                            .all(|f| self.is_pointer_free(f, depth + 1)),
                        None => false,
                    },
                }
            }
            syn::Type::Tuple(tt) => tt.elems.iter().all(|e| self.is_pointer_free(e, depth + 1)),
            _ => false,
        }
    }
}

fn is_str(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else { return false };
    tp.path.is_ident("str")
}
