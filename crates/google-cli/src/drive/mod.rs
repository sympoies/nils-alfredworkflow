pub mod client;
pub mod download;
pub mod mime;
pub mod read;
pub mod upload;

use std::ffi::OsString;

use serde_json::Value;

use crate::cmd::common::{GlobalOptions, Invocation};
use crate::error::AppError;

use self::client::DriveSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveExecutionPath {
    GeneratedCrate,
    ReqwestFallback,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeDriveResponse {
    pub payload: Value,
    pub text: String,
}

pub fn execute_native(
    global: &GlobalOptions,
    invocation: &Invocation,
) -> Result<NativeDriveResponse, AppError> {
    let Some(subcommand) = invocation.path.get(1) else {
        return Err(AppError::invalid_drive_input(
            "missing drive subcommand; expected one of ls|search|get|download|upload",
        ));
    };

    let subcommand = subcommand.to_string_lossy().to_string();
    let session = DriveSession::from_global(global)?;
    let args = os_strings_to_strings(&invocation.args);

    match subcommand.as_str() {
        "ls" => read::execute_ls(&session, &args),
        "search" => read::execute_search(&session, &args),
        "get" => read::execute_get(&session, &args),
        "download" => download::execute_download(&session, &args),
        "upload" => upload::execute_upload(&session, &args),
        unknown => Err(AppError::invalid_drive_input(format!(
            "unknown drive subcommand `{unknown}`"
        ))),
    }
}

pub(crate) fn response(payload: Value, text: impl Into<String>) -> NativeDriveResponse {
    NativeDriveResponse {
        payload,
        text: text.into(),
    }
}

fn os_strings_to_strings(values: &[OsString]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.to_string_lossy().to_string())
        .collect()
}
