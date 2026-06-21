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
    // An explicit `--out` is treated as intentional: the user typing an
    // absolute or relative path is honored verbatim. Only the
    // server-controlled file `name` is sanitized below to prevent path
    // traversal / arbitrary overwrite (e.g. a shared file named
    // `../../../x` or an absolute path).
    if let Some(path) = &request.out {
        return Ok(path.clone());
    }

    let fallback = if let Some(format) = &request.format {
        format!(
            "{}.{}",
            sanitize_file_stem(file_name, &request.file_id),
            format
        )
    } else {
        sanitize_server_file_name(file_name, &request.file_id)
    };

    let output = Path::new(&fallback).to_path_buf();
    if output.as_os_str().is_empty() {
        return Err(AppError::invalid_drive_input(
            "unable to derive output path; pass --out explicitly",
        ));
    }

    Ok(output)
}

/// Reduces a server-controlled file name to a single, safe path component.
///
/// Strips any directory components, then rejects results that are empty, `.`,
/// `..`, or that still contain a path separator. When the sanitized name is
/// unusable, falls back to the file id so the write always stays inside the
/// intended output directory.
fn sanitize_server_file_name(name: &str, fallback: &str) -> String {
    let candidate = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .unwrap_or("");

    let is_separator = |value: &str| value.contains('/') || value.contains('\\');
    if candidate.is_empty() || candidate == "." || candidate == ".." || is_separator(candidate) {
        return fallback.to_string();
    }

    candidate.to_string()
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

#[cfg(test)]
mod tests {
    use std::path::{Component, Path};

    use super::{DownloadRequest, resolve_output_path};

    fn request_without_out(file_id: &str) -> DownloadRequest {
        DownloadRequest {
            file_id: file_id.to_string(),
            out: None,
            format: None,
            overwrite: false,
        }
    }

    fn has_parent_escape(path: &Path) -> bool {
        path.components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    }

    #[test]
    fn resolve_output_path_strips_directory_traversal_from_server_name() {
        let request = request_without_out("file-123");
        let resolved = resolve_output_path("../escape.txt", &request).expect("resolve output path");

        assert_eq!(
            resolved.file_name().and_then(|value| value.to_str()),
            Some("escape.txt")
        );
        assert!(
            !has_parent_escape(&resolved),
            "resolved path must not escape the output directory: {}",
            resolved.display()
        );
    }

    #[test]
    fn resolve_output_path_keeps_plain_server_name() {
        let request = request_without_out("file-123");
        let resolved = resolve_output_path("report.csv", &request).expect("resolve output path");
        assert_eq!(resolved.to_str(), Some("report.csv"));
    }

    #[test]
    fn resolve_output_path_falls_back_to_file_id_for_unusable_name() {
        let request = request_without_out("file-123");
        let resolved = resolve_output_path("..", &request).expect("resolve output path");
        assert_eq!(resolved.to_str(), Some("file-123"));
    }

    #[test]
    fn resolve_output_path_honors_explicit_out_verbatim() {
        let mut request = request_without_out("file-123");
        request.out = Some(std::path::PathBuf::from("/abs/custom/path.txt"));
        let resolved = resolve_output_path("../escape.txt", &request).expect("resolve output path");
        assert_eq!(resolved.to_str(), Some("/abs/custom/path.txt"));
    }
}
