pub mod client;
pub mod read;
pub mod write;

use std::ffi::OsString;

use serde_json::Value;

use crate::cmd::common::{GlobalOptions, Invocation};
use crate::error::AppError;

use self::client::CalendarSession;

#[derive(Debug, Clone, PartialEq)]
pub struct NativeCalendarResponse {
    pub payload: Value,
    pub text: String,
}

pub fn execute_native(
    global: &GlobalOptions,
    invocation: &Invocation,
) -> Result<NativeCalendarResponse, AppError> {
    let Some(group) = invocation.path.get(1) else {
        return Err(AppError::invalid_calendar_input(
            "missing calendar subcommand; expected one of calendars/events",
        ));
    };

    let group = group.to_string_lossy().to_string();
    let args = os_strings_to_strings(&invocation.args);
    let (action, rest) = split_action(&args)?;

    let session = CalendarSession::from_global(global)?;

    match (group.as_str(), action.as_str()) {
        ("calendars", "list") => read::execute_calendars_list(&session, rest),
        ("events", "list") => read::execute_events_list(&session, rest),
        ("events", "get") => read::execute_events_get(&session, rest),
        ("events", "create") => write::execute_events_create(&session, rest),
        ("calendars", unknown) => Err(AppError::invalid_calendar_input(format!(
            "unknown calendars action `{unknown}`; expected list"
        ))),
        ("events", unknown) => Err(AppError::invalid_calendar_input(format!(
            "unknown events action `{unknown}`; expected list/get/create"
        ))),
        (unknown, _) => Err(AppError::invalid_calendar_input(format!(
            "unknown calendar subcommand `{unknown}`; expected calendars/events"
        ))),
    }
}

pub(crate) fn response(payload: Value, text: impl Into<String>) -> NativeCalendarResponse {
    NativeCalendarResponse {
        payload,
        text: text.into(),
    }
}

fn split_action(args: &[String]) -> Result<(String, &[String]), AppError> {
    let Some(first) = args.first() else {
        return Err(AppError::invalid_calendar_input(
            "missing calendar action; expected `calendars list`, `events list`, `events get`, or `events create`",
        ));
    };
    if first.starts_with('-') {
        return Err(AppError::invalid_calendar_input(format!(
            "expected a calendar action before flags, found `{first}`"
        )));
    }
    Ok((first.clone(), &args[1..]))
}

fn os_strings_to_strings(values: &[OsString]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.to_string_lossy().to_string())
        .collect()
}
