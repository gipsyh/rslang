use crate::error::{Error, Result};
use serde_json::Value;

pub(crate) fn expect_kind(value: &Value, expected: &'static str) -> Result<()> {
    if kind(value) == Some(expected) {
        Ok(())
    } else {
        Err(Error::UnexpectedKind {
            expected,
            actual: kind(value).map(ToOwned::to_owned),
        })
    }
}

pub(crate) fn kind(value: &Value) -> Option<&str> {
    opt_str(value, "kind")
}

pub(crate) fn str_field<'a>(
    value: &'a Value,
    field: &'static str,
    context: &str,
) -> Result<&'a str> {
    opt_str(value, field).ok_or_else(|| missing(field, context))
}

pub(crate) fn opt_str<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

pub(crate) fn opt_string(value: &Value, field: &str) -> Option<String> {
    opt_str(value, field).map(ToOwned::to_owned)
}

pub(crate) fn bool_field(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

pub(crate) fn opt_bool(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}

pub(crate) fn array<'a>(
    value: &'a Value,
    field: &'static str,
    context: &str,
) -> Result<&'a [Value]> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| missing(field, context))
}

pub(crate) fn missing(field: &'static str, context: impl Into<String>) -> Error {
    Error::MissingField {
        field,
        context: context.into(),
    }
}
