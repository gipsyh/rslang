use super::module::{Module, lower_module_instance};
use super::utils::{array, expect_kind, kind, missing};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    pub modules: Vec<Module>,
}

impl Design {
    pub fn module(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|module| module.name == name)
    }
}

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
