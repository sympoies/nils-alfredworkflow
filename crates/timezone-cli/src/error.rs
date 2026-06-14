use crate::parser::ParseError;

pub use workflow_common::{AppError, CliErrorKind as ErrorKind};

pub const ERROR_CODE_USER: &str = "NILS_TIMEZONE_001";
pub const ERROR_CODE_RUNTIME: &str = "NILS_TIMEZONE_002";

impl From<ParseError> for AppError {
    fn from(error: ParseError) -> Self {
        AppError::user(ERROR_CODE_USER, error.to_string())
    }
}
