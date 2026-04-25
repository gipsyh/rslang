use super::utils::opt_str;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

pub(crate) fn source_loc(value: &Value) -> Option<SourceLoc> {
    Some(SourceLoc {
        file: opt_str(value, "source_file")?.to_string(),
        line: value.get("source_line")?.as_u64()?,
        column: value.get("source_column")?.as_u64()?,
    })
}

pub(crate) fn source_span(value: &Value) -> Option<SourceSpan> {
    Some(SourceSpan {
        file: opt_str(value, "source_file_start")?.to_string(),
        line_start: value.get("source_line_start")?.as_u64()?,
        column_start: value.get("source_column_start")?.as_u64()?,
        line_end: value.get("source_line_end")?.as_u64()?,
        column_end: value.get("source_column_end")?.as_u64()?,
    })
}
