use crate::ast::ml::Expression;
use crate::translate::ty::{expr_type, peel_ref};
use super::{qualified, Translator};

pub fn is_string_type(ty: &syn::Type) -> bool {
    match peel_ref(ty) {
        syn::Type::Path(tp) => match tp.path.segments.last() {
            Some(segment) => segment.ident == "str" || segment.ident == "String",
            None => false,
        },
        syn::Type::Slice(ts) => is_u8(&ts.elem),
        _ => false,
    }
}

fn is_u8(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else { return false };
    tp.path.is_ident("u8")
}

impl Translator {
    pub fn is_string_expr(&self, expr: &syn::Expr) -> bool {
        expr_type(&self.scip, expr).is_some_and(|ty| is_string_type(&ty))
    }

    pub fn string_get(&self, base: Expression, index: Expression) -> Expression {
        let get = Expression::StringGet(Box::new(base), Box::new(index));
        Expression::Apply(Box::new(qualified("Char", "code")), vec![get])
    }
}
