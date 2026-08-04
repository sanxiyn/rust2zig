use crate::ast::ml::{CoreType, Longident};
use crate::scip::Kind;
use crate::translate::name::{camel_to_snake, escape_ml};
use super::Translator;

impl Translator {
    pub fn translate_type(&self, ty: &syn::Type) -> CoreType {
        match ty {
            syn::Type::Path(tp) => {
                let segment = tp.path.segments.last().unwrap();
                let ident = &segment.ident;
                let name = ident.to_string();
                if self.scip.kind_at(&ident.span().into()) == Some(Kind::TypeParameter) {
                    return CoreType::Var(name.to_lowercase());
                }
                let name = match self.type_module(ident) {
                    Some(module) => self.qualify(Some(module), "t".to_string()),
                    None => Longident::Lident(map_type_name(&name)),
                };
                let mut type_args = vec![];
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(arg_ty) = arg {
                            let type_arg = self.translate_type(arg_ty);
                            type_args.push(type_arg);
                        }
                    }
                }
                CoreType::Constr(name, type_args)
            }
            _ => CoreType::Constr(Longident::Lident("_".to_string()), vec![]),
        }
    }
}

fn map_type_name(name: &str) -> String {
    match name {
        "bool" => "bool".to_string(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
        | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "int".to_string(),
        _ => escape_ml(&camel_to_snake(name)),
    }
}
