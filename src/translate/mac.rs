use std::fmt::Write;

use super::Rust2Zig;

impl Rust2Zig {
    pub fn translate_macro(&mut self, mac: &syn::Macro) -> bool {
        if self.check_moniker(&mac.path, "core::macros::assert_eq") {
            self.translate_assert_eq(mac);
            true
        } else if self.check_moniker(&mac.path, "std::macros::panic") {
            self.translate_panic(mac);
            true
        } else if self.check_moniker(&mac.path, "std::macros::println") {
            self.translate_println(mac);
            true
        } else {
            false
        }
    }

    fn translate_assert_eq(&mut self, mac: &syn::Macro) {
        use syn::parse::Parser;
        use syn::punctuated::Punctuated;
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        let args = parser.parse2(mac.tokens.clone()).expect("failed to parse assert_eq args");
        write!(self.out, "try std.testing.expectEqual(").unwrap();
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            self.translate_expr(arg);
        }
        write!(self.out, ")").unwrap();
    }

    fn translate_panic(&mut self, mac: &syn::Macro) {
        let tokens = mac.tokens.to_string();
        if let Some(rest) = tokens.strip_prefix('"') {
            if let Some(end) = rest.find('"') {
                let message = &rest[..end];
                write!(self.out, "@panic(\"{}\")", message).unwrap();
            }
        }
    }

    fn translate_println(&mut self, mac: &syn::Macro) {
        use syn::parse::Parser;
        use syn::punctuated::Punctuated;
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        let args = parser.parse2(mac.tokens.clone()).expect("failed to parse println args");
        let mut iter = args.iter();
        let format = match iter.next() {
            Some(syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. })) => s.value(),
            _ => return,
        };
        let rest: Vec<&syn::Expr> = iter.collect();
        if rest.is_empty() {
            if format.is_empty() {
                write!(self.out, "std.debug.print(\"\\n\", .{{}})").unwrap();
            } else {
                write!(self.out, "std.debug.print(\"{}\\n\", .{{}})", format).unwrap();
            }
        } else {
            let sep = if rest.len() > 1 { " " } else { "" };
            write!(self.out, "std.debug.print(\"{}\\n\", .{{{}", format, sep).unwrap();
            for (i, arg) in rest.iter().enumerate() {
                if i > 0 {
                    write!(self.out, ", ").unwrap();
                }
                self.translate_expr(arg);
            }
            write!(self.out, "{}}})", sep).unwrap();
        }
    }
}
