use lexpr::{Value, sexp};

use crate::translate::ty::is_closure_type;
use super::{Translator, list_of, symbol, todo, type_name};

impl Translator {
    pub fn translate_type(&self, ty: &syn::Type) -> Value {
        match ty {
            syn::Type::Array(_) | syn::Type::Slice(_) => sexp!(vector),
            syn::Type::ImplTrait(_) if is_closure_type(ty) => sexp!(function),
            syn::Type::Paren(tp) => self.translate_type(&tp.elem),
            syn::Type::Path(tp) => {
                let segment = tp.path.segments.last().unwrap();
                let name = segment.ident.to_string();
                if let Some(bits) = name.strip_prefix('i').and_then(|bits| bits.parse::<u32>().ok()) {
                    return sexp!((# "signed-byte" ,bits));
                }
                if let Some(bits) = name.strip_prefix('u').and_then(|bits| bits.parse::<u32>().ok()) {
                    return sexp!((# "unsigned-byte" ,bits));
                }
                match name.as_str() {
                    "bool" => sexp!(boolean),
                    "char" => sexp!(character),
                    "isize" | "usize" => sexp!(fixnum),
                    "str" | "String" | "Vec" => sexp!(vector),
                    _ if self.check_moniker(&tp.path, "alloc::boxed::Box") => {
                        if let Some(inner_ty) = type_argument(segment) {
                            self.translate_type(inner_ty)
                        } else {
                            todo("type")
                        }
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
                            sexp!((or null ,ty))
                        } else {
                            todo("type")
                        }
                    }
                    _ if self.structs.contains_key(&name) || self.enums.contains_key(&name) => {
                        symbol(&type_name(&name))
                    }
                    _ => sexp!(t),
                }
            }
            syn::Type::Reference(tr) => self.translate_type(&tr.elem),
            syn::Type::Tuple(tt) if tt.elems.is_empty() => sexp!(null),
            _ => sexp!(t),
        }
    }

    pub fn translate_return_type(&self, output: &syn::ReturnType) -> Value {
        match output {
            syn::ReturnType::Default => sexp!(null),
            syn::ReturnType::Type(_, ty) => {
                match &**ty {
                    syn::Type::Tuple(tt) if !tt.elems.is_empty() => {
                        let tys: Vec<_> = tt.elems.iter().map(|elem| self.translate_type(elem)).collect();
                        list_of("values", tys)
                    }
                    ty => self.translate_type(ty),
                }
            }
        }
    }
}

fn type_argument(segment: &syn::PathSegment) -> Option<&syn::Type> {
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else { return None };
    let syn::GenericArgument::Type(ty) = args.args.first()? else { return None };
    Some(ty)
}
