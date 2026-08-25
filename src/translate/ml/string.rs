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

    pub fn translate_char_at(&self, emc: &syn::ExprMethodCall) -> Option<Expression> {
        if !self.check_moniker_ident(&emc.method, "core::option::Option::unwrap") {
            return None;
        }
        let syn::Expr::MethodCall(next) = &*emc.receiver else { return None };
        if !self.check_moniker_ident(&next.method, "core::str::Chars::next") {
            return None;
        }
        let syn::Expr::MethodCall(chars) = &*next.receiver else { return None };
        if !self.check_moniker_ident(&chars.method, "core::str::chars") {
            return None;
        }
        let syn::Expr::Index(index) = &*chars.receiver else { return None };
        let syn::Expr::Range(range) = &*index.index else { return None };
        let (Some(start), None) = (&range.start, &range.end) else { return None };
        let base = self.translate_expr(&index.expr);
        let offset = self.translate_expr(start);
        let get = qualified("String", "get_utf_8_uchar");
        let decode = Expression::Apply(Box::new(get), vec![base, offset]);
        let uchar = qualified("Uchar", "utf_decode_uchar");
        Some(Expression::Apply(Box::new(uchar), vec![decode]))
    }
}
