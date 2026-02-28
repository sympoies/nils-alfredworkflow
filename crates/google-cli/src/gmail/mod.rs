pub mod client;
pub mod mime;
pub mod read;
pub mod send;
pub mod thread;

use std::ffi::OsString;

use serde_json::Value;

use crate::cmd::common::{GlobalOptions, Invocation};
use crate::error::AppError;

use self::client::GmailSession;

#[derive(Debug, Clone, PartialEq)]
pub struct NativeGmailResponse {
    pub payload: Value,
    pub text: String,
}

pub fn execute_native(
    global: &GlobalOptions,
    invocation: &Invocation,
) -> Result<NativeGmailResponse, AppError> {
    let session = GmailSession::from_global(global)?;

    let Some(subcommand) = invocation.path.get(1) else {
        return Err(AppError::invalid_gmail_input(
            "missing gmail subcommand; expected one of search/get/send/thread",
        ));
    };

    let subcommand = subcommand.to_string_lossy().to_string();
    let args = os_strings_to_strings(&invocation.args);

    match subcommand.as_str() {
        "search" => read::execute_search(&session, &args),
        "get" => read::execute_get(&session, &args),
        "send" => send::execute_send(&session, &args),
        "thread" => thread::execute_thread(&session, &args),
        unknown => Err(AppError::invalid_gmail_input(format!(
            "unknown gmail subcommand `{unknown}`"
        ))),
    }
}

pub(crate) fn response(payload: Value, text: impl Into<String>) -> NativeGmailResponse {
    NativeGmailResponse {
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
