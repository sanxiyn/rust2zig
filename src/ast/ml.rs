//! An OCaml AST, modeled after Parsetree

/// Asttypes.closed_flag
pub enum ClosedFlag {
    Closed,
    Open,
}

/// Asttypes.direction_flag
pub enum Direction {
    #[allow(unused)]
    Downto,
    Upto,
}

/// Asttypes.mutable_flag
pub enum MutableFlag {
    Immutable,
    Mutable,
}

/// Asttypes.rec_flag
pub enum RecFlag {
    Nonrecursive,
    Recursive,
}

/// Longident.t
pub enum Longident {
    Ldot(Box<Longident>, String),
    Lident(String),
}

/// constant_desc
pub enum Constant {
    /// Pconst_char
    Char(char),
    /// Pconst_float
    #[allow(unused)]
    Float(String, Option<char>),
    /// Pconst_integer
    Integer(String, Option<char>),
    /// Pconst_string
    String(String),
}

/// expression_desc
pub enum Expression {
    /// Pexp_apply
    Apply(Box<Expression>, Vec<Expression>),
    /// Pexp_array
    Array(Vec<Expression>),
    /// `a.(i)`
    ArrayGet(Box<Expression>, Box<Expression>),
    /// Pexp_assert
    Assert(Box<Expression>),
    /// Pexp_constant
    Constant(Constant),
    /// Pexp_construct
    Construct(Longident, Option<Box<Expression>>),
    /// Pexp_field
    Field(Box<Expression>, Longident),
    /// Pexp_for
    For(Box<Pattern>, Box<Expression>, Box<Expression>, Direction, Box<Expression>),
    /// Pexp_function
    Function(Vec<Pattern>, Box<Expression>),
    /// Pexp_ident
    Ident(Longident),
    /// Pexp_ifthenelse
    IfThenElse(Box<Expression>, Box<Expression>, Option<Box<Expression>>),
    /// Pexp_let
    Let(RecFlag, Vec<ValueBinding>, Box<Expression>),
    /// `let exception`
    LetException(String, Option<CoreType>, Box<Expression>),
    /// Pexp_letop
    LetOp(Box<BindingOp>, Box<Expression>),
    /// Pexp_match
    Match(Box<Expression>, Vec<Case>),
    /// Pexp_record
    Record(Vec<(Longident, Expression)>, Option<Box<Expression>>),
    /// Pexp_sequence
    Sequence(Box<Expression>, Box<Expression>),
    /// Pexp_setfield
    SetField(Box<Expression>, Longident, Box<Expression>),
    /// `s.[i]`
    StringGet(Box<Expression>, Box<Expression>),
    /// Pexp_try
    Try(Box<Expression>, Vec<Case>),
    /// Pexp_tuple
    Tuple(Vec<Expression>),
    /// Pexp_while
    While(Box<Expression>, Box<Expression>),

    #[allow(unused)]
    Todo(String),
}

/// pattern_desc
pub enum Pattern {
    /// Ppat_constant
    Constant(Constant),
    /// Ppat_construct
    Construct(Longident, Option<Box<Pattern>>),
    /// Ppat_or
    Or(Vec<Pattern>),
    /// Ppat_record
    Record(Vec<(Longident, Pattern)>, ClosedFlag),
    /// Ppat_tuple
    Tuple(Vec<Pattern>),
    /// Ppat_var
    Var(String),
}

/// binding_op
pub struct BindingOp {
    pub op: String,
    pub pat: Pattern,
    pub exp: Expression,
}

/// case
pub struct Case {
    pub lhs: Pattern,
    pub guard: Option<Expression>,
    pub rhs: Expression,
}

/// value_binding
pub struct ValueBinding {
    pub pat: Pattern,
    pub params: Vec<Pattern>,
    pub expr: Expression,
}

/// structure_item_desc
pub enum StructureItem {
    /// Pstr_module
    Module(ModuleBinding),
    /// Pstr_type
    Type(Vec<TypeDeclaration>),
    /// Pstr_value
    Value(RecFlag, Vec<ValueBinding>),
}

/// module_binding
pub struct ModuleBinding {
    pub name: String,
    pub expr: ModuleExpr,
}

/// module_expr_desc
pub enum ModuleExpr {
    /// Pmod_apply
    #[allow(unused)]
    Apply(Box<ModuleExpr>, Box<ModuleExpr>),
    /// Pmod_ident
    #[allow(unused)]
    Ident(Longident),
    /// Pmod_structure
    Structure(Structure),
}

/// type_declaration
pub struct TypeDeclaration {
    pub name: String,
    pub params: Vec<String>,
    pub kind: TypeKind,
}

/// type_kind
pub enum TypeKind {
    /// Ptype_record
    Record(Vec<LabelDeclaration>),
    /// Ptype_variant
    Variant(Vec<ConstructorDeclaration>),
}

/// constructor_declaration
pub struct ConstructorDeclaration {
    pub name: String,
    pub args: ConstructorArguments,
}

/// constructor_arguments
pub enum ConstructorArguments {
    /// Pcstr_record
    Record(Vec<LabelDeclaration>),
    /// Pcstr_tuple
    Tuple(Vec<CoreType>),
}

/// label_declaration
pub struct LabelDeclaration {
    pub name: String,
    pub mutable: MutableFlag,
    pub ty: CoreType,
}

/// core_type_desc
pub enum CoreType {
    /// Ptyp_constr
    Constr(Longident, Vec<CoreType>),
    /// Ptyp_var
    Var(String),
}

/// structure
pub type Structure = Vec<StructureItem>;
