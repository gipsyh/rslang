use std::error::Error as StdError;
use std::fmt;
use std::process::ExitStatus;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    SlangFailed {
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    MissingField {
        field: &'static str,
        context: String,
    },
    UnexpectedKind {
        expected: &'static str,
        actual: Option<String>,
    },
    Message(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Json(err) => write!(f, "JSON error: {err}"),
            Self::SlangFailed {
                status,
                stdout,
                stderr,
            } => {
                write!(f, "slang failed with {status}")?;
                if !stderr.trim().is_empty() {
                    write!(f, "\nstderr:\n{stderr}")?;
                }
                if !stdout.trim().is_empty() {
                    write!(f, "\nstdout:\n{stdout}")?;
                }
                Ok(())
            }
            Self::MissingField { field, context } => {
                write!(f, "missing field `{field}` while lowering {context}")
            }
            Self::UnexpectedKind { expected, actual } => {
                write!(f, "expected kind `{expected}`, got {:?}", actual)
            }
            Self::Message(msg) => f.write_str(msg),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Json(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
