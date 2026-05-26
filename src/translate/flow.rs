use std::fmt::Write;

use super::Rust2Zig;

impl Rust2Zig {
    pub fn translate_break(&mut self, eb: &syn::ExprBreak) {
        if eb.label.is_some() || eb.expr.is_some() {
            write!(self.out, "/* TODO: break */").unwrap();
            return;
        }
        write!(self.out, "break").unwrap();
    }

    pub fn translate_continue(&mut self, ec: &syn::ExprContinue) {
        if ec.label.is_some() {
            write!(self.out, "/* TODO: continue */").unwrap();
            return;
        }
        write!(self.out, "continue").unwrap();
    }

    pub fn translate_for_loop(&mut self, efl: &syn::ExprForLoop) {
        if let syn::Expr::Range(er) = &*efl.expr {
            self.translate_for_range(efl, er);
            return;
        }
        if let syn::Expr::Call(ec) = &*efl.expr {
            if let syn::Expr::Path(ep) = &*ec.func {
                if self.check_moniker(&ep.path, "std::iter::zip") {
                    self.translate_for_zip(efl, ec);
                    return;
                }
            }
        }
        if let syn::Expr::MethodCall(emc) = &*efl.expr {
            if self.check_moniker_ident(&emc.method, "core::iter::Iterator::enumerate") {
                self.translate_for_enumerate(efl, emc);
                return;
            }
        }
        let Some(by_ref) = self.iter_by_ref(&efl.expr) else {
            write!(self.out, "/* TODO: for */").unwrap();
            return;
        };
        write!(self.out, "for (").unwrap();
        self.translate_expr(&efl.expr);
        write!(self.out, ") |{}", if by_ref { "*" } else { "" }).unwrap();
        self.translate_pat(&efl.pat);
        write!(self.out, "| ").unwrap();
        self.translate_block(&efl.body);
    }

    fn translate_for_enumerate(&mut self, efl: &syn::ExprForLoop, emc: &syn::ExprMethodCall) {
        let syn::Pat::Tuple(pt) = &*efl.pat else {
            write!(self.out, "/* TODO: for */").unwrap();
            return;
        };
        if pt.elems.len() != 2 {
            write!(self.out, "/* TODO: for */").unwrap();
            return;
        }
        let (base, by_ref) = self.peel_iter(&emc.receiver);
        write!(self.out, "for (").unwrap();
        self.translate_expr(base);
        write!(self.out, ", 0..) |").unwrap();
        if by_ref {
            write!(self.out, "*").unwrap();
        }
        self.translate_pat(&pt.elems[1]);
        write!(self.out, ", ").unwrap();
        self.translate_pat(&pt.elems[0]);
        write!(self.out, "| ").unwrap();
        self.translate_block(&efl.body);
    }

    fn translate_for_range(&mut self, efl: &syn::ExprForLoop, er: &syn::ExprRange) {
        let (Some(start), Some(end)) = (&er.start, &er.end) else {
            write!(self.out, "/* TODO: for */").unwrap();
            return;
        };
        let pi = if let syn::Pat::Ident(pi) = &*efl.pat { pi } else {
            write!(self.out, "/* TODO: for */").unwrap();
            return;
        };
        let name = self.rename_ident(&pi.ident);
        let ty = self.scip.type_at(&pi.ident.span().into());
        let mut ty_str = String::new();
        if let Some(ty) = &ty {
            let saved = std::mem::take(&mut self.out);
            self.translate_type(ty);
            ty_str = std::mem::replace(&mut self.out, saved);
        }
        let preamble = vec![if ty_str.is_empty() {
            format!("const {name} = _{name};")
        } else {
            format!("const {name}: {ty_str} = @intCast(_{name});")
        }];
        write!(self.out, "for (").unwrap();
        self.translate_expr(start);
        write!(self.out, "..").unwrap();
        if matches!(er.limits, syn::RangeLimits::Closed(_)) {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(n), .. }) = &**end {
                let v: u64 = n.base10_parse().unwrap();
                write!(self.out, "{}", v + 1).unwrap();
            } else {
                self.translate_expr(end);
                write!(self.out, " + 1").unwrap();
            }
        } else {
            self.translate_expr(end);
        }
        write!(self.out, ") |_{}| ", name).unwrap();
        self.translate_block_with_preamble(&efl.body, &preamble);
    }

    fn translate_for_zip(&mut self, efl: &syn::ExprForLoop, ec: &syn::ExprCall) {
        let syn::Pat::Tuple(pt) = &*efl.pat else {
            write!(self.out, "/* TODO: for */").unwrap();
            return;
        };
        if pt.elems.len() != ec.args.len() {
            write!(self.out, "/* TODO: for */").unwrap();
            return;
        }
        let by_refs: Vec<bool> = ec.args.iter().map(|arg| self.iter_by_ref(arg).unwrap_or(false)).collect();
        write!(self.out, "for (").unwrap();
        for (i, arg) in ec.args.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            self.translate_expr(arg);
        }
        write!(self.out, ") |").unwrap();
        for (i, elem) in pt.elems.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            if by_refs[i] {
                write!(self.out, "*").unwrap();
            }
            self.translate_pat(elem);
        }
        write!(self.out, "| ").unwrap();
        self.translate_block(&efl.body);
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

    pub fn translate_if(&mut self, ei: &syn::ExprIf) {
        if self.translate_if_option(ei) {
            return;
        }
        write!(self.out, "if (").unwrap();
        self.translate_expr(&ei.cond);
        write!(self.out, ") ").unwrap();
        self.translate_block(&ei.then_branch);
        if let Some((_, else_expr)) = &ei.else_branch {
            if let syn::Expr::Block(eb) = &**else_expr {
                write!(self.out, " else ").unwrap();
                self.translate_block(&eb.block);
            }
        }
    }

    fn translate_if_option(&mut self, ei: &syn::ExprIf) -> bool {
        let syn::Expr::Let(el) = &*ei.cond else { return false };
        let syn::Pat::TupleStruct(pts) = &*el.pat else { return false };
        if !self.check_moniker(&pts.path, "core::option::Option::Some") {
            return false;
        }
        write!(self.out, "if (").unwrap();
        self.translate_expr(&el.expr);
        write!(self.out, ") |").unwrap();
        self.translate_pat(&pts.elems[0]);
        write!(self.out, "| ").unwrap();
        self.translate_block(&ei.then_branch);
        if let Some((_, else_expr)) = &ei.else_branch {
            if let syn::Expr::Block(eb) = &**else_expr {
                write!(self.out, " else ").unwrap();
                self.translate_block(&eb.block);
            }
        }
        true
    }

    pub fn translate_while(&mut self, ew: &syn::ExprWhile) {
        write!(self.out, "while (").unwrap();
        self.translate_expr(&ew.cond);
        write!(self.out, ") ").unwrap();
        self.translate_block(&ew.body);
    }
}
