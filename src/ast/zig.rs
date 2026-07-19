//! A Zig AST, modeled after std.zig.Ast

pub enum Node {
    /// .add
    Add(Box<Node>, Box<Node>),
    /// .address_of
    AddressOf(Box<Node>),
    /// .array_access
    ArrayAccess(Box<Node>, Box<Node>),
    /// .array_init
    ArrayInit(Option<Box<Node>>, Vec<Node>),
    /// `.{value} ** len` (Zig array splat / repeat init)
    ArrayRepeat(Box<Node>, Box<Node>),
    /// .assign
    Assign(Box<Node>, Box<Node>),
    /// .assign_add
    AssignAdd(Box<Node>, Box<Node>),
    /// .assign_bit_and
    AssignBitAnd(Box<Node>, Box<Node>),
    /// .assign_bit_or
    AssignBitOr(Box<Node>, Box<Node>),
    /// .assign_bit_xor
    AssignBitXor(Box<Node>, Box<Node>),
    /// .assign_destructure
    AssignDestructure(Vec<Var>, Box<Node>),
    /// .assign_div
    AssignDiv(Box<Node>, Box<Node>),
    /// .assign_mod
    AssignMod(Box<Node>, Box<Node>),
    /// .assign_mul
    AssignMul(Box<Node>, Box<Node>),
    /// .assign_sub
    AssignSub(Box<Node>, Box<Node>),
    /// .bang_equal
    BangEqual(Box<Node>, Box<Node>),
    /// .bit_and
    BitAnd(Box<Node>, Box<Node>),
    /// .bit_or
    BitOr(Box<Node>, Box<Node>),
    /// .bit_xor
    BitXor(Box<Node>, Box<Node>),
    /// .block
    Block(Vec<Node>),
    /// .bool_and
    BoolAnd(Box<Node>, Box<Node>),
    /// .bool_not
    BoolNot(Box<Node>),
    /// .bool_or
    BoolOr(Box<Node>, Box<Node>),
    /// .@"break"
    Break,
    /// .builtin_call
    BuiltinCall(String, Vec<Node>),
    /// .call
    Call(Box<Node>, Vec<Node>),
    /// .@"continue"
    Continue,
    /// .@"defer"
    Defer(Box<Node>),
    /// .deref
    Deref(Box<Node>),
    /// labeled block used as expression: `blk: { ... break :blk result; }`
    BlockExpr {
        stmts: Vec<Node>,
        result: Box<Node>,
    },
    /// .div
    Div(Box<Node>, Box<Node>),
    /// .enum_literal
    EnumLiteral(String),
    /// .equal_equal
    EqualEqual(Box<Node>, Box<Node>),
    /// .field_access
    FieldAccess(Box<Node>, String),
    /// .for_range
    ForRange(Box<Node>, Option<Box<Node>>),
    /// .greater_or_equal
    GreaterOrEqual(Box<Node>, Box<Node>),
    /// .greater_than
    GreaterThan(Box<Node>, Box<Node>),
    /// .grouped_expression
    GroupedExpression(Box<Node>),
    /// .identifier
    Identifier(String),
    /// .less_or_equal
    LessOrEqual(Box<Node>, Box<Node>),
    /// .less_than
    LessThan(Box<Node>, Box<Node>),
    /// .mod
    Mod(Box<Node>, Box<Node>),
    /// .mul
    Mul(Box<Node>, Box<Node>),
    /// .number_literal
    NumberLiteral(String),
    /// .@"return"
    Return(Option<Box<Node>>),
    /// .shl
    Shl(Box<Node>, Box<Node>),
    /// .shr
    Shr(Box<Node>, Box<Node>),
    /// .string_literal
    StringLiteral(String),
    /// .struct_init
    StructInit(Option<Box<Node>>, Vec<(String, Node)>),
    /// .sub
    Sub(Box<Node>, Box<Node>),
    /// .@"try"
    Try(Box<Node>),

    /// .array_type
    ArrayType(Box<Node>, Box<Node>),
    /// .optional_type
    OptionalType(Box<Node>),
    /// .ptr_type
    PtrType { is_const: bool, ty: Box<Node> },
    SliceType(Box<Node>),
    StructType(Vec<Field>),
    TupleType(Vec<Node>),

    /// .@"for"
    For {
        iterables: Vec<Node>,
        captures: Vec<Capture>,
        body: Box<Node>,
    },
    /// .@"if"
    If {
        cond: Box<Node>,
        capture: Option<String>,
        then_branch: Box<Node>,
        else_branch: Option<Box<Node>>,
    },
    /// .while_simple
    While {
        cond: Box<Node>,
        body: Box<Node>,
    },

    Closure {
        captures: Vec<Field>,
        has_self: bool,
        params: Vec<Param>,
        return_type: Box<Node>,
        body: Box<Node>,
    },
    Switch {
        cond: Box<Node>,
        arms: Vec<SwitchArm>,
    },

    /// .root
    Root(Vec<Node>),
    /// .test_decl
    TestDecl(Option<String>, Box<Node>),
    /// .simple_var_decl
    SimpleVarDecl {
        var: Var,
        expr: Option<Box<Node>>,
    },

    EnumDecl {
        name: String,
        type_params: Vec<String>,
        is_union: bool,
        variants: Vec<EnumVariant>,
        methods: Vec<Node>,
    },
    StructDecl {
        name: String,
        fields: Vec<Field>,
        methods: Vec<Node>,
    },
    FnDecl {
        name: String,
        params: Vec<Param>,
        return_type: Box<Node>,
        body: Box<Node>,
    },

    #[allow(unused)]
    Todo(String),
}

pub struct Capture {
    pub name: String,
    pub by_ref: bool,
}

pub struct EnumVariant {
    pub name: String,
    pub payload: Option<Node>,
}

pub struct Field {
    pub name: String,
    pub ty: Node,
}

pub struct Param {
    pub comptime: bool,
    pub name: String,
    pub ty: Node,
}

pub struct SwitchArm {
    pub pattern: Node,
    pub capture: Option<Capture>,
    pub body: SwitchBody,
}

pub enum SwitchBody {
    Expr(Node),
    Block { bindings: Vec<Node>, result: Node },
}

pub struct Var {
    pub is_const: bool,
    pub name: String,
    pub ty: Option<Box<Node>>,
}
