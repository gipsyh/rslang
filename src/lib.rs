mod error;
mod ir;
mod lower;
mod slang;

pub use error::{Error, Result};
pub use ir::*;
pub use lower::{lower_slang_ast, lower_slang_ast_str};
pub use slang::Slang;

use std::path::Path;

pub fn parse_file(path: impl AsRef<Path>) -> Result<Design> {
    Slang::default().parse_file(path)
}

pub fn parse_files(paths: &[impl AsRef<Path>]) -> Result<Design> {
    Slang::default().parse_files(paths)
}
