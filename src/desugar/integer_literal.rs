use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};

use crate::scip::Scip;

pub fn run(scip: &Scip, file: &mut syn::File) {
    Desugar { scip }.visit_file_mut(file);
}

struct Desugar<'a> {
    scip: &'a Scip,
}

impl VisitMut for Desugar<'_> {
    fn visit_expr_binary_mut(&mut self, eb: &mut syn::ExprBinary) {
        visit_mut::visit_expr_binary_mut(self, eb);
        if !matches!(eb.op, syn::BinOp::Shl(_) | syn::BinOp::Shr(_)) {
            return;
        }
        let types = self.scip.binary_type_at(&eb.op.span().into());
        let Some((ty, _)) = types else { return };
        if let syn::Expr::Lit(el) = &mut *eb.left {
            if let syn::Lit::Int(li) = &mut el.lit {
                if li.suffix().is_empty() {
                    if let Some(name) = primitive_integer_type_name(&ty) {
                        let repr = format!("{}{}", li, name);
                        *li = syn::LitInt::new(&repr, li.span());
                    }
                }
            }
        }
    }
}

fn primitive_integer_type_name(ty: &syn::Type) -> Option<&str> {
    match ty {
        syn::Type::Path(tp) => {
            let name = tp.path.segments.last().unwrap().ident.to_string();
            match name.as_str() {
                "i8" => Some("i8"),
                "i16" => Some("i16"),
                "i32" => Some("i32"),
                "i64" => Some("i64"),
                "i128" => Some("i128"),
                "isize" => Some("isize"),
                "u8" => Some("u8"),
                "u16" => Some("u16"),
                "u32" => Some("u32"),
                "u64" => Some("u64"),
                "u128" => Some("u128"),
                "usize" => Some("usize"),
                _ => None,
            }
        }
        _ => None,
    }
}
