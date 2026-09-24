use std::path::PathBuf;

use serde_json::json;

use crate::error::AppError;

use super::client::DriveSession;
use super::{NativeDriveResponse, response};

pub fn execute_write(
    session: &DriveSession,
    action: &str,
    args: &[String],
) -> Result<NativeDriveResponse, AppError> {
    let (target, options) = parse_args(action, args)?;
    let file = match action {
        "mkdir" => session.create_folder(&target, required(&options.parent, "--parent")?)?,
        "update" => session.update_content(
            &target,
            PathBuf::from(required(&options.path, "<localPath>")?),
            options.mime.as_deref(),
        )?,
        "rename" => session.rename(&target, required(&options.name, "--name")?)?,
        "move" => session.move_file(
            &target,
            required(&options.parent, "--parent")?,
            required(&options.from, "--from")?,
        )?,
        "copy" => session.copy_file(
            &target,
            required(&options.parent, "--parent")?,
            options.name.as_deref(),
        )?,
        "trash" => session.set_trashed(&target, true)?,
        "untrash" => session.set_trashed(&target, false)?,
        _ => return Err(AppError::invalid_drive_input("unknown Drive write action")),
    };
    Ok(response(
        json!({"account": session.account, "account_source": session.account_source, "file": file}),
        format!("Drive {action} completed for `{}`.", file.id),
    ))
}

#[derive(Default)]
struct Options {
    parent: Option<String>,
    from: Option<String>,
    name: Option<String>,
    mime: Option<String>,
    path: Option<String>,
}

fn required<'a>(value: &'a Option<String>, flag: &str) -> Result<&'a str, AppError> {
    value
        .as_deref()
        .ok_or_else(|| AppError::invalid_drive_input(format!("missing {flag}")))
}

fn parse_args(action: &str, args: &[String]) -> Result<(String, Options), AppError> {
    let target = args.first().ok_or_else(|| {
        AppError::invalid_drive_input(format!("missing target for drive {action}"))
    })?;
    if target.is_empty()
        || (action != "mkdir" && target.starts_with('-'))
        || target.chars().any(char::is_control)
    {
        return Err(AppError::invalid_drive_input("invalid Drive write target"));
    }
    if action != "mkdir" && target.contains(['/', '?', '#']) {
        return Err(AppError::invalid_drive_input("invalid Drive file ID"));
    }
    let mut options = Options::default();
    let mut index = 1;
    if action == "update" {
        let path = args
            .get(index)
            .ok_or_else(|| AppError::invalid_drive_input("missing <localPath>"))?;
        if path.is_empty() || path.starts_with('-') {
            return Err(AppError::invalid_drive_input("invalid <localPath>"));
        }
        options.path = Some(path.clone());
        index += 1;
    }
    while index < args.len() {
        let flag = args[index].as_str();
        let slot = match flag {
            "--parent" if matches!(action, "mkdir" | "move" | "copy") => &mut options.parent,
            "--from" if action == "move" => &mut options.from,
            "--name" if matches!(action, "rename" | "copy") => &mut options.name,
            "--mime" if action == "update" => &mut options.mime,
            _ => {
                return Err(AppError::invalid_drive_input(format!(
                    "unknown drive {action} flag `{flag}`"
                )));
            }
        };
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| AppError::invalid_drive_input(format!("missing value for `{flag}`")))?;
        if value.is_empty()
            || (flag != "--name" && value.starts_with('-'))
            || value.chars().any(char::is_control)
        {
            return Err(AppError::invalid_drive_input(format!(
                "invalid value for `{flag}`"
            )));
        }
        if slot.replace(value.clone()).is_some() {
            return Err(AppError::invalid_drive_input(format!("duplicate `{flag}`")));
        }
        index += 1;
    }
    Ok((target.clone(), options))
}
