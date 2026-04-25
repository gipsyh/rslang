use super::expr::{Expr, lower_expr};
use super::json::{array, bool_field, kind, missing, opt_str, opt_string};
use super::source::{SourceSpan, source_span};
use super::types::{DataType, lower_optional_type};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        ty: Option<DataType>,
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

pub(crate) fn lower_stmt(value: &Value) -> Result<Stmt> {
    let source = source_span(value);
    let Some(node_kind) = kind(value) else {
        return Ok(Stmt::Unknown {
            kind: "<missing>".to_string(),
            source,
        });
    };

    match node_kind {
        "Empty" => Ok(Stmt::Empty { source }),
        "Timed" => {
            let timing = value
                .get("timing")
                .ok_or_else(|| missing("timing", "timed statement"))?;
            let stmt = value
                .get("stmt")
                .ok_or_else(|| missing("stmt", "timed statement"))?;
            Ok(Stmt::Timed {
                event: lower_event_control(timing)?,
                stmt: Box::new(lower_stmt(stmt)?),
                source,
            })
        }
        "Block" => {
            let block_kind = opt_string(value, "blockKind");
            let statements = value
                .get("body")
                .map(lower_block_body)
                .transpose()?
                .unwrap_or_default();
            Ok(Stmt::Block {
                kind: block_kind,
                statements,
                source,
            })
        }
        "List" => {
            let mut statements = Vec::new();
            for item in array(value, "list", "statement list")? {
                statements.push(lower_stmt(item)?);
            }
            Ok(Stmt::Block {
                kind: Some("List".to_string()),
                statements,
                source,
            })
        }
        "Conditional" => lower_conditional(value, source),
        "ExpressionStatement" => {
            let expr_value = value
                .get("expr")
                .ok_or_else(|| missing("expr", "expression statement"))?;
            if kind(expr_value) == Some("Assignment") {
                lower_assignment_stmt(expr_value, source)
            } else {
                Ok(Stmt::Expr {
                    expr: lower_expr(expr_value)?,
                    source,
                })
            }
        }
        "ImmediateAssertion" => {
            let cond = value
                .get("cond")
                .ok_or_else(|| missing("cond", "immediate assertion"))?;
            let action = value
                .get("ifTrue")
                .map(lower_stmt)
                .transpose()?
                .map(Box::new);
            Ok(Stmt::Assert {
                kind: lower_assertion_kind(opt_str(value, "assertionKind")),
                condition: lower_expr(cond)?,
                action,
                source,
            })
        }
        _ => Ok(Stmt::Unknown {
            kind: node_kind.to_string(),
            source,
        }),
    }
}

fn lower_block_body(value: &Value) -> Result<Vec<Stmt>> {
    match kind(value) {
        Some("List") => {
            let mut statements = Vec::new();
            for item in array(value, "list", "statement list")? {
                statements.push(lower_stmt(item)?);
            }
            Ok(statements)
        }
        _ => Ok(vec![lower_stmt(value)?]),
    }
}

fn lower_conditional(value: &Value, source: Option<SourceSpan>) -> Result<Stmt> {
    let mut conditions = Vec::new();
    for condition in array(value, "conditions", "conditional statement")? {
        let expr = condition
            .get("expr")
            .ok_or_else(|| missing("expr", "conditional condition"))?;
        conditions.push(lower_expr(expr)?);
    }

    let if_true = value
        .get("ifTrue")
        .ok_or_else(|| missing("ifTrue", "conditional statement"))?;
    let else_branch = value
        .get("ifFalse")
        .map(lower_stmt)
        .transpose()?
        .map(Box::new);

    Ok(Stmt::If {
        conditions,
        then_branch: Box::new(lower_stmt(if_true)?),
        else_branch,
        source,
    })
}

fn lower_assignment_stmt(value: &Value, source: Option<SourceSpan>) -> Result<Stmt> {
    let left = value
        .get("left")
        .ok_or_else(|| missing("left", "assignment"))?;
    let right = value
        .get("right")
        .ok_or_else(|| missing("right", "assignment"))?;
    Ok(Stmt::Assign {
        left: lower_expr(left)?,
        right: lower_expr(right)?,
        nonblocking: bool_field(value, "isNonBlocking"),
        ty: lower_optional_type(value)?,
        source,
    })
}

fn lower_event_control(value: &Value) -> Result<EventControl> {
    let source = source_span(value);
    match kind(value) {
        Some("SignalEvent") => {
            let expr = value
                .get("expr")
                .ok_or_else(|| missing("expr", "signal event"))?;
            Ok(EventControl::Signal {
                edge: lower_edge(opt_str(value, "edge")),
                expr: lower_expr(expr)?,
                source,
            })
        }
        Some(kind) => Ok(EventControl::Unknown {
            kind: kind.to_string(),
            source,
        }),
        None => Ok(EventControl::Unknown {
            kind: "<missing>".to_string(),
            source,
        }),
    }
}

fn lower_assertion_kind(value: Option<&str>) -> AssertionKind {
    match value {
        Some("Assert") => AssertionKind::Assert,
        Some("Assume") => AssertionKind::Assume,
        Some("Cover") => AssertionKind::Cover,
        Some(other) => AssertionKind::Unknown(other.to_string()),
        None => AssertionKind::Unknown("<missing>".to_string()),
    }
}

fn lower_edge(value: Option<&str>) -> Edge {
    match value {
        Some("PosEdge") => Edge::Pos,
        Some("NegEdge") => Edge::Neg,
        Some("None") => Edge::Any,
        Some(other) => Edge::Unknown(other.to_string()),
        None => Edge::Unknown("<missing>".to_string()),
    }
}
