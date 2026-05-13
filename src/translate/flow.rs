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

    pub fn translate_for_loop(&mut self, efl: &syn::ExprForLoop) {
        if let syn::Expr::Range(er) = &*efl.expr {
            let (Some(start), Some(end)) = (&er.start, &er.end) else {
                write!(self.out, "/* TODO: for */").unwrap();
                return;
            };
            let pi = if let syn::Pat::Ident(pi) = &*efl.pat { pi } else {
                write!(self.out, "/* TODO: for */").unwrap();
                return;
            };
            let name = pi.ident.to_string();
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
            return;
        }
        let by_ref = if let syn::Expr::Path(ep) = &*efl.expr {
            let ident = &ep.path.segments.last().unwrap().ident;
            match self.scip.type_at(&ident.span().into()) {
                Some(syn::Type::Array(_)) => Some(false),
                Some(syn::Type::Reference(tr)) if matches!(*tr.elem, syn::Type::Slice(_)) => {
                    Some(true)
                }
                _ => None,
            }
        } else {
            None
        };
        let Some(by_ref) = by_ref else {
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

    pub fn translate_if(&mut self, ei: &syn::ExprIf) {
        if let syn::Expr::Let(el) = &*ei.cond {
            if let syn::Pat::TupleStruct(pts) = &*el.pat {
                if self.check_moniker(&pts.path, "core::option::Option::Some") {
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
                    return;
                }
            }
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

    pub fn translate_while(&mut self, ew: &syn::ExprWhile) {
        write!(self.out, "while (").unwrap();
        self.translate_expr(&ew.cond);
        write!(self.out, ") ").unwrap();
        self.translate_block(&ew.body);
    }
}
