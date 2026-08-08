use crate::ast::ml::{Constant, Expression};
use super::{apply, Translator};

fn failwith(message: &str) -> Expression {
    let message = Expression::Constant(Constant::String(message.to_string()));
    apply("failwith", vec![message])
}

impl Translator {
    pub fn translate_macro(&self, mac: &syn::Macro) -> Option<Expression> {
        if self.check_moniker(&mac.path, "std::macros::assert") {
            Some(self.translate_assert(mac))
        } else if self.check_moniker(&mac.path, "core::macros::assert_eq") {
            Some(self.translate_assert_eq(mac))
        } else if self.check_moniker(&mac.path, "std::macros::panic") {
            self.translate_panic(mac)
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
        let equal = apply("=", args);
        Expression::Assert(Box::new(equal))
    }

    fn translate_panic(&self, mac: &syn::Macro) -> Option<Expression> {
        if mac.tokens.is_empty() {
            return Some(failwith("panic"));
        }
        let message: syn::LitStr = syn::parse2(mac.tokens.clone()).ok()?;
        Some(failwith(&message.value()))
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
