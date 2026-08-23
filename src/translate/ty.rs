use crate::scip::Scip;

pub fn peel_ref(ty: &syn::Type) -> &syn::Type {
    match ty {
        syn::Type::Reference(tr) => peel_ref(&tr.elem),
        other => other,
    }
}

pub fn int_bits(ty: &syn::Type) -> Option<u32> {
    let syn::Type::Path(tp) = ty else { return None };
    let segment = tp.path.segments.last()?;
    match segment.ident.to_string().as_str() {
        "i8" | "u8" => Some(8),
        "i16" | "u16" => Some(16),
        "i32" | "u32" => Some(32),
        "i64" | "u64" => Some(64),
        "i128" | "u128" => Some(128),
        _ => None,
    }
}

pub fn unsigned_bits(ty: &syn::Type) -> Option<u32> {
    let syn::Type::Path(tp) = ty else { return None };
    if !tp.path.segments.last()?.ident.to_string().starts_with('u') {
        return None;
    }
    int_bits(ty)
}

pub fn is_closure_type(ty: &syn::Type) -> bool {
    let syn::Type::ImplTrait(it) = ty else { return false };
    it.bounds.iter().any(|bound| {
        let syn::TypeParamBound::Trait(tb) = bound else { return false };
        let Some(last) = tb.path.segments.last() else { return false };
        matches!(last.ident.to_string().as_str(), "Fn" | "FnMut" | "FnOnce")
    })
}

pub fn expr_type(scip: &Scip, expr: &syn::Expr) -> Option<syn::Type> {
    match expr {
        syn::Expr::Binary(eb) => binary_expr_type(scip, eb),
        syn::Expr::Call(ec) => {
            let syn::Expr::Path(ep) = &*ec.func else { return None };
            let ident = &ep.path.segments.last()?.ident;
            scip.return_type_at(&ident.span().into())
        }
        syn::Expr::Cast(ec) => Some((*ec.ty).clone()),
        syn::Expr::Index(ei) => match peel_ref(&expr_type(scip, &ei.expr)?) {
            syn::Type::Array(ta) => Some((*ta.elem).clone()),
            syn::Type::Slice(ts) => Some((*ts.elem).clone()),
            _ => None,
        },
        syn::Expr::MethodCall(emc) => scip.return_type_at(&emc.method.span().into()),
        syn::Expr::Paren(ep) => expr_type(scip, &ep.expr),
        syn::Expr::Path(ep) => {
            let ident = &ep.path.segments.last()?.ident;
            scip.type_at(&ident.span().into())
        }
        _ => None,
    }
}

fn binary_expr_type(scip: &Scip, eb: &syn::ExprBinary) -> Option<syn::Type> {
    match eb.op {
        syn::BinOp::Shl(_) | syn::BinOp::Shr(_) => expr_type(scip, &eb.left),
        syn::BinOp::Add(_)
        | syn::BinOp::BitAnd(_)
        | syn::BinOp::BitOr(_)
        | syn::BinOp::BitXor(_)
        | syn::BinOp::Div(_)
        | syn::BinOp::Mul(_)
        | syn::BinOp::Rem(_)
        | syn::BinOp::Sub(_) => {
            expr_type(scip, &eb.left).or_else(|| expr_type(scip, &eb.right))
        }
        _ => None,
    }
}
