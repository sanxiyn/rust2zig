use std::fmt::Write;

use super::{PathMode, Rust2Zig, camel_to_snake, snake_to_camel};

impl Rust2Zig {
    pub fn translate_stmt(&mut self, stmt: &syn::Stmt, is_last: bool) {
        let pad = self.pad();
        match stmt {
            syn::Stmt::Expr(expr, semi) => {
                let no_trailing_semi = matches!(expr, syn::Expr::If(_) | syn::Expr::While(_));
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

    pub fn translate_expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Assign(ea) => self.translate_assign(ea),
            syn::Expr::Binary(eb) => self.translate_binary(eb),
            syn::Expr::Call(ec) => self.translate_call(ec),
            syn::Expr::Field(ef) => self.translate_field(ef),
            syn::Expr::If(ei) => self.translate_if(ei),
            syn::Expr::Lit(el) => self.translate_lit(el),
            syn::Expr::Macro(em) => {
                if !self.translate_macro(&em.mac) {
                    write!(self.out, "/* TODO: macro */").unwrap();
                }
            }
            syn::Expr::Match(em) => self.translate_match(em),
            syn::Expr::MethodCall(emc) => self.translate_method_call(emc),
            syn::Expr::Path(ep) => {
                let mode = self.path_mode(&ep.path);
                if matches!(mode, PathMode::EnumVariant) {
                    self.translate_path(&ep.path, mode);
                } else {
                    if self.check_moniker(&ep.path, "core::option::Option::None") {
                        write!(self.out, "null").unwrap();
                    } else {
                        self.translate_path(&ep.path, mode);
                    }
                }
            }
            syn::Expr::Struct(es) => self.translate_struct_expr(es),
            syn::Expr::Tuple(et) => self.translate_tuple(et),
            syn::Expr::While(ew) => self.translate_while(ew),
            _ => {
                write!(self.out, "/* TODO: expr */").unwrap();
            }
        }
    }

    fn translate_assign(&mut self, ea: &syn::ExprAssign) {
        self.translate_expr(&ea.left);
        write!(self.out, " = ").unwrap();
        self.translate_expr(&ea.right);
    }

    fn translate_binary(&mut self, eb: &syn::ExprBinary) {
        self.translate_expr(&eb.left);
        let op = self.translate_binop(&eb.op);
        write!(self.out, " {} ", op).unwrap();
        self.translate_expr(&eb.right);
    }

    fn translate_binop(&mut self, op: &syn::BinOp) -> &'static str {
        match op {
            syn::BinOp::Add(_) => "+",
            syn::BinOp::Sub(_) => "-",
            syn::BinOp::Mul(_) => "*",
            syn::BinOp::Div(_) => "/",
            syn::BinOp::Rem(_) => "%",
            syn::BinOp::Eq(_) => "==",
            syn::BinOp::Ne(_) => "!=",
            syn::BinOp::Lt(_) => "<",
            syn::BinOp::Le(_) => "<=",
            syn::BinOp::Gt(_) => ">",
            syn::BinOp::Ge(_) => ">=",
            _ => "/* TODO: binop */",
        }
    }

    fn translate_call(&mut self, ec: &syn::ExprCall) {
        if let syn::Expr::Path(ep) = &*ec.func {
            if matches!(self.path_mode(&ep.path), PathMode::EnumVariant) {
                let name = ep.path.segments.last().unwrap().ident.to_string();
                let variant = camel_to_snake(&name);
                write!(self.out, ".{{ .{} = ", variant).unwrap();
                for (i, arg) in ec.args.iter().enumerate() {
                    if i > 0 {
                        write!(self.out, ", ").unwrap();
                    }
                    self.translate_expr(arg);
                }
                write!(self.out, " }}").unwrap();
                return;
            }
            if self.check_moniker(&ep.path, "core::option::Option::Some") {
                self.translate_expr(&ec.args[0]);
                return;
            }
        }
        self.translate_expr(&ec.func);
        write!(self.out, "(").unwrap();
        for (i, arg) in ec.args.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            self.translate_expr(arg);
        }
        write!(self.out, ")").unwrap();
    }

    fn translate_field(&mut self, ef: &syn::ExprField) {
        self.translate_expr(&ef.base);
        match &ef.member {
            syn::Member::Named(ident) => write!(self.out, ".{}", ident).unwrap(),
            syn::Member::Unnamed(index) => write!(self.out, ".{}", index.index).unwrap(),
        }
    }

    fn translate_if(&mut self, ei: &syn::ExprIf) {
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

    fn translate_lit(&mut self, el: &syn::ExprLit) {
        match &el.lit {
            syn::Lit::Bool(b) => write!(self.out, "{}", b.value).unwrap(),
            syn::Lit::Int(i) => write!(self.out, "{}", i.base10_digits()).unwrap(),
            syn::Lit::Str(s) => write!(self.out, "\"{}\"", s.value()).unwrap(),
            _ => writeln!(self.out, "/* TODO: lit */").unwrap(),
        }
    }

    fn translate_macro(&mut self, mac: &syn::Macro) -> bool {
        if self.check_moniker(&mac.path, "std::macros::panic") {
            self.translate_panic(mac);
            true
        } else if self.check_moniker(&mac.path, "std::macros::println") {
            self.translate_println(mac);
            true
        } else {
            false
        }
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

    fn translate_match(&mut self, em: &syn::ExprMatch) {
        write!(self.out, "switch (").unwrap();
        self.translate_expr(&em.expr);
        write!(self.out, ") ").unwrap();
        self.translate_match_arms(&em.arms);
    }

    fn translate_match_arms(&mut self, arms: &[syn::Arm]) {
        writeln!(self.out, "{{").unwrap();
        self.indent();
        for arm in arms {
            let pad = self.pad();
            write!(self.out, "{}", pad).unwrap();
            let captures = self.translate_match_pat(&arm.pat);
            write!(self.out, " => ").unwrap();
            if !captures.is_empty() {
                write!(self.out, "|").unwrap();
                for (i, cap) in captures.iter().enumerate() {
                    if i > 0 {
                        write!(self.out, ", ").unwrap();
                    }
                    write!(self.out, "{}", cap).unwrap();
                }
                write!(self.out, "| ").unwrap();
            }
            self.translate_expr(&arm.body);
            writeln!(self.out, ",").unwrap();
        }
        self.dedent();
        write!(self.out, "{}}}", self.pad()).unwrap();
    }

    fn translate_method_call(&mut self, emc: &syn::ExprMethodCall) {
        self.translate_expr(&emc.receiver);
        write!(self.out, ".{}", snake_to_camel(&emc.method.to_string())).unwrap();
        write!(self.out, "(").unwrap();
        for (i, arg) in emc.args.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            self.translate_expr(arg);
        }
        write!(self.out, ")").unwrap();
    }

    fn translate_struct_expr(&mut self, es: &syn::ExprStruct) {
        self.translate_path(&es.path, PathMode::Normal);
        write!(self.out, "{{ ").unwrap();
        for (i, field) in es.fields.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            match &field.member {
                syn::Member::Named(ident) => write!(self.out, ".{} = ", ident).unwrap(),
                syn::Member::Unnamed(index) => write!(self.out, ".{} = ", index.index).unwrap(),
            }
            self.translate_expr(&field.expr);
        }
        write!(self.out, " }}").unwrap();
    }

    fn translate_tuple(&mut self, et: &syn::ExprTuple) {
        write!(self.out, ".{{ ").unwrap();
        for (i, elem) in et.elems.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            self.translate_expr(elem);
        }
        write!(self.out, " }}").unwrap();
    }

    fn translate_while(&mut self, ew: &syn::ExprWhile) {
        write!(self.out, "while (").unwrap();
        self.translate_expr(&ew.cond);
        write!(self.out, ") ").unwrap();
        self.translate_block(&ew.body);
    }
}
