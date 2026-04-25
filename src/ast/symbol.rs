use serde::{Deserialize, Serialize};

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
