mod ast;
mod error;
mod slang;

pub use ast::*;
pub use error::{Error, Result};
pub use slang::Slang;

use std::path::Path;

pub fn parse_file(path: impl AsRef<Path>) -> Result<Design> {
    Slang::default().parse_file(path)
}

pub fn parse_files(paths: &[impl AsRef<Path>]) -> Result<Design> {
    Slang::default().parse_files(paths)
}
