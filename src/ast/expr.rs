use super::source::{SourceSpan, source_span};
use super::symbol::SymbolRef;
use super::types::{DataType, lower_optional_type};
use super::utils::{bool_field, kind, missing, opt_str, opt_string, str_field};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
            | Self::Conversion { ty, .. }
            | Self::Assignment { ty, .. }
            | Self::Unknown { ty, .. } => ty.as_ref(),
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
                op: lower_unary_op(opt_str(value, "op")),
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
                op: lower_binary_op(opt_str(value, "op")),
                left: Box::new(lower_expr(left)?),
                right: Box::new(lower_expr(right)?),
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

fn lower_unary_op(value: Option<&str>) -> UnaryOp {
    match value {
        Some("LogicalNot") => UnaryOp::LogicalNot,
        Some("BitwiseNot") => UnaryOp::BitwiseNot,
        Some("Plus") => UnaryOp::Plus,
        Some("Minus") => UnaryOp::Minus,
        Some(other) => UnaryOp::Unknown(other.to_string()),
        None => UnaryOp::Unknown("<missing>".to_string()),
    }
}

fn lower_binary_op(value: Option<&str>) -> BinaryOp {
    match value {
        Some("Add") => BinaryOp::Add,
        Some("Subtract") => BinaryOp::Subtract,
        Some("Multiply") => BinaryOp::Multiply,
        Some("Divide") => BinaryOp::Divide,
        Some("Mod") => BinaryOp::Mod,
        Some("LogicalAnd") => BinaryOp::LogicalAnd,
        Some("LogicalOr") => BinaryOp::LogicalOr,
        Some("BinaryAnd") | Some("BitwiseAnd") => BinaryOp::BitwiseAnd,
        Some("BinaryOr") | Some("BitwiseOr") => BinaryOp::BitwiseOr,
        Some("BinaryXor") | Some("BitwiseXor") => BinaryOp::BitwiseXor,
        Some("Equality") => BinaryOp::Equality,
        Some("Inequality") => BinaryOp::Inequality,
        Some("LessThan") => BinaryOp::LessThan,
        Some("LessThanEqual") => BinaryOp::LessThanEqual,
        Some("GreaterThan") => BinaryOp::GreaterThan,
        Some("GreaterThanEqual") => BinaryOp::GreaterThanEqual,
        Some("LogicalShiftLeft") | Some("ShiftLeft") => BinaryOp::ShiftLeft,
        Some("LogicalShiftRight") | Some("ShiftRight") => BinaryOp::ShiftRight,
        Some(other) => BinaryOp::Unknown(other.to_string()),
        None => BinaryOp::Unknown("<missing>".to_string()),
    }
}
