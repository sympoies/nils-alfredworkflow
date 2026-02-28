use std::path::{Path, PathBuf};

use mail_builder::MessageBuilder;
use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct ComposeRequest {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
    pub thread_id: Option<String>,
    pub reply_to: Option<String>,
    pub attachments: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttachmentSummary {
    pub file: String,
    pub content_type: String,
    pub bytes: usize,
}

#[derive(Debug, Clone)]
pub struct ComposeResult {
    pub rfc822: Vec<u8>,
    pub attachments: Vec<AttachmentSummary>,
}

pub fn compose_message(request: &ComposeRequest) -> Result<ComposeResult, AppError> {
    if request.to.is_empty() {
        return Err(AppError::invalid_gmail_input(
            "gmail send requires at least one --to recipient",
        ));
    }

    let mut builder = MessageBuilder::new()
        .from(("google-cli", request.from.as_str()))
        .subject(request.subject.as_str())
        .text_body(request.body.as_str());

    for recipient in &request.to {
        builder = builder.to(recipient.as_str());
    }

    if let Some(reply_to) = &request.reply_to {
        builder = builder.reply_to(reply_to.as_str());
    }

    if let Some(thread_id) = &request.thread_id {
        builder = builder
            .in_reply_to(thread_id.as_str())
            .references(thread_id.as_str());
    }

    let mut attachments = Vec::new();
    for path in &request.attachments {
        let bytes = std::fs::read(path).map_err(|error| {
            AppError::invalid_gmail_input(format!(
                "failed to read attachment `{}`: {error}",
                path.display()
            ))
        })?;

        let file_name = file_name(path)?;
        let content_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_string();

        attachments.push(AttachmentSummary {
            file: file_name.clone(),
            content_type: content_type.clone(),
            bytes: bytes.len(),
        });

        builder = builder.attachment(content_type, file_name, bytes);
    }

    let rfc822 = builder.write_to_vec().map_err(|error| {
        AppError::gmail_failure(format!("failed to build MIME message: {error}"))
    })?;

    Ok(ComposeResult {
        rfc822,
        attachments,
    })
}

fn file_name(path: &Path) -> Result<String, AppError> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            AppError::invalid_gmail_input(format!(
                "attachment path `{}` has no valid file name",
                path.display()
            ))
        })
}
