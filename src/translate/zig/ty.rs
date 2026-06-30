use crate::ast::zig::Node;
use super::Translator;

impl Translator {
    pub fn translate_type(&self, ty: &syn::Type) -> Node {
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
                            let type_constructor = Node::Identifier(name);
                            let mut type_args = vec![];
                            for arg in &args.args {
                                if let syn::GenericArgument::Type(arg_ty) = arg {
                                    let type_arg = self.translate_type(arg_ty);
                                    type_args.push(type_arg);
                                }
                            }
                            Node::Call(Box::new(type_constructor), type_args)
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
                } else if tr.mutability.is_some() {
                    let ty = self.translate_type(&tr.elem);
                    Node::PtrType {
                        is_const: false,
                        ty: Box::new(ty),
                    }
                } else {
                    let ty = self.translate_type(&tr.elem);
                    Node::PtrType {
                        is_const: true,
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
