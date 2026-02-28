use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::error::AppError;

use super::client::DriveSession;
use super::{NativeDriveResponse, response};

#[derive(Debug, Clone, PartialEq, Eq)]
struct DownloadRequest {
    file_id: String,
    out: Option<PathBuf>,
    format: Option<String>,
    overwrite: bool,
}

pub fn execute_download(
    session: &DriveSession,
    args: &[String],
) -> Result<NativeDriveResponse, AppError> {
    let request = parse_download_args(args)?;
    let payload = session.resolve_download(&request.file_id, request.format.as_deref())?;
    let output_path = resolve_output_path(&payload.file_name, &request)?;

    if output_path.exists() && !request.overwrite {
        return Err(AppError::invalid_drive_input(format!(
            "output path `{}` already exists; pass --overwrite to replace",
            output_path.display()
        )));
    }

    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::drive_failure(format!(
                "failed creating output directory `{}`: {error}",
                parent.display()
            ))
        })?;
    }

    fs::write(&output_path, &payload.bytes).map_err(|error| {
        AppError::drive_failure(format!(
            "failed writing `{}`: {error}",
            output_path.display()
        ))
    })?;

    let output_path = output_path.canonicalize().unwrap_or(output_path);
    let action = if payload.source == "export" {
        "Exported"
    } else {
        "Downloaded"
    };
    let format_suffix = payload
        .format
        .as_deref()
        .map(|value| format!(" ({value})"))
        .unwrap_or_default();

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "file_id": payload.file_id,
            "file_name": payload.file_name,
            "mime_type": payload.mime_type,
            "source": payload.source,
            "format": payload.format,
            "bytes_written": payload.bytes.len(),
            "path": output_path.display().to_string(),
        }),
        format!(
            "{action} `{}`{format_suffix} to `{}`.",
            payload.file_id,
            output_path.display()
        ),
    ))
}

fn parse_download_args(args: &[String]) -> Result<DownloadRequest, AppError> {
    let Some(first) = args.first() else {
        return Err(AppError::invalid_drive_input(
            "missing file id; expected `drive download <fileId>`",
        ));
    };
    if first.starts_with('-') {
        return Err(AppError::invalid_drive_input(
            "missing file id; expected positional <fileId>",
        ));
    }

    let mut out = None;
    let mut format = None;
    let mut overwrite = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--out`"))?;
                out = Some(PathBuf::from(value));
            }
            "--format" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--format`"))?;
                if value.trim().is_empty() {
                    return Err(AppError::invalid_drive_input(
                        "empty `--format` value is not allowed",
                    ));
                }
                format = Some(value.clone());
            }
            "--overwrite" => overwrite = true,
            value if value.starts_with('-') => {
                return Err(AppError::invalid_drive_input(format!(
                    "unknown drive download flag `{value}`"
                )));
            }
            value => {
                return Err(AppError::invalid_drive_input(format!(
                    "unexpected positional argument `{value}` for drive download"
                )));
            }
        }
        index += 1;
    }

    Ok(DownloadRequest {
        file_id: first.clone(),
        out,
        format,
        overwrite,
    })
}

fn resolve_output_path(file_name: &str, request: &DownloadRequest) -> Result<PathBuf, AppError> {
    if let Some(path) = &request.out {
        return Ok(path.clone());
    }

    let fallback = if let Some(format) = &request.format {
        format!(
            "{}.{}",
            sanitize_file_stem(file_name, &request.file_id),
            format
        )
    } else if file_name.trim().is_empty() {
        request.file_id.clone()
    } else {
        file_name.to_string()
    };

    let output = Path::new(&fallback).to_path_buf();
    if output.as_os_str().is_empty() {
        return Err(AppError::invalid_drive_input(
            "unable to derive output path; pass --out explicitly",
        ));
    }

    Ok(output)
}

fn sanitize_file_stem(name: &str, fallback: &str) -> String {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if stem.trim().is_empty() {
        fallback.to_string()
    } else {
        stem.to_string()
    }
}
