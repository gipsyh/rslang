use super::source::{SourceSpan, source_span};
use super::symbol::SymbolRef;
use super::types::{DataType, lower_optional_type};
use super::utils::{array, bool_field, kind, missing, opt_string, str_field};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum::EnumString;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expr {
    NamedValue {
        symbol: SymbolRef,
        ty: Option<DataType>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    IntegerLiteral {
        value: String,
        ty: Option<DataType>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        ty: Option<DataType>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        ty: Option<DataType>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Concatenation {
        operands: Vec<Expr>,
        ty: Option<DataType>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Replication {
        count: Box<Expr>,
        concat: Box<Expr>,
        ty: Option<DataType>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Conversion {
        ty: Option<DataType>,
        expr: Box<Expr>,
        constant: Option<String>,
        source: Option<SourceSpan>,
    },
    Assignment {
        left: Box<Expr>,
        right: Box<Expr>,
        nonblocking: bool,
        ty: Option<DataType>,
        source: Option<SourceSpan>,
    },
    Unknown {
        kind: String,
        ty: Option<DataType>,
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
            | Self::Concatenation { source, .. }
            | Self::Replication { source, .. }
            | Self::Conversion { source, .. }
            | Self::Assignment { source, .. }
            | Self::Unknown { source, .. } => source.as_ref(),
        }
    }

    pub fn ty(&self) -> Option<&DataType> {
        match self {
            Self::NamedValue { ty, .. }
            | Self::IntegerLiteral { ty, .. }
            | Self::Unary { ty, .. }
            | Self::Binary { ty, .. }
            | Self::Concatenation { ty, .. }
            | Self::Replication { ty, .. }
            | Self::Conversion { ty, .. }
            | Self::Assignment { ty, .. }
            | Self::Unknown { ty, .. } => ty.as_ref(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, EnumString)]
pub enum UnaryOp {
    Plus,
    Minus,
    BitwiseNot,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitwiseNand,
    BitwiseNor,
    BitwiseXnor,
    LogicalNot,
    Preincrement,
    Predecrement,
    Postincrement,
    Postdecrement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, EnumString)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Mod,
    BinaryAnd,
    BinaryOr,
    BinaryXor,
    BinaryXnor,
    Equality,
    Inequality,
    CaseEquality,
    CaseInequality,
    GreaterThanEqual,
    GreaterThan,
    LessThanEqual,
    LessThan,
    WildcardEquality,
    WildcardInequality,
    LogicalAnd,
    LogicalOr,
    LogicalImplication,
    LogicalEquivalence,
    LogicalShiftLeft,
    LogicalShiftRight,
    ArithmeticShiftLeft,
    ArithmeticShiftRight,
    Power,
}

pub(crate) fn lower_expr(value: &Value) -> Result<Expr> {
    let source = source_span(value);
    let ty = lower_optional_type(value)?;
    let constant = opt_string(value, "constant");
    let Some(kind) = kind(value) else {
        return Ok(Expr::Unknown {
            kind: "<missing>".to_string(),
            ty,
            source,
        });
    };

    match kind {
        "NamedValue" => {
            let symbol = str_field(value, "symbol", "named value")?;
            Ok(Expr::NamedValue {
                symbol: SymbolRef::parse(symbol),
                ty,
                constant,
                source,
            })
        }
        "IntegerLiteral" => Ok(Expr::IntegerLiteral {
            value: opt_string(value, "value").unwrap_or_default(),
            ty,
            constant,
            source,
        }),
        "UnaryOp" => {
            let operand = value
                .get("operand")
                .ok_or_else(|| missing("operand", "unary expression"))?;
            Ok(Expr::Unary {
                op: lower_unary_op(str_field(value, "op", "unary expression")?)?,
                expr: Box::new(lower_expr(operand)?),
                ty,
                constant,
                source,
            })
        }
        "BinaryOp" => {
            let left = value
                .get("left")
                .ok_or_else(|| missing("left", "binary expression"))?;
            let right = value
                .get("right")
                .ok_or_else(|| missing("right", "binary expression"))?;
            Ok(Expr::Binary {
                op: lower_binary_op(str_field(value, "op", "binary expression")?)?,
                left: Box::new(lower_expr(left)?),
                right: Box::new(lower_expr(right)?),
                ty,
                constant,
                source,
            })
        }
        "Concatenation" => {
            let mut operands = Vec::new();
            for operand in array(value, "operands", "concatenation expression")? {
                operands.push(lower_expr(operand)?);
            }
            Ok(Expr::Concatenation {
                operands,
                ty,
                constant,
                source,
            })
        }
        "Replication" => {
            let count = value
                .get("count")
                .ok_or_else(|| missing("count", "replication expression"))?;
            let concat = value
                .get("concat")
                .ok_or_else(|| missing("concat", "replication expression"))?;
            Ok(Expr::Replication {
                count: Box::new(lower_expr(count)?),
                concat: Box::new(lower_expr(concat)?),
                ty,
                constant,
                source,
            })
        }
        "Conversion" => {
            let operand = value
                .get("operand")
                .ok_or_else(|| missing("operand", "conversion expression"))?;
            Ok(Expr::Conversion {
                ty,
                expr: Box::new(lower_expr(operand)?),
                constant,
                source,
            })
        }
        "Assignment" => {
            let left = value
                .get("left")
                .ok_or_else(|| missing("left", "assignment expression"))?;
            let right = value
                .get("right")
                .ok_or_else(|| missing("right", "assignment expression"))?;
            Ok(Expr::Assignment {
                left: Box::new(lower_expr(left)?),
                right: Box::new(lower_expr(right)?),
                nonblocking: bool_field(value, "isNonBlocking"),
                ty,
                source,
            })
        }
        _ => Ok(Expr::Unknown {
            kind: kind.to_string(),
            ty,
            source,
        }),
    }
}

fn lower_unary_op(value: &str) -> Result<UnaryOp> {
    value
        .parse()
        .map_err(|_| Error::Message(format!("unknown unary operator `{value}`")))
}

fn lower_binary_op(value: &str) -> Result<BinaryOp> {
    value
        .parse()
        .map_err(|_| Error::Message(format!("unknown binary operator `{value}`")))
}
