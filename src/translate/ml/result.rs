use syn::visit::Visit;

use crate::ast::ml::{BindingOp, Expression, Pattern, RecFlag, ValueBinding};
use super::{qualified, Translator};

const LET_OP: &str = "let*";

impl Translator {
    pub fn translate_try_local(
        &self,
        pat: Pattern,
        et: &syn::ExprTry,
        body: Expression,
    ) -> Expression {
        let binding = BindingOp {
            op: LET_OP.to_string(),
            pat,
            exp: self.translate_expr(&et.expr),
        };
        Expression::LetOp(Box::new(binding), Box::new(body))
    }

    pub fn wrap_let_op(&self, block: &syn::Block, body: Expression) -> Expression {
        let Some(module) = self.let_op_module(block) else { return body };
        let binding = ValueBinding {
            pat: Pattern::Var(LET_OP.to_string()),
            params: Default::default(),
            expr: qualified(module, "bind"),
        };
        Expression::Let(RecFlag::Nonrecursive, vec![binding], Box::new(body))
    }

    fn let_op_module(&self, block: &syn::Block) -> Option<&'static str> {
        let mut finder = Finder { translator: self, module: None };
        finder.visit_block(block);
        finder.module
    }

    fn try_module(&self, et: &syn::ExprTry) -> Option<&'static str> {
        let range = et.question_token.spans[0].into();
        if self.check_moniker_at(&range, "core::option::branch") {
            return Some("Option");
        }
        if self.check_moniker_at(&range, "core::result::branch") {
            return Some("Result");
        }
        None
    }
}

struct Finder<'a> {
    translator: &'a Translator,
    module: Option<&'static str>,
}

impl<'ast> Visit<'ast> for Finder<'_> {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(init) = &local.init {
            if let syn::Expr::Try(et) = &*init.expr {
                if self.module.is_none() {
                    self.module = self.translator.try_module(et);
                }
            }
        }
        syn::visit::visit_local(self, local);
    }
}
