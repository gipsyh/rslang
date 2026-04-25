use super::json::{bool_field, kind, missing, opt_bool, opt_str, opt_string, str_field};
use super::source::{SourceLoc, source_loc};
use super::symbol::SymbolRef;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeDecl {
    pub name: String,
    pub ty: DataType,
    pub source: Option<SourceLoc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    Scalar(ScalarType),
    PredefinedInteger(PredefinedIntegerType),
    Floating(FloatingType),
    PackedArray {
        element: Box<DataType>,
        range: TypeRange,
    },
    FixedSizeUnpackedArray {
        element: Box<DataType>,
        range: TypeRange,
    },
    DynamicArray {
        element: Box<DataType>,
    },
    DpiOpenArray {
        element: Box<DataType>,
        packed: bool,
    },
    AssociativeArray {
        element: Box<DataType>,
        index: Option<Box<DataType>>,
    },
    Queue {
        element: Box<DataType>,
        max_bound: Option<u32>,
    },
    Enum {
        name: Option<String>,
        base: Box<DataType>,
        values: Vec<EnumValue>,
    },
    PackedStruct {
        name: Option<String>,
        signed: bool,
        fields: Vec<TypeField>,
    },
    UnpackedStruct {
        name: Option<String>,
        fields: Vec<TypeField>,
    },
    PackedUnion {
        name: Option<String>,
        signed: bool,
        tagged: bool,
        fields: Vec<TypeField>,
    },
    UnpackedUnion {
        name: Option<String>,
        tagged: bool,
        fields: Vec<TypeField>,
    },
    Void,
    Null,
    CHandle,
    String,
    Event,
    Unbounded,
    TypeRef,
    Untyped,
    Sequence,
    Property,
    VirtualInterface {
        name: Option<String>,
        iface: Option<SymbolRef>,
        modport: Option<SymbolRef>,
        real_iface: bool,
    },
    Alias {
        name: String,
        target: Option<SymbolRef>,
    },
    Error,
    Unknown {
        kind: String,
        name: Option<String>,
    },
}

impl Default for DataType {
    fn default() -> Self {
        Self::Unknown {
            kind: "<missing>".to_string(),
            name: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScalarType {
    pub kind: ScalarKind,
    pub signed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalarKind {
    Bit,
    Logic,
    Reg,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredefinedIntegerType {
    pub kind: PredefinedIntegerKind,
    pub signed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PredefinedIntegerKind {
    ShortInt,
    Int,
    LongInt,
    Byte,
    Integer,
    Time,
    Unknown(String),
}

impl PredefinedIntegerKind {
    pub fn default_signed(&self) -> bool {
        !matches!(self, Self::Time | Self::Unknown(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloatingType {
    pub kind: FloatingKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FloatingKind {
    Real,
    ShortReal,
    RealTime,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeRange {
    Range { left: i64, right: i64 },
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumValue {
    pub name: String,
    pub value: Option<String>,
    pub source: Option<SourceLoc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeField {
    pub name: String,
    pub ty: DataType,
    pub source: Option<SourceLoc>,
}

pub(crate) fn lower_type_decl(value: &Value) -> Result<TypeDecl> {
    Ok(TypeDecl {
        name: str_field(value, "name", "type declaration")?.to_string(),
        ty: lower_data_type(value)?,
        source: source_loc(value),
    })
}

pub(crate) fn lower_required_type(value: &Value, context: &str) -> Result<DataType> {
    let ty = value.get("type").ok_or_else(|| missing("type", context))?;
    lower_data_type(ty)
}

pub(crate) fn lower_optional_type(value: &Value) -> Result<Option<DataType>> {
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
