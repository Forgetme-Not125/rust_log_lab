use std::fmt;
use std::io;
use std::num::ParseIntError;

/// Application-level error type.
///
/// The project does not use `unwrap` in normal business logic. Every fallible
/// operation is converted into this enum and returned by `Result<T, AppError>`.
#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    InvalidArgument(String),
    Parse(String),
    Thread(String),
    EmptyInput,
}

pub type AppResult<T> = Result<T, AppError>;

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(err) => write!(f, "I/O error: {err}"),
            AppError::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
            AppError::Parse(msg) => write!(f, "parse error: {msg}"),
            AppError::Thread(msg) => write!(f, "thread error: {msg}"),
            AppError::EmptyInput => write!(f, "input file is empty"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        AppError::Io(value)
    }
}

impl From<ParseIntError> for AppError {
    fn from(value: ParseIntError) -> Self {
        AppError::Parse(value.to_string())
    }
}
