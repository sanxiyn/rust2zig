use crate::ast::zig::{Node, Var};
use super::Translator;

impl Translator {
    fn translate_stmt(&self, stmt: &syn::Stmt, is_last: bool) -> Node {
        match stmt {
            syn::Stmt::Expr(expr, semi) => {
                if is_last && semi.is_none() && !matches!(expr, syn::Expr::If(_)) {
                    let expr = self.translate_expr(expr);
                    Node::Return(Some(Box::new(expr)))
                } else {
                    self.translate_expr(expr)
                }
            }
            syn::Stmt::Local(local) => self.translate_local(local),
            syn::Stmt::Macro(sm) => self.translate_macro(&sm.mac)
                .unwrap_or_else(|| Node::Todo("macro".to_string())),
            _ => Node::Todo("stmt".to_string()),
        }
    }

    fn translate_local(&self, local: &syn::Local) -> Node {
        if let syn::Pat::Tuple(pt) = &local.pat {
            let vars = pt.elems.iter().map(|elem| match elem {
                syn::Pat::Ident(pi) => self.var_of(pi),
                _ => Var { is_const: true, name: "_".to_string(), ty: None },
            }).collect();
            let expr = self.translate_expr(&local.init.as_ref().unwrap().expr);
            return Node::AssignDestructure(vars, Box::new(expr));
        }
        let pat = match &local.pat {
            syn::Pat::Type(pt) => &*pt.pat,
            pat => pat,
        };
        let syn::Pat::Ident(pi) = pat else {
            return Node::Todo("local".to_string());
        };
        if let Some(init) = &local.init {
            if let syn::Expr::Closure(ec) = &*init.expr {
                return self.translate_closure_local(pi, ec);
            }
        }
        let expr = self.translate_expr(&local.init.as_ref().unwrap().expr);
        Node::SimpleVarDecl { var: self.var_of(pi), expr: Some(Box::new(expr)) }
    }

    fn var_of(&self, pi: &syn::PatIdent) -> Var {
        let is_const = pi.mutability.is_none();
        let name = self.rename_ident(&pi.ident);
        let ty = if let Some(ty) = self.scip.type_at(&pi.ident.span().into()) {
            let ty = self.translate_type(&ty);
            Some(Box::new(ty))
        } else {
            None
        };
        Var { is_const, name, ty }
    }

    pub fn translate_block(&self, block: &syn::Block) -> Node {
        self.translate_block_with_preamble(block, Default::default())
    }

    pub fn translate_block_with_preamble(&self, block: &syn::Block, preamble: Vec<Node>) -> Node {
        let mut stmts = preamble;
        let count = block.stmts.len();
        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i + 1 == count;
            stmts.push(self.translate_stmt(stmt, is_last));
        }
        Node::Block(stmts)
    }
}
