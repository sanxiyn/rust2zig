use std::fmt::Write;

use super::Rust2Zig;

impl Rust2Zig {
    pub fn translate_stmt(&mut self, stmt: &syn::Stmt, is_last: bool) {
        let pad = self.pad();
        match stmt {
            syn::Stmt::Expr(expr, semi) => {
                let no_trailing_semi = matches!(
                    expr,
                    syn::Expr::ForLoop(_) | syn::Expr::If(_) | syn::Expr::While(_)
                );
                if is_last && semi.is_none() && !matches!(expr, syn::Expr::If(_)) {
                    write!(self.out, "{}return ", pad).unwrap();
                    self.translate_expr(expr);
                    writeln!(self.out, ";").unwrap();
                } else if no_trailing_semi {
                    write!(self.out, "{}", pad).unwrap();
                    self.translate_expr(expr);
                    writeln!(self.out).unwrap();
                } else {
                    write!(self.out, "{}", pad).unwrap();
                    self.translate_expr(expr);
                    writeln!(self.out, ";").unwrap();
                }
            }
            syn::Stmt::Local(local) => {
                if let syn::Pat::Tuple(pt) = &local.pat {
                    write!(self.out, "{}", pad).unwrap();
                    for (i, elem) in pt.elems.iter().enumerate() {
                        if i > 0 {
                            write!(self.out, ", ").unwrap();
                        }
                        write!(self.out, "const ").unwrap();
                        self.translate_pat(elem);
                    }
                    if let Some(init) = &local.init {
                        write!(self.out, " = ").unwrap();
                        self.translate_expr(&init.expr);
                    }
                    writeln!(self.out, ";").unwrap();
                    return;
                }
                let binding = match &local.pat {
                    syn::Pat::Ident(pi) => Some(pi),
                    syn::Pat::Type(pt) => {
                        if let syn::Pat::Ident(pi) = &*pt.pat {
                            Some(pi)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(pi) = binding {
                    if let Some(init) = &local.init {
                        if let syn::Expr::Closure(ec) = &*init.expr {
                            self.translate_closure_local(pi, ec);
                            return;
                        }
                    }
                    let keyword = if pi.mutability.is_some() { "var" } else { "const" };
                    write!(self.out, "{}{} {}", pad, keyword, pi.ident).unwrap();
                    if let Some(ty) = self.scip.type_at(&pi.ident.span().into()) {
                        write!(self.out, ": ").unwrap();
                        self.translate_type(&ty);
                    }
                    if let Some(init) = &local.init {
                        write!(self.out, " = ").unwrap();
                        self.translate_expr(&init.expr);
                    }
                    writeln!(self.out, ";").unwrap();
                } else {
                    write!(self.out, "{}const ", pad).unwrap();
                    self.translate_pat(&local.pat);
                    if let Some(init) = &local.init {
                        write!(self.out, " = ").unwrap();
                        self.translate_expr(&init.expr);
                    }
                    writeln!(self.out, ";").unwrap();
                }
            }
            syn::Stmt::Macro(sm) => {
                write!(self.out, "{}", pad).unwrap();
                if self.translate_macro(&sm.mac) {
                    writeln!(self.out, ";").unwrap();
                } else {
                    writeln!(self.out, "// TODO: macro").unwrap();
                }
            }
            _ => {
                writeln!(self.out, "{}// TODO: stmt", pad).unwrap();
            }
        }
    }

    fn translate_closure_local(&mut self, pi: &syn::PatIdent, ec: &syn::ExprClosure) {
        let pad = self.pad();
        if self.has_capture(ec) {
            writeln!(
                self.out,
                "{}const {} = /* TODO: closure */;",
                pad, pi.ident
            )
            .unwrap();
            return;
        }
        writeln!(self.out, "{}const {} = struct {{", pad, pi.ident).unwrap();
        self.indent();
        write!(self.out, "{}fn call(", self.pad()).unwrap();
        for (i, input) in ec.inputs.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            if let syn::Pat::Ident(pi) = input {
                write!(self.out, "{}", pi.ident).unwrap();
                if let Some(ty) = self.scip.type_at(&pi.ident.span().into()) {
                    write!(self.out, ": ").unwrap();
                    self.translate_type(&ty);
                }
            } else {
                self.translate_pat(input);
            }
        }
        write!(self.out, ") ").unwrap();
        if let Some(ret) = self.closure_return_type(&pi.ident) {
            self.translate_type(&ret);
        } else {
            write!(self.out, "void").unwrap();
        }
        write!(self.out, " ").unwrap();
        if let syn::Expr::Block(eb) = &*ec.body {
            self.translate_block(&eb.block);
            writeln!(self.out).unwrap();
        } else {
            writeln!(self.out, "{{").unwrap();
            self.indent();
            write!(self.out, "{}return ", self.pad()).unwrap();
            self.translate_expr(&ec.body);
            writeln!(self.out, ";").unwrap();
            self.dedent();
            writeln!(self.out, "{}}}", self.pad()).unwrap();
        }
        self.dedent();
        writeln!(self.out, "{}}}.call;", pad).unwrap();
    }

    pub fn translate_block(&mut self, block: &syn::Block) {
        self.translate_block_with_preamble(block, &[]);
    }

    pub fn translate_block_with_preamble(&mut self, block: &syn::Block, preamble: &[String]) {
        writeln!(self.out, "{{").unwrap();
        self.indent();
        for line in preamble {
            let pad = self.pad();
            writeln!(self.out, "{}{}", pad, line).unwrap();
        }
        let stmts = &block.stmts;
        for (i, stmt) in stmts.iter().enumerate() {
            let is_last = i == stmts.len() - 1;
            self.translate_stmt(stmt, is_last);
        }
        self.dedent();
        write!(self.out, "{}}}", self.pad()).unwrap();
    }
}
