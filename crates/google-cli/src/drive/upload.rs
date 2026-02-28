use std::path::PathBuf;

use serde_json::json;

use crate::error::AppError;

use super::client::{DriveSession, UploadRequest};
use super::{NativeDriveResponse, response};

pub fn execute_upload(
    session: &DriveSession,
    args: &[String],
) -> Result<NativeDriveResponse, AppError> {
    let request = parse_upload_args(args)?;
    let result = session.upload(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "upload": {
                "replaced": result.replaced,
                "replaced_file_id": result.replaced_file_id,
                "inferred_mime_type": result.inferred_mime_type,
                "source_path": result.source_path,
                "convert_requested": result.convert_requested,
                "file": result.file,
            },
        }),
        if result.replaced {
            format!("Replaced Drive file `{}`.", result.file.id)
        } else {
            format!("Uploaded `{}` to Drive.", result.file.name)
        },
    ))
}

fn parse_upload_args(args: &[String]) -> Result<UploadRequest, AppError> {
    let Some(local_path) = args.first() else {
        return Err(AppError::invalid_drive_input(
            "missing source path; expected `drive upload <localPath>`",
        ));
    };
    if local_path.starts_with('-') {
        return Err(AppError::invalid_drive_input(
            "missing source path; expected positional <localPath>",
        ));
    }

    let mut parent = None;
    let mut name = None;
    let mut mime_type = None;
    let mut replace = false;
    let mut convert = false;

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--parent" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--parent`"))?;
                parent = Some(value.clone());
            }
            "--name" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--name`"))?;
                name = Some(value.clone());
            }
            "--mime" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--mime`"))?;
                mime_type = Some(value.clone());
            }
            "--replace" => {
                replace = true;
            }
            "--convert" => {
                convert = true;
            }
            value if value.starts_with('-') => {
                return Err(AppError::invalid_drive_input(format!(
                    "unknown drive upload flag `{value}`"
                )));
            }
            value => {
                return Err(AppError::invalid_drive_input(format!(
                    "unexpected positional argument `{value}` for drive upload"
                )));
            }
        }

        index += 1;
    }

    Ok(UploadRequest {
        local_path: PathBuf::from(local_path),
        parent,
        name,
        mime_type,
        replace,
        convert,
    })
}
