use super::expr::Expr;
use super::source::{SourceLoc, source_loc};
use super::statement::{Stmt, lower_stmt};
use super::symbol::SymbolRef;
use super::types::{DataType, TypeDecl, lower_required_type, lower_type_decl};
use super::utils::{array, expect_kind, opt_str, opt_string, str_field};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub source: Option<SourceLoc>,
    pub types: Vec<TypeDecl>,
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

    pub fn type_decl(&self, name: &str) -> Option<&TypeDecl> {
        self.types.iter().find(|ty| ty.name == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub ty: DataType,
    pub value: Option<String>,
    pub initializer: Option<Expr>,
    pub source: Option<SourceLoc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub name: String,
    pub direction: PortDirection,
    pub ty: DataType,
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
    pub ty: DataType,
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

pub(crate) fn lower_module_instance(value: &Value) -> Result<Module> {
    let body = value
        .get("body")
        .ok_or_else(|| super::utils::missing("body", "module instance"))?;
    expect_kind(body, "InstanceBody")?;

    let mut module = Module {
        name: str_field(body, "name", "module body")?.to_string(),
        source: source_loc(body),
        types: Vec::new(),
        parameters: Vec::new(),
        ports: Vec::new(),
        nets: Vec::new(),
        variables: Vec::new(),
        procedures: Vec::new(),
    };

    for member in array(body, "members", "module body")? {
        match super::utils::kind(member) {
            Some("EnumType") | Some("TypeAlias") => module.types.push(lower_type_decl(member)?),
            Some("Parameter") => module.parameters.push(lower_parameter(member)?),
            Some("Port") => module.ports.push(lower_port(member)?),
            Some("Net") => module.nets.push(lower_signal(member, SignalKind::Net)?),
            Some("Variable") => module
                .variables
                .push(lower_signal(member, SignalKind::Variable)?),
            Some("ProceduralBlock") => module.procedures.push(lower_procedure(member)?),
            _ => {}
        }
    }

    Ok(module)
}

fn lower_parameter(value: &Value) -> Result<Parameter> {
    Ok(Parameter {
        name: str_field(value, "name", "parameter")?.to_string(),
        ty: lower_required_type(value, "parameter")?,
        value: opt_string(value, "value"),
        initializer: value
            .get("initializer")
            .map(super::expr::lower_expr)
            .transpose()?,
        source: source_loc(value),
    })
}

fn lower_port(value: &Value) -> Result<Port> {
    Ok(Port {
        name: str_field(value, "name", "port")?.to_string(),
        direction: lower_port_direction(opt_str(value, "direction")),
        ty: lower_required_type(value, "port")?,
        internal_symbol: opt_str(value, "internalSymbol").map(SymbolRef::parse),
        source: source_loc(value),
    })
}

fn lower_signal(value: &Value, kind: SignalKind) -> Result<Signal> {
    Ok(Signal {
        name: str_field(value, "name", "signal")?.to_string(),
        kind,
        ty: lower_required_type(value, "signal")?,
        source: source_loc(value),
    })
}

fn lower_procedure(value: &Value) -> Result<ProcedureBlock> {
    let body = value
        .get("body")
        .ok_or_else(|| super::utils::missing("body", "procedural block"))?;
    Ok(ProcedureBlock {
        kind: lower_procedure_kind(opt_str(value, "procedureKind")),
        body: lower_stmt(body)?,
        source: source_loc(value),
    })
}

fn lower_port_direction(value: Option<&str>) -> PortDirection {
    match value {
        Some("In") => PortDirection::In,
        Some("Out") => PortDirection::Out,
        Some("InOut") => PortDirection::InOut,
        Some("Ref") => PortDirection::Ref,
        Some(other) => PortDirection::Unknown(other.to_string()),
        None => PortDirection::Unknown("<missing>".to_string()),
    }
}

fn lower_procedure_kind(value: Option<&str>) -> ProcedureKind {
    match value {
        Some("Always") => ProcedureKind::Always,
        Some("Initial") => ProcedureKind::Initial,
        Some("Final") => ProcedureKind::Final,
        Some(other) => ProcedureKind::Unknown(other.to_string()),
        None => ProcedureKind::Unknown("<missing>".to_string()),
    }
}
