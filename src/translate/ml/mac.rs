use crate::ast::ml::Expression;
use super::{apply_op, Translator};

impl Translator {
    pub fn translate_macro(&self, mac: &syn::Macro) -> Option<Expression> {
        if self.check_moniker(&mac.path, "std::macros::assert") {
            Some(self.translate_assert(mac))
        } else if self.check_moniker(&mac.path, "core::macros::assert_eq") {
            Some(self.translate_assert_eq(mac))
        } else {
            None
        }
    }

    fn translate_assert(&self, mac: &syn::Macro) -> Expression {
        let mut args = self.translate_macro_args(mac);
        let arg = args.remove(0);
        Expression::Assert(Box::new(arg))
    }

    fn translate_assert_eq(&self, mac: &syn::Macro) -> Expression {
        let args = self.translate_macro_args(mac);
        let equal = apply_op("=", args);
        Expression::Assert(Box::new(equal))
    }

    fn translate_macro_args(&self, mac: &syn::Macro) -> Vec<Expression> {
        use syn::parse::Parser;
        use syn::punctuated::Punctuated;
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        let macro_args = parser.parse2(mac.tokens.clone()).expect("failed to parse macro args");
        let mut args = vec![];
        for arg in &macro_args {
            let arg = self.translate_expr(arg);
            args.push(arg);
        }
        args
    }
}
