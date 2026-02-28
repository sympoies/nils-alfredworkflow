use serde_json::json;

use crate::error::AppError;

use super::client::{DriveSession, GetRequest, ListRequest, SearchRequest};
use super::{NativeDriveResponse, response};

pub fn execute_ls(
    session: &DriveSession,
    args: &[String],
) -> Result<NativeDriveResponse, AppError> {
    let request = parse_ls_args(args)?;
    let files = session.list(&request);

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "parent": request.parent,
            "query": request.query,
            "max": request.max,
            "page_token": request.page_token,
            "count": files.len(),
            "files": files,
        }),
        format!("Listed {} Drive file(s).", files.len()),
    ))
}

pub fn execute_search(
    session: &DriveSession,
    args: &[String],
) -> Result<NativeDriveResponse, AppError> {
    let request = parse_search_args(args)?;
    let files = session.search(&request);

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "query": request.query,
            "raw_query": request.raw_query,
            "max": request.max,
            "page_token": request.page_token,
            "count": files.len(),
            "files": files,
        }),
        format!("Found {} Drive file(s).", files.len()),
    ))
}

pub fn execute_get(
    session: &DriveSession,
    args: &[String],
) -> Result<NativeDriveResponse, AppError> {
    let request = parse_get_args(args)?;
    let file = session.get(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "file": file,
        }),
        format!("Fetched Drive file `{}`.", request.file_id),
    ))
}

fn parse_ls_args(args: &[String]) -> Result<ListRequest, AppError> {
    let mut parent = None;
    let mut query = None;
    let mut max = 100usize;
    let mut page_token = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--parent" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--parent`"))?;
                parent = Some(value.clone());
            }
            "--query" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--query`"))?;
                query = Some(value.clone());
            }
            "--max" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--max`"))?;
                max = value.parse::<usize>().map_err(|_| {
                    AppError::invalid_drive_input(format!("invalid --max value `{value}`"))
                })?;
            }
            "--page" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--page`"))?;
                page_token = Some(value.clone());
            }
            value if value.starts_with('-') => {
                return Err(AppError::invalid_drive_input(format!(
                    "unknown drive ls flag `{value}`"
                )));
            }
            value => {
                return Err(AppError::invalid_drive_input(format!(
                    "unexpected positional argument `{value}` for drive ls"
                )));
            }
        }
        index += 1;
    }

    Ok(ListRequest {
        parent,
        query,
        max,
        page_token,
    })
}

fn parse_search_args(args: &[String]) -> Result<SearchRequest, AppError> {
    let mut query_tokens = Vec::new();
    let mut query = None;
    let mut max = 25usize;
    let mut page_token = None;
    let mut raw_query = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--query" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--query`"))?;
                query = Some(value.clone());
            }
            "--raw-query" => {
                raw_query = true;
            }
            "--max" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--max`"))?;
                max = value.parse::<usize>().map_err(|_| {
                    AppError::invalid_drive_input(format!("invalid --max value `{value}`"))
                })?;
            }
            "--page" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_drive_input("missing value for `--page`"))?;
                page_token = Some(value.clone());
            }
            value if value.starts_with('-') => {
                return Err(AppError::invalid_drive_input(format!(
                    "unknown drive search flag `{value}`"
                )));
            }
            value => query_tokens.push(value.to_string()),
        }

        index += 1;
    }

    let query = query
        .or_else(|| {
            if query_tokens.is_empty() {
                None
            } else {
                Some(query_tokens.join(" "))
            }
        })
        .ok_or_else(|| {
            AppError::invalid_drive_input(
                "missing query; expected `drive search <query>` or `drive search --query <query>`",
            )
        })?;

    Ok(SearchRequest {
        query,
        max,
        page_token,
        raw_query,
    })
}

fn parse_get_args(args: &[String]) -> Result<GetRequest, AppError> {
    let Some(file_id) = args.first() else {
        return Err(AppError::invalid_drive_input(
            "missing file id; expected `drive get <fileId>`",
        ));
    };
    if file_id.starts_with('-') {
        return Err(AppError::invalid_drive_input(
            "missing file id; expected positional <fileId>",
        ));
    }

    if args.len() > 1 {
        return Err(AppError::invalid_drive_input(format!(
            "unexpected positional or flag argument `{}` for drive get",
            args[1]
        )));
    }

    Ok(GetRequest {
        file_id: file_id.clone(),
    })
}
