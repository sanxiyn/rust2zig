use syn::visit::Visit;

use crate::ast::ml::{Expression, Longident, Pattern, RecFlag, ValueBinding};
use crate::translate::name::escape_ml;
use super::{Translator, apply, todo, unit};

fn binding_ident(pat: &syn::Pat) -> Option<&syn::Ident> {
    match pat {
        syn::Pat::Ident(pi) => Some(&pi.ident),
        syn::Pat::Type(pt) => binding_ident(&pt.pat),
        _ => None,
    }
}

fn try_init(local: &syn::Local) -> Option<&syn::ExprTry> {
    let init = local.init.as_ref()?;
    match &*init.expr {
        syn::Expr::Try(et) => Some(et),
        _ => None,
    }
}

impl Translator {
    pub fn translate_block(&self, block: &syn::Block) -> Expression {
        self.translate_stmts(&block.stmts)
    }

    fn translate_stmts(&self, stmts: &[syn::Stmt]) -> Expression {
        let Some((first, rest)) = stmts.split_first() else {
            return unit();
        };
        match first {
            syn::Stmt::Expr(expr, _) => {
                let return_type = match expr {
                    syn::Expr::MethodCall(emc) => self.scip.return_type_at(&emc.method.span().into()),
                    _ => None,
                };
                let expr = if return_type.is_some() {
                    let expr = self.translate_expr(expr);
                    apply("ignore", vec![expr])
                } else {
                    self.translate_expr(expr)
                };
                self.sequence(expr, rest)
            }
            syn::Stmt::Local(local) if try_init(local).is_some() => {
                let pat = self.translate_pat(&local.pat);
                let body = self.translate_stmts(rest);
                self.translate_try_local(pat, try_init(local).unwrap(), body)
            }
            syn::Stmt::Local(local) => {
                let binding = self.translate_local(local);
                let mut body = self.translate_stmts(rest);
                let upgrades = self.ref_upgrade_bindings(&local.pat);
                if !upgrades.is_empty() {
                    body = Expression::Let(RecFlag::Nonrecursive, upgrades, Box::new(body));
                }
                Expression::Let(RecFlag::Nonrecursive, vec![binding], Box::new(body))
            }
            syn::Stmt::Macro(sm) => {
                let node = self.translate_macro(&sm.mac)
                    .unwrap_or_else(|| todo("macro"));
                self.sequence(node, rest)
            }
            _ => todo("stmt"),
        }
    }

    fn sequence(&self, expr: Expression, rest: &[syn::Stmt]) -> Expression {
        if rest.is_empty() {
            expr
        } else {
            Expression::Sequence(Box::new(expr), Box::new(self.translate_stmts(rest)))
        }
    }

    fn translate_local(&self, local: &syn::Local) -> ValueBinding {
        let pat = self.translate_pat(&local.pat);
        let init = &local.init.as_ref().unwrap().expr;
        if let syn::Expr::Closure(ec) = &**init {
            let (params, expr) = self.closure_parts(ec);
            return ValueBinding { pat, params, expr };
        }
        let init = match binding_ident(&local.pat).and_then(|ident| self.declared_repr(ident)) {
            Some(repr) => self.translate_int_operand(init, repr),
            None => self.translate_expr(init),
        };
        let expr = self.wrap_ref_init(&local.pat, init);
        ValueBinding { pat, params: Default::default(), expr }
    }

    fn ref_upgrade_bindings(&self, pat: &syn::Pat) -> Vec<ValueBinding> {
        struct Collect<'a> {
            translator: &'a Translator,
            upgrades: Vec<ValueBinding>,
        }

        impl<'ast> Visit<'ast> for Collect<'_> {
            fn visit_pat_ident(&mut self, pi: &'ast syn::PatIdent) {
                if self.translator.is_ref_binding(&pi.ident) {
                    let name = escape_ml(&pi.ident.to_string());
                    self.upgrades.push(ValueBinding {
                        pat: Pattern::Var(name.clone()),
                        params: Vec::new(),
                        expr: apply("ref", vec![Expression::Ident(Longident::Lident(name))]),
                    });
                }
                syn::visit::visit_pat_ident(self, pi);
            }
        }

        let mut collector = Collect {
            translator: self,
            upgrades: Default::default(),
        };
        match pat {
            syn::Pat::Ident(pi) => {
                if let Some((_, subpat)) = &pi.subpat {
                    collector.visit_pat(subpat);
                }
            }
            _ => collector.visit_pat(pat),
        }
        collector.upgrades
    }

    fn wrap_ref_init(&self, pat: &syn::Pat, expr: Expression) -> Expression {
        if let syn::Pat::Ident(pi) = pat && self.is_ref_binding(&pi.ident) {
            return apply("ref", vec![expr]);
        }
        expr
    }

    fn is_ref_binding(&self, ident: &syn::Ident) -> bool {
        let Some(symbol) = self.scip.symbol_at(&ident.span().into()) else { return false };
        self.ref_vars.contains(symbol)
    }
}
