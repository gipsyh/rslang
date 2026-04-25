use crate::ast::Design;
use crate::error::{Error, Result};
use crate::lower_slang_ast_str;
use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
pub struct Slang {
    executable: PathBuf,
    extra_args: Vec<OsString>,
    include_source_info: bool,
    include_detailed_type_info: bool,
}

impl Default for Slang {
    fn default() -> Self {
        Self {
            executable: PathBuf::from("slang"),
            extra_args: Vec::new(),
            include_source_info: true,
            include_detailed_type_info: true,
        }
    }
}

impl Slang {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            ..Self::default()
        }
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        self.extra_args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn include_source_info(mut self, include_source_info: bool) -> Self {
        self.include_source_info = include_source_info;
        self
    }

    pub fn include_detailed_type_info(mut self, include_detailed_type_info: bool) -> Self {
        self.include_detailed_type_info = include_detailed_type_info;
        self
    }

    pub fn ast_json_for_file(&self, path: impl AsRef<Path>) -> Result<Value> {
        self.ast_json_for_files(&[path])
    }

    pub fn ast_json_for_files(&self, paths: &[impl AsRef<Path>]) -> Result<Value> {
        let output = self.output_ast_json(paths)?;
        parse_json_from_stdout(&output)
    }

    pub fn parse_file(&self, path: impl AsRef<Path>) -> Result<Design> {
        self.parse_files(&[path])
    }

    pub fn parse_files(&self, paths: &[impl AsRef<Path>]) -> Result<Design> {
        let output = self.output_ast_json(paths)?;
        lower_slang_ast_str(&output)
    }

    fn output_ast_json(&self, paths: &[impl AsRef<Path>]) -> Result<String> {
        let mut command = Command::new(&self.executable);
        command.arg("--quiet").arg("--ast-json").arg("-");
        if self.include_source_info {
            command.arg("--ast-json-source-info");
        }
        if self.include_detailed_type_info {
            command.arg("--ast-json-detailed-types");
        }
        command.args(&self.extra_args);
        for path in paths {
            command.arg(path.as_ref());
        }

        let output = command.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() {
            return Err(Error::SlangFailed {
                status: output.status,
                stdout,
                stderr,
            });
        }
        Ok(stdout)
    }
}

fn parse_json_from_stdout(output: &str) -> Result<Value> {
    match serde_json::from_str(output) {
        Ok(value) => Ok(value),
        Err(first_err) => {
            let Some(start) = output.find('{') else {
                return Err(first_err.into());
            };
            serde_json::from_str(&output[start..]).map_err(Into::into)
        }
    }
}
