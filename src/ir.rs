use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    pub modules: Vec<Module>,
}

impl Design {
    pub fn module(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|module| module.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub source: Option<SourceLoc>,
    pub parameters: Vec<Parameter>,
    pub ports: Vec<Port>,
    pub nets: Vec<Signal>,
    pub variables: Vec<Signal>,
    pub procedures: Vec<ProcedureBlock>,
}

impl Module {
    pub fn port(&self, name: &str) -> Option<&Port> {
        self.ports.iter().find(|port| port.name == name)
    }

    pub fn variable(&self, name: &str) -> Option<&Signal> {
        self.variables.iter().find(|variable| variable.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLoc {
    pub file: String,
    pub line: u64,
    pub column: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: String,
    pub line_start: u64,
    pub column_start: u64,
    pub line_end: u64,
    pub column_end: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub ty: String,
    pub value: Option<String>,
    pub initializer: Option<Expr>,
    pub source: Option<SourceLoc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub name: String,
    pub direction: PortDirection,
    pub ty: String,
    pub internal_symbol: Option<SymbolRef>,
    pub source: Option<SourceLoc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    In,
    Out,
    InOut,
    Ref,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    pub name: String,
    pub kind: SignalKind,
    pub ty: String,
    pub source: Option<SourceLoc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalKind {
    Net,
    Variable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcedureBlock {
    pub kind: ProcedureKind,
    pub body: Stmt,
    pub source: Option<SourceLoc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcedureKind {
    Always,
    Initial,
    Final,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stmt {
    Empty {
        source: Option<SourceSpan>,
    },
    Block {
        kind: Option<String>,
        statements: Vec<Stmt>,
        source: Option<SourceSpan>,
    },
    Timed {
        event: EventControl,
        stmt: Box<Stmt>,
        source: Option<SourceSpan>,
    },
    If {
        conditions: Vec<Expr>,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
        source: Option<SourceSpan>,
    },
    Assign {
        left: Expr,
        right: Expr,
        nonblocking: bool,
        ty: Option<String>,
        source: Option<SourceSpan>,
    },
    Expr {
        expr: Expr,
        source: Option<SourceSpan>,
    },
    Assert {
        kind: AssertionKind,
        condition: Expr,
        action: Option<Box<Stmt>>,
        source: Option<SourceSpan>,
    },
    Unknown {
        kind: String,
        source: Option<SourceSpan>,
    },
}

impl Stmt {
    pub fn source(&self) -> Option<&SourceSpan> {
        match self {
            Self::Empty { source }
            | Self::Block { source, .. }
            | Self::Timed { source, .. }
            | Self::If { source, .. }
            | Self::Assign { source, .. }
            | Self::Expr { source, .. }
            | Self::Assert { source, .. }
            | Self::Unknown { source, .. } => source.as_ref(),
        }
    }

    pub fn for_each<'a>(&'a self, visit: &mut impl FnMut(&'a Stmt)) {
        visit(self);
        match self {
            Self::Block { statements, .. } => {
                for stmt in statements {
                    stmt.for_each(visit);
                }
            }
            Self::Timed { stmt, .. } => stmt.for_each(visit),
            Self::If {
                then_branch,
                else_branch,
                ..
            } => {
                then_branch.for_each(visit);
                if let Some(else_branch) = else_branch {
                    else_branch.for_each(visit);
                }
            }
            Self::Assert { action, .. } => {
                if let Some(action) = action {
                    action.for_each(visit);
                }
            }
            Self::Empty { .. } | Self::Assign { .. } | Self::Expr { .. } | Self::Unknown { .. } => {
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssertionKind {
    Assert,
    Assume,
    Cover,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventControl {
    Signal {
        edge: Edge,
        expr: Expr,
        source: Option<SourceSpan>,
    },
    Unknown {
        kind: String,
        source: Option<SourceSpan>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edge {
    Pos,
    Neg,
    Any,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expr {
    NamedValue {
        symbol: SymbolRef,
        ty: Option<String>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    IntegerLiteral {
        value: String,
        ty: Option<String>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        ty: Option<String>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        ty: Option<String>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Conversion {
        ty: Option<String>,
        expr: Box<Expr>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Assignment {
        left: Box<Expr>,
        right: Box<Expr>,
        nonblocking: bool,
        ty: Option<String>,
        source: Option<SourceSpan>,
    },
    Unknown {
        kind: String,
        ty: Option<String>,
        source: Option<SourceSpan>,
    },
}

impl Expr {
    pub fn source(&self) -> Option<&SourceSpan> {
        match self {
            Self::NamedValue { source, .. }
            | Self::IntegerLiteral { source, .. }
            | Self::Unary { source, .. }
            | Self::Binary { source, .. }
            | Self::Conversion { source, .. }
            | Self::Assignment { source, .. }
            | Self::Unknown { source, .. } => source.as_ref(),
        }
    }

    pub fn ty(&self) -> Option<&str> {
        match self {
            Self::NamedValue { ty, .. }
            | Self::IntegerLiteral { ty, .. }
            | Self::Unary { ty, .. }
            | Self::Binary { ty, .. }
            | Self::Conversion { ty, .. }
            | Self::Assignment { ty, .. }
            | Self::Unknown { ty, .. } => ty.as_deref(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRef {
    pub id: Option<u64>,
    pub name: String,
}

impl SymbolRef {
    pub fn parse(raw: &str) -> Self {
        let mut parts = raw.split_whitespace();
        let first = parts.next();
        let second = parts.next();
        match (first, second) {
            (Some(id), Some(name)) => Self {
                id: id.parse().ok(),
                name: name.to_string(),
            },
            _ => Self {
                id: None,
                name: raw.to_string(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    LogicalNot,
    BitwiseNot,
    Plus,
    Minus,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Mod,
    LogicalAnd,
    LogicalOr,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Equality,
    Inequality,
    LessThan,
    LessThanEqual,
    GreaterThan,
    GreaterThanEqual,
    ShiftLeft,
    ShiftRight,
    Unknown(String),
}
