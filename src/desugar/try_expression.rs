use proc_macro2::Span;
use syn::visit_mut::{self, VisitMut};

pub fn run(file: &mut syn::File) {
    Desugar { counter: 0 }.visit_file_mut(file);
}

struct Desugar {
    counter: usize,
}

impl VisitMut for Desugar {
    fn visit_item_fn_mut(&mut self, f: &mut syn::ItemFn) {
        self.counter = 0;
        visit_mut::visit_item_fn_mut(self, f);
    }

    fn visit_impl_item_fn_mut(&mut self, f: &mut syn::ImplItemFn) {
        self.counter = 0;
        visit_mut::visit_impl_item_fn_mut(self, f);
    }

    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        visit_mut::visit_block_mut(self, block);
        let mut stmts = Vec::with_capacity(block.stmts.len());
        for mut stmt in block.stmts.drain(..) {
            let mut hoisted = vec![];
            self.hoist_stmt(&mut stmt, &mut hoisted);
            stmts.extend(hoisted);
            stmts.push(stmt);
        }
        block.stmts = stmts;
    }
}

impl Desugar {
    fn hoist_stmt(&mut self, stmt: &mut syn::Stmt, hoisted: &mut Vec<syn::Stmt>) {
        match stmt {
            syn::Stmt::Local(local) => {
                let Some(init) = &mut local.init else { return };
                match &mut *init.expr {
                    syn::Expr::Try(et) => self.hoist(&mut et.expr, hoisted),
                    expr => self.hoist(expr, hoisted),
                }
            }
            syn::Stmt::Expr(expr, _) => self.hoist(expr, hoisted),
            _ => {}
        }
    }

    fn hoist(&mut self, expr: &mut syn::Expr, hoisted: &mut Vec<syn::Stmt>) {
        Hoist { desugar: self, hoisted }.visit_expr_mut(expr);
    }

    fn fresh(&mut self) -> syn::Ident {
        self.counter += 1;
        syn::Ident::new(&format!("try{}", self.counter), Span::call_site())
    }
}

struct Hoist<'a> {
    desugar: &'a mut Desugar,
    hoisted: &'a mut Vec<syn::Stmt>,
}

impl VisitMut for Hoist<'_> {
    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        visit_mut::visit_expr_mut(self, expr);
        if !matches!(expr, syn::Expr::Try(_)) {
            return;
        }
        let name = self.desugar.fresh();
        let operand = std::mem::replace(expr, syn::parse_quote!(#name));
        self.hoisted.push(syn::parse_quote!(let #name = #operand;));
    }

    fn visit_arm_mut(&mut self, _: &mut syn::Arm) {}

    fn visit_block_mut(&mut self, _: &mut syn::Block) {}

    fn visit_expr_binary_mut(&mut self, eb: &mut syn::ExprBinary) {
        self.visit_expr_mut(&mut eb.left);
        if !matches!(eb.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
            self.visit_expr_mut(&mut eb.right);
        }
    }

    fn visit_expr_closure_mut(&mut self, _: &mut syn::ExprClosure) {}
}
