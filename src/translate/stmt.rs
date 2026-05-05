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
                let mutability = match &local.pat {
                    syn::Pat::Ident(pi) => pi.mutability.is_some(),
                    syn::Pat::Type(pt) => {
                        if let syn::Pat::Ident(pi) = &*pt.pat {
                            pi.mutability.is_some()
                        } else {
                            false
                        }
                    }
                    _ => false,
                };
                let keyword = if mutability { "var" } else { "const" };
                write!(self.out, "{}{} ", pad, keyword).unwrap();
                if let (syn::Pat::Type(pt), Some(init)) = (&local.pat, &local.init) {
                    if let (syn::Type::Array(ta), syn::Expr::Array(ea)) = (&*pt.ty, &*init.expr) {
                        if matches!(ta.len, syn::Expr::Infer(_)) {
                            self.translate_pat(&pt.pat);
                            write!(self.out, ": [{}]", ea.elems.len()).unwrap();
                            self.translate_type(&ta.elem);
                            write!(self.out, " = ").unwrap();
                            self.translate_expr(&init.expr);
                            writeln!(self.out, ";").unwrap();
                            return;
                        }
                    }
                }
                self.translate_pat(&local.pat);
                if let Some(init) = &local.init {
                    write!(self.out, " = ").unwrap();
                    self.translate_expr(&init.expr);
                }
                writeln!(self.out, ";").unwrap();
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

    pub fn translate_block(&mut self, block: &syn::Block) {
        self.translate_block_with_mut_params(block, &[]);
    }

    pub fn translate_block_with_mut_params(&mut self, block: &syn::Block, mut_params: &[String]) {
        writeln!(self.out, "{{").unwrap();
        self.indent();
        for name in mut_params {
            let pad = self.pad();
            writeln!(self.out, "{}var {name} = _{name};", pad, name = name).unwrap();
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
