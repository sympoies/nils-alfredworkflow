use std::collections::BTreeSet;

use serde_json::json;

use crate::error::AppError;

use super::client::{GmailSession, MessageFormat, ThreadGetRequest, ThreadModifyRequest};
use super::{NativeGmailResponse, response};

pub fn execute_thread(
    session: &GmailSession,
    args: &[String],
) -> Result<NativeGmailResponse, AppError> {
    let Some(action) = args.first() else {
        return Err(AppError::invalid_gmail_input(
            "missing thread action; expected `gmail thread get|modify ...`",
        ));
    };

    match action.as_str() {
        "get" => execute_thread_get(session, &args[1..]),
        "modify" => execute_thread_modify(session, &args[1..]),
        unknown => Err(AppError::invalid_gmail_input(format!(
            "unknown thread action `{unknown}`; expected get|modify"
        ))),
    }
}

fn execute_thread_get(
    session: &GmailSession,
    args: &[String],
) -> Result<NativeGmailResponse, AppError> {
    let request = parse_thread_get_args(args)?;
    let thread = session.thread_get(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "format": request.format.as_str(),
            "thread": thread,
        }),
        format!(
            "Fetched thread `{}` with {} message(s).",
            request.thread_id, thread.message_count
        ),
    ))
}

fn execute_thread_modify(
    session: &GmailSession,
    args: &[String],
) -> Result<NativeGmailResponse, AppError> {
    let request = parse_thread_modify_args(args)?;
    let result = session.thread_modify(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "thread": result,
        }),
        format!(
            "Modified labels for thread `{}` across {} message(s).",
            request.thread_id, result.modified_message_count
        ),
    ))
}

fn parse_thread_get_args(args: &[String]) -> Result<ThreadGetRequest, AppError> {
    let Some(thread_id) = args.first() else {
        return Err(AppError::invalid_gmail_input(
            "missing thread id; expected `gmail thread get <threadId>`",
        ));
    };
    if thread_id.starts_with('-') {
        return Err(AppError::invalid_gmail_input(
            "missing thread id; expected positional <threadId>",
        ));
    }

    let mut format = MessageFormat::Metadata;
    let mut headers = Vec::new();

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--format" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_gmail_input("missing value for `--format`"))?;
                format = MessageFormat::parse(value)?;
            }
            "--headers" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    AppError::invalid_gmail_input("missing value for `--headers`")
                })?;
                headers = parse_csv(value);
            }
            value if value.starts_with('-') => {
                return Err(AppError::invalid_gmail_input(format!(
                    "unknown gmail thread get flag `{value}`"
                )));
            }
            value => {
                return Err(AppError::invalid_gmail_input(format!(
                    "unexpected positional argument `{value}` for gmail thread get"
                )));
            }
        }
        index += 1;
    }

    Ok(ThreadGetRequest {
        thread_id: thread_id.clone(),
        format,
        headers,
    })
}

fn parse_thread_modify_args(args: &[String]) -> Result<ThreadModifyRequest, AppError> {
    let Some(thread_id) = args.first() else {
        return Err(AppError::invalid_gmail_input(
            "missing thread id; expected `gmail thread modify <threadId>`",
        ));
    };
    if thread_id.starts_with('-') {
        return Err(AppError::invalid_gmail_input(
            "missing thread id; expected positional <threadId>",
        ));
    }

    let mut add_labels = BTreeSet::new();
    let mut remove_labels = BTreeSet::new();

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--add-label" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    AppError::invalid_gmail_input("missing value for `--add-label`")
                })?;
                for label in parse_csv(value) {
                    add_labels.insert(label);
                }
            }
            "--remove-label" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    AppError::invalid_gmail_input("missing value for `--remove-label`")
                })?;
                for label in parse_csv(value) {
                    remove_labels.insert(label);
                }
            }
            value if value.starts_with('-') => {
                return Err(AppError::invalid_gmail_input(format!(
                    "unknown gmail thread modify flag `{value}`"
                )));
            }
            value => {
                return Err(AppError::invalid_gmail_input(format!(
                    "unexpected positional argument `{value}` for gmail thread modify"
                )));
            }
        }
        index += 1;
    }

    Ok(ThreadModifyRequest {
        thread_id: thread_id.clone(),
        add_labels: add_labels.into_iter().collect(),
        remove_labels: remove_labels.into_iter().collect(),
    })
}

fn parse_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
