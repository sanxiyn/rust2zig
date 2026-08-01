use crate::ast::zig::Node;
use super::Translator;

impl Translator {
    pub fn translate_type(&self, ty: &syn::Type) -> Node {
        if let Some((ok, error)) = self.result_types(ty) {
            if !self.is_error_set(&error) {
                return Node::Todo("type".to_string());
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
                let ident = &segment.ident;
                let name = ident.to_string();
                match name.as_str() {
                    "bool"
                    | "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
                    | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => {
                        Node::Identifier(name)
                    }
                    _ if self.check_moniker(&tp.path, "core::cell::Cell") => {
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                self.translate_type(inner_ty)
                            } else {
                                Node::Todo("type".to_string())
                            }
                        } else {
                            Node::Todo("type".to_string())
                        }
                    }
                    _ if self.check_moniker(&tp.path, "core::option::Option") => {
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                let ty = self.translate_type(inner_ty);
                                Node::OptionalType(Box::new(ty))
                            } else {
                                Node::Todo("type".to_string())
                            }
                        } else {
                            Node::Todo("type".to_string())
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
            _ => Node::Todo("type".to_string()),
        }
    }

    pub fn translate_return_type(&self, ret: &syn::ReturnType) -> Node {
        match ret {
            syn::ReturnType::Default => Node::Identifier("void".to_string()),
            syn::ReturnType::Type(_, ty) => self.translate_type(ty),
        }
    }
}

fn is_str(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else { return false };
    tp.path.is_ident("str")
}
