use crate::{convert::ConvertError, parser::ParseError};

pub use workflow_common::{AppError, CliErrorKind as ErrorKind};

pub const ERROR_CODE_USER: &str = "NILS_EPOCH_001";
pub const ERROR_CODE_RUNTIME: &str = "NILS_EPOCH_002";

impl From<ParseError> for AppError {
    fn from(error: ParseError) -> Self {
        AppError::user(ERROR_CODE_USER, error.to_string())
    }
}

impl From<ConvertError> for AppError {
    fn from(error: ConvertError) -> Self {
        AppError::user(ERROR_CODE_USER, error.to_string())
    }
}
