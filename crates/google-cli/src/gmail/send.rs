use std::path::PathBuf;

use serde_json::json;

use crate::error::AppError;

use super::client::GmailSession;
use super::mime::{ComposeRequest, compose_message};
use super::{NativeGmailResponse, response};

pub fn execute_send(
    session: &GmailSession,
    args: &[String],
) -> Result<NativeGmailResponse, AppError> {
    let request = parse_send_args(args)?;

    let composed = compose_message(&ComposeRequest {
        from: session.account.clone(),
        to: request.to.clone(),
        subject: request.subject.clone(),
        body: request.body.clone(),
        thread_id: request.thread_id.clone(),
        reply_to: request.reply_to.clone(),
        attachments: request.attachments.clone(),
    })?;

    let sent = session.send_raw_message(&composed.rfc822, request.thread_id.as_deref())?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "message": {
                "id": sent.id,
                "thread_id": sent.thread_id,
                "to": request.to,
                "subject": request.subject,
                "attachment_count": composed.attachments.len(),
                "attachments": composed.attachments,
                "mime_bytes": composed.rfc822.len(),
                "mime_preview": String::from_utf8_lossy(&composed.rfc822).chars().take(120).collect::<String>(),
            },
        }),
        "Sent Gmail message via native API path.",
    ))
}

#[derive(Debug, Clone)]
struct SendRequest {
    to: Vec<String>,
    subject: String,
    body: String,
    thread_id: Option<String>,
    reply_to: Option<String>,
    attachments: Vec<PathBuf>,
}

fn parse_send_args(args: &[String]) -> Result<SendRequest, AppError> {
    let mut to = Vec::new();
    let mut subject = None;
    let mut body = None;
    let mut body_file = None;
    let mut thread_id = None;
    let mut reply_to = None;
    let mut attachments = Vec::new();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--to" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_gmail_input("missing value for `--to`"))?;
                to.extend(parse_csv(value));
            }
            "--subject" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    AppError::invalid_gmail_input("missing value for `--subject`")
                })?;
                subject = Some(value.clone());
            }
            "--body" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_gmail_input("missing value for `--body`"))?;
                body = Some(value.clone());
            }
            "--body-file" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    AppError::invalid_gmail_input("missing value for `--body-file`")
                })?;
                body_file = Some(PathBuf::from(value));
            }
            "--thread-id" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    AppError::invalid_gmail_input("missing value for `--thread-id`")
                })?;
                thread_id = Some(value.clone());
            }
            "--reply-to" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    AppError::invalid_gmail_input("missing value for `--reply-to`")
                })?;
                reply_to = Some(value.clone());
            }
            "--attachment" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    AppError::invalid_gmail_input("missing value for `--attachment`")
                })?;
                attachments.push(PathBuf::from(value));
            }
            value if value.starts_with('-') => {
                return Err(AppError::invalid_gmail_input(format!(
                    "unknown gmail send flag `{value}`"
                )));
            }
            value => {
                return Err(AppError::invalid_gmail_input(format!(
                    "unexpected positional argument `{value}` for gmail send"
                )));
            }
        }

        index += 1;
    }

    if to.is_empty() {
        return Err(AppError::invalid_gmail_input(
            "gmail send requires at least one `--to <email>`",
        ));
    }

    let subject = subject
        .ok_or_else(|| AppError::invalid_gmail_input("gmail send requires `--subject <text>`"))?;

    let body = match (body, body_file) {
        (Some(text), None) => text,
        (None, Some(path)) => std::fs::read_to_string(&path).map_err(|error| {
            AppError::invalid_gmail_input(format!(
                "failed to read body file `{}`: {error}",
                path.display()
            ))
        })?,
        (Some(_), Some(_)) => {
            return Err(AppError::invalid_gmail_input(
                "use either `--body` or `--body-file`, not both",
            ));
        }
        (None, None) => {
            return Err(AppError::invalid_gmail_input(
                "gmail send requires `--body <text>` or `--body-file <path>`",
            ));
        }
    };

    Ok(SendRequest {
        to,
        subject,
        body,
        thread_id,
        reply_to,
        attachments,
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
