//! The one error type for loading workloads.
//!
//! Why an enum instead of Box<dyn Error>: a Box hides what went wrong behind a
//! string, so tests could only compare messages. An enum lets tests match the
//! exact variant, and forces every variant to carry the path (and line number)
//! needed to find and fix the bad data.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum LoadError {
    /// A directory or loads file that should exist does not.
    MissingFile { path: PathBuf },
    /// Any other I/O failure (no permission, file is not valid UTF-8, ...).
    Io { path: PathBuf, source: io::Error },
    /// The workloads directory exists but has no day_NN folders in it.
    NoDays { path: PathBuf },
    /// A blank or whitespace only line.
    EmptyLine { path: PathBuf, line: usize },
    /// Not a whole number that fits in 64 bits, e.g. "12.5", "abc", "1e6".
    NotAnInteger {
        path: PathBuf,
        line: usize,
        text: String,
    },
    /// A whole number that is zero or negative. Requests must be positive.
    NotPositive {
        path: PathBuf,
        line: usize,
        value: i64,
    },
}

impl LoadError {
    /// Turns an io::Error into the right variant, so "file not found" is
    /// reported as MissingFile instead of a generic I/O failure.
    pub fn from_io(path: PathBuf, source: io::Error) -> Self {
        if source.kind() == io::ErrorKind::NotFound {
            LoadError::MissingFile { path }
        } else {
            LoadError::Io { path, source }
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::MissingFile { path } => {
                write!(f, "{}: file or directory not found", path.display())
            }
            LoadError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            LoadError::NoDays { path } => {
                write!(f, "{}: no day_NN folders found", path.display())
            }
            LoadError::EmptyLine { path, line } => {
                write!(f, "{}:{line}: empty line", path.display())
            }
            LoadError::NotAnInteger { path, line, text } => write!(
                f,
                "{}:{line}: {text:?} is not a whole number of bits",
                path.display()
            ),
            LoadError::NotPositive { path, line, value } => write!(
                f,
                "{}:{line}: request must be positive, got {value}",
                path.display()
            ),
        }
    }
}

impl Error for LoadError {
    /// Exposes the underlying io::Error so callers can walk the error chain.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            LoadError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
