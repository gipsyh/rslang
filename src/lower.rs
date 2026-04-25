use crate::error::{Error, Result};
use crate::ir::*;
use serde_json::Value;

pub fn lower_slang_ast_str(json: &str) -> Result<Design> {
    let value = match serde_json::from_str(json) {
        Ok(value) => value,
        Err(err) => {
            let Some(start) = json.find('{') else {
                return Err(err.into());
            };
            serde_json::from_str(&json[start..])?
        }
    };
    lower_slang_ast(&value)
}

pub fn lower_slang_ast(value: &Value) -> Result<Design> {
    let design = value
        .get("design")
        .ok_or_else(|| missing("design", "root AST JSON"))?;
    expect_kind(design, "Root")?;

    let mut modules = Vec::new();
    for member in array(design, "members", "root design")? {
        if kind(member) == Some("Instance") {
            modules.push(lower_module_instance(member)?);
        }
    }

    Ok(Design { modules })
}

fn lower_module_instance(value: &Value) -> Result<Module> {
    let body = value
        .get("body")
        .ok_or_else(|| missing("body", "module instance"))?;
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
        match kind(member) {
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

fn lower_type_decl(value: &Value) -> Result<TypeDecl> {
    Ok(TypeDecl {
        name: str_field(value, "name", "type declaration")?.to_string(),
        ty: lower_data_type(value)?,
        source: source_loc(value),
    })
}

fn lower_parameter(value: &Value) -> Result<Parameter> {
    Ok(Parameter {
        name: str_field(value, "name", "parameter")?.to_string(),
        ty: lower_required_type(value, "parameter")?,
        value: opt_string(value, "value"),
        initializer: value.get("initializer").map(lower_expr).transpose()?,
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
        .ok_or_else(|| missing("body", "procedural block"))?;
    Ok(ProcedureBlock {
        kind: lower_procedure_kind(opt_str(value, "procedureKind")),
        body: lower_stmt(body)?,
        source: source_loc(value),
    })
}

fn lower_stmt(value: &Value) -> Result<Stmt> {
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

fn lower_expr(value: &Value) -> Result<Expr> {
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

fn lower_required_type(value: &Value, context: &str) -> Result<DataType> {
    let ty = value.get("type").ok_or_else(|| missing("type", context))?;
    lower_data_type(ty)
}

fn lower_optional_type(value: &Value) -> Result<Option<DataType>> {
    value.get("type").map(lower_data_type).transpose()
}

fn lower_data_type(value: &Value) -> Result<DataType> {
    if let Some(raw) = value.as_str() {
        return Ok(parse_type_text(raw));
    }

    let Some(type_kind) = kind(value) else {
        return Ok(DataType::Unknown {
            kind: "<missing>".to_string(),
            name: type_name(value),
        });
    };

    match type_kind {
        "ScalarType" => Ok(DataType::Scalar(ScalarType {
            kind: lower_scalar_kind(opt_str(value, "name")),
            signed: bool_field(value, "isSigned"),
        })),
        "PredefinedIntegerType" => {
            let kind = lower_predefined_integer_kind(opt_str(value, "name"));
            Ok(DataType::PredefinedInteger(PredefinedIntegerType {
                signed: opt_bool(value, "isSigned").unwrap_or_else(|| kind.default_signed()),
                kind,
            }))
        }
        "FloatingType" => Ok(DataType::Floating(FloatingType {
            kind: lower_floating_kind(opt_str(value, "name")),
        })),
        "PackedArrayType" => Ok(DataType::PackedArray {
            element: Box::new(lower_type_field(value, "elementType", "packed array type")?),
            range: lower_type_range(value, "range"),
        }),
        "FixedSizeUnpackedArrayType" => Ok(DataType::FixedSizeUnpackedArray {
            element: Box::new(lower_type_field(
                value,
                "elementType",
                "fixed size unpacked array type",
            )?),
            range: lower_type_range(value, "range"),
        }),
        "DynamicArrayType" => Ok(DataType::DynamicArray {
            element: Box::new(lower_type_field(
                value,
                "elementType",
                "dynamic array type",
            )?),
        }),
        "DPIOpenArrayType" => Ok(DataType::DpiOpenArray {
            element: Box::new(lower_type_field(
                value,
                "elementType",
                "DPI open array type",
            )?),
            packed: bool_field(value, "isPacked"),
        }),
        "AssociativeArrayType" => Ok(DataType::AssociativeArray {
            element: Box::new(lower_type_field(
                value,
                "elementType",
                "associative array type",
            )?),
            index: value
                .get("indexType")
                .map(lower_data_type)
                .transpose()?
                .map(Box::new),
        }),
        "QueueType" => Ok(DataType::Queue {
            element: Box::new(lower_type_field(value, "elementType", "queue type")?),
            max_bound: value
                .get("maxBound")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
        }),
        "EnumType" => Ok(DataType::Enum {
            name: type_name(value),
            base: Box::new(
                value
                    .get("baseType")
                    .map(lower_data_type)
                    .transpose()?
                    .unwrap_or_default(),
            ),
            values: lower_enum_values(value)?,
        }),
        "PackedStructType" => Ok(DataType::PackedStruct {
            name: type_name(value),
            signed: bool_field(value, "isSigned"),
            fields: lower_type_fields(value)?,
        }),
        "UnpackedStructType" => Ok(DataType::UnpackedStruct {
            name: type_name(value),
            fields: lower_type_fields(value)?,
        }),
        "PackedUnionType" => Ok(DataType::PackedUnion {
            name: type_name(value),
            signed: bool_field(value, "isSigned"),
            tagged: bool_field(value, "isTagged"),
            fields: lower_type_fields(value)?,
        }),
        "UnpackedUnionType" => Ok(DataType::UnpackedUnion {
            name: type_name(value),
            tagged: bool_field(value, "isTagged"),
            fields: lower_type_fields(value)?,
        }),
        "VoidType" => Ok(DataType::Void),
        "NullType" => Ok(DataType::Null),
        "CHandleType" => Ok(DataType::CHandle),
        "StringType" => Ok(DataType::String),
        "EventType" => Ok(DataType::Event),
        "UnboundedType" => Ok(DataType::Unbounded),
        "TypeRefType" => Ok(DataType::TypeRef),
        "UntypedType" => Ok(DataType::Untyped),
        "SequenceType" => Ok(DataType::Sequence),
        "PropertyType" => Ok(DataType::Property),
        "VirtualInterfaceType" => Ok(DataType::VirtualInterface {
            name: type_name(value),
            iface: opt_str(value, "iface")
                .or_else(|| opt_str(value, "interface"))
                .map(SymbolRef::parse),
            modport: opt_str(value, "modport").map(SymbolRef::parse),
            real_iface: bool_field(value, "isRealIface"),
        }),
        "TypeAlias" => Ok(DataType::Alias {
            name: opt_string(value, "name").unwrap_or_default(),
            target: opt_str(value, "target").map(SymbolRef::parse),
        }),
        "ErrorType" => Ok(DataType::Error),
        other => Ok(DataType::Unknown {
            kind: other.to_string(),
            name: type_name(value),
        }),
    }
}

fn lower_type_field(value: &Value, field: &'static str, context: &str) -> Result<DataType> {
    value
        .get(field)
        .map(lower_data_type)
        .transpose()?
        .ok_or_else(|| missing(field, context))
}

fn lower_enum_values(value: &Value) -> Result<Vec<EnumValue>> {
    let Some(members) = value.get("members").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let mut values = Vec::new();
    for member in members {
        if kind(member) == Some("EnumValue") {
            values.push(EnumValue {
                name: str_field(member, "name", "enum value")?.to_string(),
                value: opt_string(member, "value"),
                source: source_loc(member),
            });
        }
    }
    Ok(values)
}

fn lower_type_fields(value: &Value) -> Result<Vec<TypeField>> {
    let members = value
        .get("members")
        .or_else(|| value.get("fields"))
        .and_then(Value::as_array);
    let Some(members) = members else {
        return Ok(Vec::new());
    };

    let mut fields = Vec::new();
    for member in members {
        if kind(member) == Some("Field") || member.get("type").is_some() {
            fields.push(TypeField {
                name: str_field(member, "name", "type field")?.to_string(),
                ty: lower_required_type(member, "type field")?,
                source: source_loc(member),
            });
        }
    }
    Ok(fields)
}

fn lower_scalar_kind(value: Option<&str>) -> ScalarKind {
    match value {
        Some("bit") | Some("Bit") => ScalarKind::Bit,
        Some("logic") | Some("Logic") => ScalarKind::Logic,
        Some("reg") | Some("Reg") => ScalarKind::Reg,
        Some(other) => ScalarKind::Unknown(other.to_string()),
        None => ScalarKind::Unknown("<missing>".to_string()),
    }
}

fn lower_predefined_integer_kind(value: Option<&str>) -> PredefinedIntegerKind {
    match value {
        Some("shortint") | Some("ShortInt") => PredefinedIntegerKind::ShortInt,
        Some("int") | Some("Int") => PredefinedIntegerKind::Int,
        Some("longint") | Some("LongInt") => PredefinedIntegerKind::LongInt,
        Some("byte") | Some("Byte") => PredefinedIntegerKind::Byte,
        Some("integer") | Some("Integer") => PredefinedIntegerKind::Integer,
        Some("time") | Some("Time") => PredefinedIntegerKind::Time,
        Some(other) => PredefinedIntegerKind::Unknown(other.to_string()),
        None => PredefinedIntegerKind::Unknown("<missing>".to_string()),
    }
}

fn lower_floating_kind(value: Option<&str>) -> FloatingKind {
    match value {
        Some("real") | Some("Real") => FloatingKind::Real,
        Some("shortreal") | Some("ShortReal") => FloatingKind::ShortReal,
        Some("realtime") | Some("RealTime") => FloatingKind::RealTime,
        Some(other) => FloatingKind::Unknown(other.to_string()),
        None => FloatingKind::Unknown("<missing>".to_string()),
    }
}

fn lower_type_range(value: &Value, field: &str) -> TypeRange {
    opt_str(value, field)
        .map(parse_type_range)
        .unwrap_or_else(|| TypeRange::Unknown("<missing>".to_string()))
}

fn parse_type_range(raw: &str) -> TypeRange {
    let trimmed = raw.trim();
    let Some(body) = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return TypeRange::Unknown(trimmed.to_string());
    };

    let Some((left, right)) = body.split_once(':') else {
        return TypeRange::Unknown(trimmed.to_string());
    };

    match (left.trim().parse(), right.trim().parse()) {
        (Ok(left), Ok(right)) => TypeRange::Range { left, right },
        _ => TypeRange::Unknown(trimmed.to_string()),
    }
}

fn parse_type_text(raw: &str) -> DataType {
    let raw = raw.trim();
    if raw.is_empty() {
        return DataType::default();
    }

    let mut base = raw;
    let mut ranges = Vec::new();
    while let Some(prefix) = base.strip_suffix(']') {
        let Some(start) = prefix.rfind('[') else {
            break;
        };
        ranges.push(parse_type_range(&base[start..]));
        base = prefix[..start].trim_end();
    }

    let mut signed = None;
    let mut base_parts = Vec::new();
    for part in base.split_whitespace() {
        match part {
            "signed" => signed = Some(true),
            "unsigned" => signed = Some(false),
            other => base_parts.push(other),
        }
    }
    let base = base_parts.join(" ");

    let mut ty = match base.as_str() {
        "bit" => DataType::Scalar(ScalarType {
            kind: ScalarKind::Bit,
            signed: signed.unwrap_or(false),
        }),
        "logic" => DataType::Scalar(ScalarType {
            kind: ScalarKind::Logic,
            signed: signed.unwrap_or(false),
        }),
        "reg" => DataType::Scalar(ScalarType {
            kind: ScalarKind::Reg,
            signed: signed.unwrap_or(false),
        }),
        "shortint" => predefined_integer_type(PredefinedIntegerKind::ShortInt, signed),
        "int" => predefined_integer_type(PredefinedIntegerKind::Int, signed),
        "longint" => predefined_integer_type(PredefinedIntegerKind::LongInt, signed),
        "byte" => predefined_integer_type(PredefinedIntegerKind::Byte, signed),
        "integer" => predefined_integer_type(PredefinedIntegerKind::Integer, signed),
        "time" => predefined_integer_type(PredefinedIntegerKind::Time, signed),
        "real" => DataType::Floating(FloatingType {
            kind: FloatingKind::Real,
        }),
        "shortreal" => DataType::Floating(FloatingType {
            kind: FloatingKind::ShortReal,
        }),
        "realtime" => DataType::Floating(FloatingType {
            kind: FloatingKind::RealTime,
        }),
        "void" => DataType::Void,
        "null" => DataType::Null,
        "chandle" => DataType::CHandle,
        "string" => DataType::String,
        "event" => DataType::Event,
        "$" => DataType::Unbounded,
        "type reference" => DataType::TypeRef,
        "untyped" => DataType::Untyped,
        "sequence" => DataType::Sequence,
        "property" => DataType::Property,
        _ => DataType::Unknown {
            kind: "StringType".to_string(),
            name: Some(raw.to_string()),
        },
    };

    for range in ranges.into_iter().rev() {
        ty = DataType::PackedArray {
            element: Box::new(ty),
            range,
        };
    }

    ty
}

fn predefined_integer_type(kind: PredefinedIntegerKind, signed: Option<bool>) -> DataType {
    DataType::PredefinedInteger(PredefinedIntegerType {
        signed: signed.unwrap_or_else(|| kind.default_signed()),
        kind,
    })
}

fn type_name(value: &Value) -> Option<String> {
    opt_str(value, "name")
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
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

fn expect_kind(value: &Value, expected: &'static str) -> Result<()> {
    if kind(value) == Some(expected) {
        Ok(())
    } else {
        Err(Error::UnexpectedKind {
            expected,
            actual: kind(value).map(ToOwned::to_owned),
        })
    }
}

fn kind(value: &Value) -> Option<&str> {
    opt_str(value, "kind")
}

fn str_field<'a>(value: &'a Value, field: &'static str, context: &str) -> Result<&'a str> {
    opt_str(value, field).ok_or_else(|| missing(field, context))
}

fn opt_str<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn opt_string(value: &Value, field: &str) -> Option<String> {
    opt_str(value, field).map(ToOwned::to_owned)
}

fn bool_field(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn opt_bool(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}

fn array<'a>(value: &'a Value, field: &'static str, context: &str) -> Result<&'a [Value]> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| missing(field, context))
}

fn source_loc(value: &Value) -> Option<SourceLoc> {
    Some(SourceLoc {
        file: opt_str(value, "source_file")?.to_string(),
        line: value.get("source_line")?.as_u64()?,
        column: value.get("source_column")?.as_u64()?,
    })
}

fn source_span(value: &Value) -> Option<SourceSpan> {
    Some(SourceSpan {
        file: opt_str(value, "source_file_start")?.to_string(),
        line_start: value.get("source_line_start")?.as_u64()?,
        column_start: value.get("source_column_start")?.as_u64()?,
        line_end: value.get("source_line_end")?.as_u64()?,
        column_end: value.get("source_column_end")?.as_u64()?,
    })
}

fn missing(field: &'static str, context: impl Into<String>) -> Error {
    Error::MissingField {
        field,
        context: context.into(),
    }
}
