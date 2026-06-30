use crate::ast::zig::Node;
use super::Translator;

impl Translator {
    pub fn translate_macro(&self, mac: &syn::Macro) -> Option<Node> {
        if self.check_moniker(&mac.path, "core::macros::assert_eq") {
            Some(self.translate_assert_eq(mac))
        } else if self.check_moniker(&mac.path, "std::macros::panic") {
            Some(self.translate_panic(mac))
        } else if self.check_moniker(&mac.path, "std::macros::println") {
            Some(self.translate_println(mac))
        } else {
            None
        }
    }

    fn translate_assert_eq(&self, mac: &syn::Macro) -> Node {
        use syn::parse::Parser;
        use syn::punctuated::Punctuated;
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        let macro_args = parser.parse2(mac.tokens.clone()).expect("failed to parse assert_eq args");
        let func = dotted_name("std.testing.expectEqual");
        let mut args = vec![];
        for arg in &macro_args {
            let arg = self.translate_expr(arg);
            args.push(arg);
        }
        let call = Node::Call(Box::new(func), args);
        Node::Try(Box::new(call))
    }

    fn translate_panic(&self, mac: &syn::Macro) -> Node {
        let tokens = mac.tokens.to_string();
        let message = tokens.strip_prefix('"')
            .and_then(|rest| rest.find('"').map(|end| rest[..end].to_string()))
            .unwrap_or_default();
        Node::BuiltinCall(
            "panic".to_string(),
            vec![Node::StringLiteral(message)],
        )
    }

    fn translate_println(&self, _mac: &syn::Macro) -> Node {
        Node::Todo("println".to_string())
    }
}

fn dotted_name(name: &str) -> Node {
    let mut parts = name.split('.');
    let mut node = Node::Identifier(parts.next().unwrap().to_string());
    while let Some(part) = parts.next() {
        node = Node::FieldAccess(Box::new(node), part.to_string());
    }
    node
}
