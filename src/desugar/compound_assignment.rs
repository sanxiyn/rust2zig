use syn::visit_mut::{self, VisitMut};

pub fn run(file: &mut syn::File) {
    Desugar.visit_file_mut(file);
}

struct Desugar;

impl VisitMut for Desugar {
    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        visit_mut::visit_expr_mut(self, expr);
        if let Some(rewritten) = rewrite(expr) {
            *expr = rewritten;
        }
    }
}

fn rewrite(expr: &syn::Expr) -> Option<syn::Expr> {
    let syn::Expr::Binary(eb) = expr else { return None };
    let left = (*eb.left).clone();
    let right = (*eb.right).clone();
    let new: syn::Expr = match &eb.op {
        syn::BinOp::AddAssign(_) => syn::parse_quote!(#left = #left + #right),
        syn::BinOp::DivAssign(_) => syn::parse_quote!(#left = #left / #right),
        syn::BinOp::MulAssign(_) => syn::parse_quote!(#left = #left * #right),
        syn::BinOp::RemAssign(_) => syn::parse_quote!(#left = #left % #right),
        syn::BinOp::SubAssign(_) => syn::parse_quote!(#left = #left - #right),
        _ => return None,
    };
    Some(new)
}
