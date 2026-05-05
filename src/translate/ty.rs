use std::fmt::Write;

use super::Rust2Zig;

impl Rust2Zig {
    pub fn translate_type(&mut self, ty: &syn::Type) {
        match ty {
            syn::Type::Array(ta) => {
                write!(self.out, "[").unwrap();
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(n), .. }) = &ta.len {
                    write!(self.out, "{}", n.base10_digits()).unwrap();
                } else {
                    write!(self.out, "_").unwrap();
                }
                write!(self.out, "]").unwrap();
                self.translate_type(&ta.elem);
            }
            syn::Type::Path(tp) => {
                let segment = tp.path.segments.last().unwrap();
                let ident = &segment.ident;
                let name = ident.to_string();
                match name.as_str() {
                    "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => {
                        write!(self.out, "{}", ident).unwrap();
                    }
                    "str" => write!(self.out, "[]const u8").unwrap(),
                    _ if self.check_moniker(&tp.path, "core::option::Option") => {
                        write!(self.out, "?").unwrap();
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                self.translate_type(inner_ty);
                            }
                        }
                    }
                    _ => {
                        write!(self.out, "{}", ident).unwrap();
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            write!(self.out, "(").unwrap();
                            for (i, arg) in args.args.iter().enumerate() {
                                if i > 0 {
                                    write!(self.out, ", ").unwrap();
                                }
                                if let syn::GenericArgument::Type(arg_ty) = arg {
                                    self.translate_type(arg_ty);
                                }
                            }
                            write!(self.out, ")").unwrap();
                        }
                    }
                }
            }
            syn::Type::Reference(tr) => {
                self.translate_type(&tr.elem);
            }
            syn::Type::Tuple(tt) => {
                write!(self.out, "struct {{ ").unwrap();
                for (i, elem) in tt.elems.iter().enumerate() {
                    if i > 0 {
                        write!(self.out, ", ").unwrap();
                    }
                    self.translate_type(elem);
                }
                write!(self.out, " }}").unwrap();
            }
            _ => write!(self.out, "/* TODO: type */").unwrap(),
        }
    }

    pub fn translate_return_type(&mut self, ret: &syn::ReturnType) {
        match ret {
            syn::ReturnType::Default => write!(self.out, "void").unwrap(),
            syn::ReturnType::Type(_, ty) => self.translate_type(ty),
        }
    }
}
