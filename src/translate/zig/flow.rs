use crate::ast::zig::{Capture, Node, Var};
use super::Translator;

impl Translator {
    pub fn translate_break(&self, eb: &syn::ExprBreak) -> Node {
        if eb.label.is_some() || eb.expr.is_some() {
            return Node::Todo("break".to_string());
        }
        Node::Break
    }

    pub fn translate_continue(&self, ec: &syn::ExprContinue) -> Node {
        if ec.label.is_some() {
            return Node::Todo("continue".to_string());
        }
        Node::Continue
    }

    pub fn translate_for_loop(&self, efl: &syn::ExprForLoop) -> Node {
        if let syn::Expr::Range(er) = &*efl.expr {
            return self.translate_for_range(efl, er);
        }
        if let syn::Expr::Call(ec) = &*efl.expr {
            if let syn::Expr::Path(ep) = &*ec.func {
                if self.check_moniker(&ep.path, "std::iter::zip") {
                    return self.translate_for_zip(efl, ec);
                }
            }
        }
        if let syn::Expr::MethodCall(emc) = &*efl.expr {
            if self.check_moniker_ident(&emc.method, "core::iter::Iterator::enumerate") {
                return self.translate_for_enumerate(efl, emc);
            }
        }
        let Some(by_ref) = self.iter_by_ref(&efl.expr) else {
            return Node::Todo("for".to_string());
        };
        let iterable = self.translate_expr(&efl.expr);
        let name = self.pat_name(&efl.pat);
        let capture = Capture { name, by_ref };
        let body = self.translate_block(&efl.body);
        Node::For {
            iterables: vec![iterable],
            captures: vec![capture],
            body: Box::new(body),
        }
    }

    fn translate_for_enumerate(&self, efl: &syn::ExprForLoop, emc: &syn::ExprMethodCall) -> Node {
        let syn::Pat::Tuple(pt) = &*efl.pat else {
            return Node::Todo("for".to_string());
        };
        if pt.elems.len() != 2 {
            return Node::Todo("for".to_string());
        }
        let (base, by_ref) = self.peel_iter(&emc.receiver);
        let iterable = self.translate_expr(base);
        let counter = Node::ForRange(Box::new(Node::NumberLiteral("0".to_string())), None);
        let name = self.pat_name(&pt.elems[1]);
        let counter_name = self.pat_name(&pt.elems[0]);
        let capture = Capture { name, by_ref };
        let counter_capture = Capture { name: counter_name, by_ref: false };
        let body = self.translate_block(&efl.body);
        Node::For {
            iterables: vec![iterable, counter],
            captures: vec![capture, counter_capture],
            body: Box::new(body),
        }
    }

    fn translate_for_range(&self, efl: &syn::ExprForLoop, er: &syn::ExprRange) -> Node {
        let (Some(start), Some(end)) = (&er.start, &er.end) else {
            return Node::Todo("for".to_string());
        };
        let syn::Pat::Ident(pi) = &*efl.pat else {
            return Node::Todo("for".to_string());
        };
        let name = self.rename_ident(&pi.ident);
        let ty = self.scip.type_at(&pi.ident.span().into());
        let preamble = match ty {
            Some(ty) => Node::SimpleVarDecl {
                var: Var { is_const: true, name: name.clone(), ty: Some(Box::new(self.translate_type(&ty))) },
                expr: Some(Box::new(Node::BuiltinCall(
                    "intCast".to_string(),
                    vec![Node::Identifier(format!("_{name}"))],
                ))),
            },
            None => Node::SimpleVarDecl {
                var: Var { is_const: true, name: name.clone(), ty: None },
                expr: Some(Box::new(Node::Identifier(format!("_{name}")))),
            },
        };
        let start = self.translate_expr(start);
        let end = if matches!(er.limits, syn::RangeLimits::Closed(_)) {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(n), .. }) = &**end {
                let v: u64 = n.base10_parse().unwrap();
                Node::NumberLiteral((v + 1).to_string())
            } else {
                let left = self.translate_expr(end);
                let right = Node::NumberLiteral("1".to_string());
                Node::Add(Box::new(left), Box::new(right))
            }
        } else {
            self.translate_expr(end)
        };
        let iterable = Node::ForRange(Box::new(start), Some(Box::new(end)));
        let capture = Capture { name: format!("_{name}"), by_ref: false };
        let body = self.translate_block_with_preamble(&efl.body, vec![preamble]);
        Node::For {
            iterables: vec![iterable],
            captures: vec![capture],
            body: Box::new(body),
        }
    }

    fn translate_for_zip(&self, efl: &syn::ExprForLoop, ec: &syn::ExprCall) -> Node {
        let syn::Pat::Tuple(pt) = &*efl.pat else {
            return Node::Todo("for".to_string());
        };
        if pt.elems.len() != ec.args.len() {
            return Node::Todo("for".to_string());
        }
        let mut iterables = vec![];
        for arg in ec.args.iter() {
            let iterable = self.translate_expr(arg);
            iterables.push(iterable);
        }
        let mut captures = vec![];
        for (elem, arg) in pt.elems.iter().zip(ec.args.iter()) {
            let name = self.pat_name(elem);
            let by_ref = self.iter_by_ref(arg).unwrap_or(false);
            let capture = Capture { name, by_ref };
            captures.push(capture);
        }
        let body = self.translate_block(&efl.body);
        Node::For {
            iterables,
            captures,
            body: Box::new(body),
        }
    }

    fn peel_iter<'a>(&self, expr: &'a syn::Expr) -> (&'a syn::Expr, bool) {
        if let syn::Expr::MethodCall(emc) = expr {
            if self.check_moniker_ident(&emc.method, "core::slice::iter") {
                return (&emc.receiver, true);
            }
        }
        (expr, self.iter_by_ref(expr).unwrap_or(false))
    }

    fn iter_by_ref(&self, expr: &syn::Expr) -> Option<bool> {
        let syn::Expr::Path(ep) = expr else { return None };
        let ident = &ep.path.segments.last().unwrap().ident;
        match self.scip.type_at(&ident.span().into()) {
            Some(syn::Type::Array(_)) => Some(false),
            Some(syn::Type::Reference(tr)) if matches!(*tr.elem, syn::Type::Slice(_)) => Some(true),
            _ => None,
        }
    }

    pub fn translate_if(&self, ei: &syn::ExprIf) -> Node {
        if let Some(node) = self.translate_if_option(ei) {
            return node;
        }
        let cond = self.translate_expr(&ei.cond);
        let then_branch = self.translate_block(&ei.then_branch);
        let else_branch = if let Some((_, else_expr)) = &ei.else_branch {
            if let syn::Expr::Block(eb) = &**else_expr {
                Some(Box::new(self.translate_block(&eb.block)))
            } else {
                None
            }
        } else {
            None
        };
        Node::If {
            cond: Box::new(cond),
            capture: None,
            then_branch: Box::new(then_branch),
            else_branch: else_branch,
        }
    }

    fn translate_if_option(&self, ei: &syn::ExprIf) -> Option<Node> {
        let syn::Expr::Let(el) = &*ei.cond else { return None };
        let syn::Pat::TupleStruct(pts) = &*el.pat else { return None };
        if !self.check_moniker(&pts.path, "core::option::Option::Some") {
            return None;
        }
        let cond = self.translate_expr(&el.expr);
        let capture = self.pat_name(&pts.elems[0]);
        let then_branch = self.translate_block(&ei.then_branch);
        let else_branch = if let Some((_, else_expr)) = &ei.else_branch {
            if let syn::Expr::Block(eb) = &**else_expr {
                Some(Box::new(self.translate_block(&eb.block)))
            } else {
                None
            }
        } else {
            None
        };
        Some(Node::If {
            cond: Box::new(cond),
            capture: Some(capture),
            then_branch: Box::new(then_branch),
            else_branch: else_branch,
        })
    }

    pub fn translate_while(&self, ew: &syn::ExprWhile) -> Node {
        let cond = self.translate_expr(&ew.cond);
        let body = self.translate_block(&ew.body);
        Node::While {
            cond: Box::new(cond),
            body: Box::new(body),
        }
    }
}
