use syn::visit::Visit;

use crate::ast::ml::{Case, Constant, Direction, Expression, Longident, Pattern};
use super::{Translator, apply, qualified, todo, unit};

#[derive(Clone, Copy, PartialEq)]
enum Jump {
    Break,
    Continue,
}

fn block_has_jump(block: &syn::Block, jump: Jump) -> bool {
    struct Finder {
        jump: Jump,
        found: bool,
    }
    impl<'ast> Visit<'ast> for Finder {
        fn visit_expr_break(&mut self, _: &'ast syn::ExprBreak) {
            self.found |= self.jump == Jump::Break;
        }
        fn visit_expr_continue(&mut self, _: &'ast syn::ExprContinue) {
            self.found |= self.jump == Jump::Continue;
        }
        fn visit_expr_for_loop(&mut self, _: &'ast syn::ExprForLoop) {}
        fn visit_expr_loop(&mut self, _: &'ast syn::ExprLoop) {}
        fn visit_expr_while(&mut self, _: &'ast syn::ExprWhile) {}
        fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
    }
    let mut finder = Finder { jump, found: false };
    finder.visit_block(block);
    finder.found
}

impl Translator {
    pub fn translate_for_loop(&self, efl: &syn::ExprForLoop) -> Expression {
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
        if self.is_array_like(&efl.expr) {
            return self.translate_for_each(efl);
        }
        todo("for")
    }

    fn translate_for_each(&self, efl: &syn::ExprForLoop) -> Expression {
        let pat = self.translate_pat(&efl.pat);
        let iterable = self.translate_expr(&efl.expr);
        let body = self.loop_body(&efl.body);
        let func = Expression::Function(vec![pat], Box::new(body));
        let node = Expression::Apply(Box::new(qualified("Array", "iter")), vec![func, iterable]);
        self.wrap_break(&efl.body, node)
    }

    fn translate_for_enumerate(&self, efl: &syn::ExprForLoop, emc: &syn::ExprMethodCall) -> Expression {
        let syn::Pat::Tuple(pt) = &*efl.pat else {
            return todo("for");
        };
        if pt.elems.len() != 2 {
            return todo("for");
        }
        let params = pt.elems.iter().map(|elem| self.translate_pat(elem)).collect();
        let iterable = self.translate_expr(self.peel_iter(&emc.receiver));
        let body = self.loop_body(&efl.body);
        let func = Expression::Function(params, Box::new(body));
        let node = Expression::Apply(Box::new(qualified("Array", "iteri")), vec![func, iterable]);
        self.wrap_break(&efl.body, node)
    }

    fn translate_for_range(&self, efl: &syn::ExprForLoop, er: &syn::ExprRange) -> Expression {
        let (Some(start), Some(end)) = (&er.start, &er.end) else {
            return todo("for");
        };
        let pat = self.translate_pat(&efl.pat);
        let start = self.translate_expr(start);
        let end = if matches!(er.limits, syn::RangeLimits::Closed(_)) {
            self.translate_expr(end)
        } else if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(n), .. }) = &**end {
            let v: u64 = n.base10_parse().unwrap();
            if v >= 1 {
                Expression::Constant(Constant::Integer((v - 1).to_string(), None))
            } else {
                apply("-", vec![self.translate_expr(end), int(1)])
            }
        } else {
            apply("-", vec![self.translate_expr(end), int(1)])
        };
        let body = self.loop_body(&efl.body);
        let node = Expression::For(Box::new(pat), Box::new(start), Box::new(end), Direction::Upto, Box::new(body));
        self.wrap_break(&efl.body, node)
    }

    fn translate_for_zip(&self, efl: &syn::ExprForLoop, ec: &syn::ExprCall) -> Expression {
        let syn::Pat::Tuple(pt) = &*efl.pat else {
            return todo("for");
        };
        if pt.elems.len() != 2 || ec.args.len() != 2 {
            return todo("for");
        }
        let mut params = vec![];
        for elem in &pt.elems {
            let param = self.translate_pat(elem);
            params.push(param);
        }
        let body = self.loop_body(&efl.body);
        let func = Expression::Function(params, Box::new(body));
        let a = self.translate_expr(&ec.args[0]);
        let b = self.translate_expr(&ec.args[1]);
        let node = Expression::Apply(Box::new(qualified("Array", "iter2")), vec![func, a, b]);
        self.wrap_break(&efl.body, node)
    }

    fn loop_body(&self, block: &syn::Block) -> Expression {
        let body = self.translate_block(block);
        if !block_has_jump(block, Jump::Continue) {
            return body;
        }
        Expression::Try(Box::new(body), vec![handler("Continue")])
    }

    fn wrap_break(&self, block: &syn::Block, node: Expression) -> Expression {
        if !block_has_jump(block, Jump::Break) {
            return node;
        }
        Expression::Try(Box::new(node), vec![handler("Exit")])
    }

    fn peel_iter<'a>(&self, expr: &'a syn::Expr) -> &'a syn::Expr {
        if let syn::Expr::MethodCall(emc) = expr {
            if self.check_moniker_ident(&emc.method, "core::slice::iter") {
                return &emc.receiver;
            }
        }
        expr
    }

    fn is_array_like(&self, expr: &syn::Expr) -> bool {
        let syn::Expr::Path(ep) = expr else { return false };
        let ident = &ep.path.segments.last().unwrap().ident;
        match self.scip.type_at(&ident.span().into()) {
            Some(syn::Type::Array(_)) => true,
            Some(syn::Type::Reference(tr)) => matches!(*tr.elem, syn::Type::Slice(_)),
            _ => false,
        }
    }

    pub fn translate_if(&self, ei: &syn::ExprIf) -> Expression {
        if let Some(expr) = self.translate_if_option(ei) {
            return expr;
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
        Expression::IfThenElse(Box::new(cond), Box::new(then_branch), else_branch)
    }

    fn translate_if_option(&self, ei: &syn::ExprIf) -> Option<Expression> {
        let syn::Expr::Let(el) = &*ei.cond else { return None };
        let syn::Pat::TupleStruct(pts) = &*el.pat else { return None };
        if !self.check_moniker(&pts.path, "core::option::Option::Some") {
            return None;
        }
        let syn::Pat::Ident(pi) = &pts.elems[0] else { return None };
        let bind = pi.ident.to_string();
        let scrutinee = self.translate_expr(&el.expr);
        let then_branch = self.translate_block(&ei.then_branch);
        let else_branch = if let Some((_, else_expr)) = &ei.else_branch {
            if let syn::Expr::Block(eb) = &**else_expr {
                self.translate_block(&eb.block)
            } else {
                unit()
            }
        } else {
            unit()
        };
        let some = Case {
            lhs: Pattern::Construct(Longident::Lident("Some".to_string()), Some(Box::new(Pattern::Var(bind)))),
            guard: None,
            rhs: then_branch,
        };
        let none = Case {
            lhs: Pattern::Construct(Longident::Lident("None".to_string()), None),
            guard: None,
            rhs: else_branch,
        };
        Some(Expression::Match(Box::new(scrutinee), vec![some, none]))
    }

    pub fn translate_loop(&self, el: &syn::ExprLoop) -> Expression {
        if el.label.is_some() {
            return todo("loop");
        }
        let cond = Expression::Construct(Longident::Lident("true".to_string()), None);
        let body = self.loop_body(&el.body);
        let node = Expression::While(Box::new(cond), Box::new(body));
        if block_has_jump(&el.body, Jump::Break) {
            return self.wrap_break(&el.body, node);
        }
        let unreachable = Expression::Assert(Box::new(
            Expression::Construct(Longident::Lident("false".to_string()), None),
        ));
        self.wrap_break(&el.body, Expression::Sequence(Box::new(node), Box::new(unreachable)))
    }

    pub fn translate_while(&self, ew: &syn::ExprWhile) -> Expression {
        let cond = self.translate_expr(&ew.cond);
        let body = self.loop_body(&ew.body);
        let node = Expression::While(Box::new(cond), Box::new(body));
        self.wrap_break(&ew.body, node)
    }
}

fn handler(name: &str) -> Case {
    Case {
        lhs: Pattern::Construct(Longident::Lident(name.to_string()), None),
        guard: None,
        rhs: unit(),
    }
}

fn int(n: u64) -> Expression {
    Expression::Constant(Constant::Integer(n.to_string(), None))
}
