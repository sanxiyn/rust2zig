use crate::ast::zig::{Node, Var};
use super::Translator;

impl Translator {
    fn translate_stmt(&self, stmt: &syn::Stmt, is_last: bool) -> Vec<Node> {
        match stmt {
            syn::Stmt::Expr(expr, semi) => {
                if let syn::Expr::Unsafe(eu) = expr {
                    let mut nodes = Vec::new();
                    let count = eu.block.stmts.len();
                    for (i, inner) in eu.block.stmts.iter().enumerate() {
                        let inner_last = is_last && semi.is_none() && i + 1 == count;
                        nodes.extend(self.translate_stmt(inner, inner_last));
                    }
                    return nodes;
                }
                let mut nodes = self.prelude_clear_flags(expr);
                if is_last && semi.is_none() && !matches!(expr, syn::Expr::If(_)) {
                    let expr = self.translate_expr(expr);
                    nodes.push(Node::Return(Some(Box::new(expr))));
                } else {
                    nodes.push(self.translate_expr(expr));
                }
                nodes
            }
            syn::Stmt::Local(local) => self.translate_local(local),
            syn::Stmt::Macro(sm) => vec![self.translate_macro(&sm.mac)
                .unwrap_or_else(|| Node::Todo("macro".to_string()))],
            _ => vec![Node::Todo("stmt".to_string())],
        }
    }

    fn translate_local(&self, local: &syn::Local) -> Vec<Node> {
        if let syn::Pat::Tuple(pt) = &local.pat {
            let vars = pt.elems.iter().map(|elem| match elem {
                syn::Pat::Ident(pi) => self.var_of(pi, false),
                _ => Var { is_const: true, name: "_".to_string(), ty: None },
            }).collect();
            let init = &local.init.as_ref().unwrap().expr;
            let mut nodes = self.prelude_clear_flags(init);
            let expr = self.translate_expr(init);
            nodes.push(Node::AssignDestructure(vars, Box::new(expr)));
            return nodes;
        }
        let pat = match &local.pat {
            syn::Pat::Type(pt) => &*pt.pat,
            pat => pat,
        };
        if matches!(pat, syn::Pat::Wild(_)) {
            let init = &local.init.as_ref().unwrap().expr;
            let mut nodes = self.prelude_clear_flags(init);
            let expr = self.translate_expr(init);
            nodes.push(Node::Assign(
                Box::new(Node::Identifier("_".to_string())),
                Box::new(expr),
            ));
            return nodes;
        }
        let syn::Pat::Ident(pi) = pat else {
            return vec![Node::Todo("local".to_string())];
        };
        if let Some(init) = &local.init {
            if let syn::Expr::Closure(ec) = &*init.expr {
                return vec![self.translate_closure_local(pi, ec)];
            }
        }
        let init = &local.init.as_ref().unwrap().expr;
        let mut nodes = self.prelude_clear_flags(init);
        let (needs_var, needs_defer, needs_flag) = self.local_drop_flags(&pi.ident);
        let expr = self.translate_expr(init);
        nodes.push(Node::SimpleVarDecl {
            var: self.var_of(pi, needs_var),
            expr: Some(Box::new(expr)),
        });
        if needs_flag {
            let zig_name = self.rename_ident(&pi.ident);
            nodes.push(Node::SimpleVarDecl {
                var: Var {
                    is_const: false,
                    name: Self::alive_name(&zig_name),
                    ty: Some(Box::new(Node::Identifier("bool".to_string()))),
                },
                expr: Some(Box::new(Node::Identifier("true".to_string()))),
            });
            nodes.push(self.conditional_defer(&zig_name));
        } else if needs_defer {
            let zig_name = self.rename_ident(&pi.ident);
            nodes.push(Node::Defer(Box::new(self.drop_call(&zig_name))));
        }
        nodes
    }

    fn var_of(&self, pi: &syn::PatIdent, force_var: bool) -> Var {
        let is_const = pi.mutability.is_none() && !force_var;
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
            stmts.extend(self.translate_stmt(stmt, is_last));
        }
        Node::Block(stmts)
    }

    pub fn translate_block_expr(&self, block: &syn::Block) -> Node {
        let mut stmts = Vec::new();
        let count = block.stmts.len();
        let mut result = Node::Identifier("void".to_string());
        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i + 1 == count;
            if is_last {
                if let syn::Stmt::Expr(expr, semi) = stmt {
                    if semi.is_none() {
                        stmts.extend(self.prelude_clear_flags(expr));
                        result = self.translate_expr(expr);
                        continue;
                    }
                }
            }
            stmts.extend(self.translate_stmt(stmt, false));
        }
        Node::BlockExpr {
            stmts,
            result: Box::new(result),
        }
    }
}
