#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Void,
    Bool,
    I32,
    I64,
    F64,
    String,
    Char,
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Vec(Box<Type>),
    HashMap(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Struct(StructDef),
    Enum(EnumDef),
    Function(Box<FunctionSig>),
    Trait(TraitDef),
    Reference(Box<Type>, Mutability),
    Owned(Box<Type>),
    DynTrait(String),
    Generic(String),
    Any,
    Never,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mutability {
    Mutable,
    Immutable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub impls: Vec<MethodDef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub ty: Type,
    pub is_pub: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodDef {
    pub name: String,
    pub params: Vec<ParamDef>,
    pub return_type: Type,
    pub body: Vec<RsStmt>,
    pub is_pub: bool,
    pub is_async: bool,
    pub self_param: Option<SelfParam>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelfParam {
    None,
    SelfRef,
    SelfMut,
    SelfOwned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParamDef {
    pub name: String,
    pub ty: Type,
    pub default: Option<RsExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSig {
    pub params: Vec<ParamDef>,
    pub return_type: Type,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitDef {
    pub name: String,
    pub methods: Vec<TraitMethod>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<ParamDef>,
    pub return_type: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RsLit {
    Bool(bool),
    I32(i32),
    I64(i64),
    F64(f64),
    Str(String),
    Char(char),
    Null,
    Undefined,
    NaN,
    Infinity,
    Array(Vec<RsExpr>),
    Object(Vec<(String, RsExpr)>),
    TemplateLiteral(Vec<TemplatePart>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    Text(String),
    Expression(RsExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    TypeOf,
    Void,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    StrictEq,
    StrictNeq,
    Lt,
    Lte,
    Gt,
    Gte,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UnsignedShr,
    Coalesce,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    AndAssign,
    OrAssign,
    XorAssign,
    ShlAssign,
    ShrAssign,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RsExpr {
    Lit(RsLit),
    Ident(String),
    Binary(BinOp, Box<RsExpr>, Box<RsExpr>),
    Unary(UnaryOp, Box<RsExpr>),
    Assign(AssignOp, Box<RsExpr>, Box<RsExpr>),
    Call(Box<RsExpr>, Vec<RsExpr>),
    New(String, Vec<RsExpr>),
    Member(Box<RsExpr>, String),
    Index(Box<RsExpr>, Box<RsExpr>),
    ArrowFunction(Vec<ParamDef>, Type, Vec<RsStmt>),
    FunctionExpr(Option<String>, Vec<ParamDef>, Type, Vec<RsStmt>),
    If(Box<RsExpr>, Vec<RsStmt>, Option<Vec<RsStmt>>),
    Match(Vec<MatchArm>),
    Block(Vec<RsStmt>),
    Array(Vec<RsExpr>),
    Object(Vec<(String, RsExpr)>),
    Tuple(Vec<RsExpr>),
    StructLiteral(String, Vec<(String, RsExpr)>),
    FieldAccess(Box<RsExpr>, String),
    MethodCall(Box<RsExpr>, String, Vec<RsExpr>),
    OptionalChain(Box<RsExpr>),
    NullishCoalesce(Box<RsExpr>, Box<RsExpr>),
    Spread(Vec<RsExpr>),
    Closure(Vec<ClosureParam>, Box<RsExpr>),
    AsyncBlock(Vec<RsStmt>),
    Await(Box<RsExpr>),
    TryBlock(Vec<RsStmt>, String, Vec<RsStmt>),
    Throw(Box<RsExpr>),
    Range(Box<RsExpr>, Box<RsExpr>),
    RangeInclusive(Box<RsExpr>, Box<RsExpr>),
    Reference(Box<RsExpr>),
    Deref(Box<RsExpr>),
    TypeAscription(Box<RsExpr>, Type),
    Path(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClosureParam {
    pub name: String,
    pub ty: Type,
    pub by_ref: bool,
    pub is_mutable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<RsExpr>,
    pub body: RsExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Lit(RsLit),
    Ident(String),
    Wildcard,
    Tuple(Vec<Pattern>),
    Struct(String, Vec<(String, Pattern)>),
    Enum(String, Option<String>, Vec<Pattern>),
    Or(Vec<Pattern>),
    Range(Box<RsExpr>, Box<RsExpr>),
    Reference(Box<Pattern>),
    Deref(Box<Pattern>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RsStmt {
    Expr(RsExpr),
    Let(String, Type, RsExpr),
    LetMut(String, Type, RsExpr),
    Return(Option<RsExpr>),
    Break,
    Continue,
    If(RsExpr, Vec<RsStmt>, Option<Vec<RsStmt>>),
    While(RsExpr, Vec<RsStmt>),
    For(String, RsExpr, Vec<RsStmt>),
    ForLoop {
        init: Option<Box<RsStmt>>,
        test: Option<RsExpr>,
        update: Option<RsExpr>,
        body: Vec<RsStmt>,
    },
    ForIn(String, RsExpr, Vec<RsStmt>),
    Loop(Vec<RsStmt>),
    Match(RsExpr, Vec<MatchArm>),
    Fn(FunctionDef),
    Struct(StructDef),
    Enum(EnumDef),
    Impl(ImplDef),
    Trait(TraitDef),
    Use(String),
    Mod(String),
    Pub(Box<RsStmt>),
    Async(Vec<RsStmt>),
    AwaitStmt(RsExpr),
    Try(Vec<RsStmt>, String, Vec<RsStmt>),
    Throw(RsExpr),
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: String,
    pub params: Vec<ParamDef>,
    pub return_type: Type,
    pub body: Vec<RsStmt>,
    pub is_async: bool,
    pub generics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplDef {
    pub self_type: Type,
    pub trait_name: Option<String>,
    pub methods: Vec<MethodDef>,
    pub generics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustModule {
    pub name: String,
    pub imports: Vec<RsImport>,
    pub items: Vec<RsStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RsImport {
    pub path: String,
    pub items: Vec<String>,
    pub is_glob: bool,
}

impl Type {
    pub fn to_rust_string(&self) -> String {
        match self {
            Type::Void => "()".to_string(),
            Type::Bool => "bool".to_string(),
            Type::I32 => "i32".to_string(),
            Type::I64 => "i64".to_string(),
            Type::F64 => "f64".to_string(),
            Type::String => "String".to_string(),
            Type::Char => "char".to_string(),
            Type::Option(inner) => format!("Option<{}>", inner.to_rust_string()),
            Type::Result(ok, err) => {
                format!("Result<{}, {}>", ok.to_rust_string(), err.to_rust_string())
            }
            Type::Vec(inner) => format!("Vec<{}>", inner.to_rust_string()),
            Type::HashMap(k, v) => {
                format!("HashMap<{}, {}>", k.to_rust_string(), v.to_rust_string())
            }
            Type::Tuple(elems) => {
                let inner: Vec<String> = elems.iter().map(|e| e.to_rust_string()).collect();
                format!("({})", inner.join(", "))
            }
            Type::Struct(def) => def.name.clone(),
            Type::Enum(def) => def.name.clone(),
            Type::Function(sig) => {
                let params: Vec<String> =
                    sig.params.iter().map(|p| p.ty.to_rust_string()).collect();
                format!(
                    "fn({}) -> {}",
                    params.join(", "),
                    sig.return_type.to_rust_string()
                )
            }
            Type::Trait(def) => def.name.clone(),
            Type::Reference(inner, mutability) => {
                let m = match mutability {
                    Mutability::Mutable => "mut ",
                    Mutability::Immutable => "",
                };
                format!("&{}{}", m, inner.to_rust_string())
            }
            Type::Owned(inner) => inner.to_rust_string(),
            Type::DynTrait(name) => format!("dyn {}", name),
            Type::Generic(name) => name.clone(),
            Type::Any => "serde_json::Value".to_string(),
            Type::Never => "!".to_string(),
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::I32 | Type::I64 | Type::F64)
    }

    pub fn is_option(&self) -> bool {
        matches!(self, Type::Option(_))
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Bool)
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Type::String)
    }

    pub fn is_void(&self) -> bool {
        matches!(self, Type::Void)
    }

    pub fn is_any(&self) -> bool {
        matches!(self, Type::Any)
    }

    pub fn unwrap_option(&self) -> &Type {
        match self {
            Type::Option(inner) => inner,
            _ => self,
        }
    }

    pub fn lift_option(&self) -> Type {
        if self.is_option() {
            self.clone()
        } else {
            Type::Option(Box::new(self.clone()))
        }
    }
}
