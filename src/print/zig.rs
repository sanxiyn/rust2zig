use crate::ast::zig::{EnumVariant, Field, Node, Param, SwitchArm, SwitchBody};

const INDENT_SIZE: usize = 4;

pub fn print(node: &Node) -> String {
    let mut printer = Printer {
        out: Default::default(),
        indent: Default::default(),
    };
    let Node::Root(items) = node else {
        panic!("print expects a Root node");
    };
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            printer.out.push('\n');
        }
        printer.decl(item);
    }
    printer.out
}

struct Printer {
    out: String,
    indent: usize,
}

impl Printer {
    fn indent(&mut self) {
        self.indent += INDENT_SIZE;
    }

    fn dedent(&mut self) {
        self.indent -= INDENT_SIZE;
    }

    fn pad(&self) -> String {
        " ".repeat(self.indent)
    }

    fn decl(&mut self, node: &Node) {
        match node {
            Node::TestDecl(name, body) => {
                match name {
                    Some(name) => self.out.push_str(&format!("test \"{}\" ", name)),
                    None => self.out.push_str("test "),
                }
                self.block(body);
                self.out.push('\n');
            }
            Node::SimpleVarDecl { is_const, name, ty, expr } => {
                self.simple_var_decl(*is_const, name, ty.as_deref(), expr.as_deref());
            }
            Node::EnumDecl { name, type_params, is_union, variants, methods } => {
                let keyword = if *is_union { "union(enum)" } else { "enum" };
                if type_params.is_empty() {
                    self.out.push_str(&format!("const {} = {} {{\n", name, keyword));
                    self.indent();
                    self.container_body(methods, |printer| printer.variants(variants));
                    self.dedent();
                    self.out.push_str("};\n");
                } else {
                    let params: Vec<String> = type_params.iter().map(|p| format!("comptime {}: type", p)).collect();
                    self.out.push_str(&format!("fn {}({}) type {{\n", name, params.join(", ")));
                    self.indent();
                    self.out.push_str(&format!("{}return {} {{\n", self.pad(), keyword));
                    self.indent();
                    self.container_body(methods, |printer| printer.variants(variants));
                    self.dedent();
                    self.out.push_str(&format!("{}}};\n", self.pad()));
                    self.dedent();
                    self.out.push_str("}\n");
                }
            }
            Node::StructDecl { name, fields, methods } => {
                self.out.push_str(&format!("const {} = struct {{\n", name));
                self.indent();
                self.container_body(methods, |printer| {
                    for field in fields {
                        printer.out.push_str(&format!("{}{}: {},\n", printer.pad(), field.name, printer.expr(&field.ty)));
                    }
                });
                self.dedent();
                self.out.push_str("};\n");
            }
            Node::FnDecl { name, params, return_type, body } => {
                self.fn_decl(name, params, return_type, body);
                self.out.push('\n');
            }
            _ => self.out.push_str("// TODO: decl\n"),
        }
    }

    fn container_body(&mut self, methods: &[Node], members: impl FnOnce(&mut Self)) {
        if !methods.is_empty() {
            self.out.push_str(&format!("{}const Self = @This();\n\n", self.pad()));
        }
        members(self);
        for method in methods {
            self.out.push('\n');
            let Node::FnDecl { name, params, return_type, body } = method else { continue };
            self.fn_decl(name, params, return_type, body);
            self.out.push('\n');
        }
    }

    fn variants(&mut self, variants: &[EnumVariant]) {
        for variant in variants {
            match &variant.payload {
                Some(payload) => self.out.push_str(&format!("{}{}: {},\n", self.pad(), variant.name, self.expr(payload))),
                None => self.out.push_str(&format!("{}{},\n", self.pad(), variant.name)),
            }
        }
    }

    fn fn_decl(&mut self, name: &str, params: &[Param], return_type: &Node, body: &Node) {
        let params: Vec<String> = params.iter().map(|p| self.param(p)).collect();
        self.out.push_str(&format!(
            "{}fn {}({}) {} ",
            self.pad(),
            name,
            params.join(", "),
            self.expr(return_type),
        ));
        self.block(body);
    }

    fn param(&self, param: &Param) -> String {
        let prefix = if param.comptime { "comptime " } else { "" };
        format!("{}{}: {}", prefix, param.name, self.expr(&param.ty))
    }

    fn stmt(&mut self, node: &Node) {
        let pad = self.pad();
        match node {
            Node::SimpleVarDecl { is_const, name, ty, expr } => {
                self.simple_var_decl(*is_const, name, ty.as_deref(), expr.as_deref());
            }
            Node::AssignDestructure(names, expr) => {
                let vars: Vec<String> = names.iter().map(|name| format!("const {}", name)).collect();
                let expr = self.expr(expr);
                self.out.push_str(&format!("{}{} = {};\n", pad, vars.join(", "), expr));
            }
            Node::Return(expr) => match expr {
                Some(expr) => {
                    self.out.push_str(&format!("{}return ", pad));
                    self.value(expr);
                    self.out.push_str(";\n");
                }
                None => self.out.push_str(&format!("{}return;\n", pad)),
            },

            Node::For { iterables, captures, body } => {
                let iterables: Vec<String> = iterables.iter().map(|i| self.expr(i)).collect();
                let captures: Vec<String> = captures.iter()
                    .map(|capture| format!("{}{}", if capture.by_ref { "*" } else { "" }, capture.name))
                    .collect();
                self.out.push_str(&format!("{}for ({}) |{}| ", pad, iterables.join(", "), captures.join(", ")));
                self.block(body);
                self.out.push('\n');
            }
            Node::If { cond, capture, then_branch, else_branch } => {
                self.out.push_str(&format!("{}if ({}) ", pad, self.expr(cond)));
                if let Some(capture) = capture {
                    self.out.push_str(&format!("|{}| ", capture));
                }
                self.block(then_branch);
                if let Some(else_branch) = else_branch {
                    self.out.push_str(" else ");
                    self.block(else_branch);
                }
                self.out.push('\n');
            }
            Node::While { cond, body } => {
                self.out.push_str(&format!("{}while ({}) ", pad, self.expr(cond)));
                self.block(body);
                self.out.push('\n');
            }
            _ => self.out.push_str(&format!("{}{};\n", pad, self.expr(node))),
        }
    }

    fn simple_var_decl(&mut self, is_const: bool, name: &str, ty: Option<&Node>, expr: Option<&Node>) {
        let keyword = if is_const { "const" } else { "var" };
        self.out.push_str(&format!("{}{} {}", self.pad(), keyword, name));
        if let Some(ty) = ty {
            let ty = self.expr(ty);
            self.out.push_str(&format!(": {}", ty));
        }
        if let Some(expr) = expr {
            self.out.push_str(" = ");
            self.value(expr);
        }
        self.out.push_str(";\n");
    }

    fn block(&mut self, node: &Node) {
        let Node::Block(stmts) = node else {
            panic!("block expects a Block node");
        };
        self.out.push_str("{\n");
        self.indent();
        for stmt in stmts {
            self.stmt(stmt);
        }
        self.dedent();
        self.out.push_str(&format!("{}}}", self.pad()));
    }

    fn value(&mut self, node: &Node) {
        match node {
            Node::Closure { captures, has_self, params, return_type, body } => {
                self.closure(captures, *has_self, params, return_type, body)
            }
            Node::Switch { cond, arms } => self.switch(cond, arms),
            _ => {
                let text = self.expr(node);
                self.out.push_str(&text);
            }
        }
    }

    fn closure(&mut self, captures: &[Field], has_self: bool, params: &[Param], return_type: &Node, body: &Node) {
        self.out.push_str("struct {\n");
        self.indent();
        for field in captures {
            self.out.push_str(&format!("{}{}: {},\n", self.pad(), field.name, self.expr(&field.ty)));
        }
        let self_name = if has_self { "self" } else { "_" };
        let mut all_params = vec![format!("{}: @This()", self_name)];
        all_params.extend(params.iter().map(|p| self.param(p)));
        self.out.push_str(&format!("{}fn call({}) {} ", self.pad(), all_params.join(", "), self.expr(return_type)));
        self.block(body);
        self.out.push('\n');
        self.dedent();
        self.out.push_str(&format!("{}}}", self.pad()));
        if captures.is_empty() {
            self.out.push_str("{}");
        } else {
            let inits: Vec<String> = captures.iter().map(|field| format!(".{} = {}", field.name, field.name)).collect();
            self.out.push_str(&format!("{{ {} }}", inits.join(", ")));
        }
    }

    fn switch(&mut self, cond: &Node, arms: &[SwitchArm]) {
        self.out.push_str(&format!("switch ({}) {{\n", self.expr(cond)));
        self.indent();
        for arm in arms {
            self.out.push_str(&format!("{}{} => ", self.pad(), self.expr(&arm.pattern)));
            if let Some(capture) = &arm.capture {
                let star = if capture.by_ref { "*" } else { "" };
                self.out.push_str(&format!("|{}{}| ", star, capture.name));
            }
            match &arm.body {
                SwitchBody::Expr(body) => {
                    let body = self.expr(body);
                    self.out.push_str(&format!("{},\n", body));
                }
                SwitchBody::Block { bindings, result } => {
                    self.out.push_str("blk: {\n");
                    self.indent();
                    for binding in bindings {
                        self.stmt(binding);
                    }
                    self.out.push_str(&format!("{}break :blk {};\n", self.pad(), self.expr(result)));
                    self.dedent();
                    self.out.push_str(&format!("{}}},\n", self.pad()));
                }
            }
        }
        self.dedent();
        self.out.push_str(&format!("{}}}", self.pad()));
    }

    fn expr(&self, node: &Node) -> String {
        match node {
            Node::Add(left, right) |
            Node::AssignAdd(left, right) |
            Node::AssignDiv(left, right) |
            Node::AssignMod(left, right) |
            Node::AssignMul(left, right) |
            Node::AssignSub(left, right) |
            Node::BangEqual(left, right) |
            Node::Div(left, right) |
            Node::EqualEqual(left, right) |
            Node::GreaterOrEqual(left, right) |
            Node::GreaterThan(left, right) |
            Node::LessOrEqual(left, right) |
            Node::LessThan(left, right) |
            Node::Mod(left, right) |
            Node::Mul(left, right) |
            Node::Sub(left, right) => {
                let left = self.expr(left);
                let right = self.expr(right);
                let op = self.binop(node);
                format!("{} {} {}", left, op, right)
            }
            Node::AddressOf(expr) => format!("&{}", self.expr(expr)),
            Node::ArrayAccess(base, index) => format!("{}[{}]", self.expr(base), self.expr(index)),
            Node::ArrayInit(ty, elements) => {
                let ty = if let Some(ty) = ty { self.expr(ty) } else { ".".to_string() };
                let elements: Vec<String> = elements.iter().map(|e| self.expr(e)).collect();
                format!("{}{{ {} }}", ty, elements.join(", "))
            }
            Node::Assign(left, right) => format!("{} = {}", self.expr(left), self.expr(right)),
            Node::Break => "break".to_string(),
            Node::BuiltinCall(name, args) => {
                let args: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                format!("@{}({})", name, args.join(", "))
            }
            Node::Call(func, args) => {
                let func = self.expr(func);
                let args: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                format!("{}({})", func, args.join(", "))
            }
            Node::Continue => "continue".to_string(),
            Node::Deref(expr) => format!("{}.*", self.expr(expr)),
            Node::EnumLiteral(name) => format!(".{}", name),
            Node::FieldAccess(base, field) => format!("{}.{}", self.expr(base), field),
            Node::ForRange(start, end) => {
                let start = self.expr(start);
                let end = if let Some(end) = end { self.expr(end) } else { "".to_string() };
                format!("{}..{}", start, end)
            }
            Node::Identifier(name) => name.clone(),
            Node::NumberLiteral(text) => text.clone(),
            Node::StringLiteral(text) => format!("\"{}\"", text),
            Node::StructInit(ty, fields) => {
                let ty = if let Some(ty) = ty { self.expr(ty) } else { ".".to_string() };
                let fields: Vec<String> = fields.iter()
                    .map(|(name, value)| format!(".{} = {}", name, self.expr(value)))
                    .collect();
                format!("{}{{ {} }}", ty, fields.join(", "))
            }
            Node::Try(expr) => format!("try {}", self.expr(expr)),

            Node::ArrayType(len, ty) => format!("[{}]{}", self.expr(len), self.expr(ty)),
            Node::OptionalType(ty) => format!("?{}", self.expr(ty)),
            Node::PtrType { is_const, ty } => {
                let prefix = if *is_const { "*const " } else { "*" };
                format!("{}{}", prefix, self.expr(ty))
            }
            Node::SliceType(ty) => format!("[]const {}", self.expr(ty)),
            Node::StructType(fields) => {
                let fields: Vec<String> = fields.iter()
                    .map(|field| format!("{}: {}", field.name, self.expr(&field.ty)))
                    .collect();
                format!("struct {{ {} }}", fields.join(", "))
            }
            Node::TupleType(elements) => {
                let elements: Vec<String> = elements.iter().map(|e| self.expr(e)).collect();
                format!("struct {{ {} }}", elements.join(", "))
            }
            _ => "/* TODO: expr */".to_string(),
        }
    }

    fn binop(&self, node: &Node) -> &'static str {
        match node {
            Node::Add(..) => "+",
            Node::AssignAdd(..) => "+=",
            Node::AssignDiv(..) => "/=",
            Node::AssignMod(..) => "%=",
            Node::AssignMul(..) => "*=",
            Node::AssignSub(..) => "-=",
            Node::BangEqual(..) => "!=",
            Node::Div(..) => "/",
            Node::EqualEqual(..) => "==",
            Node::GreaterOrEqual(..) => ">=",
            Node::GreaterThan(..) => ">",
            Node::LessOrEqual(..) => "<=",
            Node::LessThan(..) => "<",
            Node::Mod(..) => "%",
            Node::Mul(..) => "*",
            Node::Sub(..) => "-",
            _ => "/* TODO: binop */",
        }
    }
}
