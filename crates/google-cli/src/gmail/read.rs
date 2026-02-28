use serde_json::json;

use crate::error::AppError;

use super::client::{GetRequest, GmailSession, MessageFormat, SearchRequest};
use super::{NativeGmailResponse, response};

pub fn execute_search(
    session: &GmailSession,
    args: &[String],
) -> Result<NativeGmailResponse, AppError> {
    let request = parse_search_args(args)?;
    let messages = session.search(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "query": request.query,
            "format": request.format.as_str(),
            "max": request.max,
            "page_token": request.page_token,
            "count": messages.len(),
            "messages": messages,
        }),
        format!(
            "Found {} message(s) for `{}`.",
            messages.len(),
            request.query
        ),
    ))
}

pub fn execute_get(
    session: &GmailSession,
    args: &[String],
) -> Result<NativeGmailResponse, AppError> {
    let request = parse_get_args(args)?;
    let message = session.get(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "format": request.format.as_str(),
            "message": message,
        }),
        format!("Fetched message `{}`.", request.message_id),
    ))
}

fn parse_search_args(args: &[String]) -> Result<SearchRequest, AppError> {
    let mut query_tokens = Vec::new();
    let mut query = None;
    let mut max = 25usize;
    let mut page_token = None;
    let mut format = MessageFormat::Minimal;
    let mut headers = Vec::new();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--query" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_gmail_input("missing value for `--query`"))?;
                query = Some(value.clone());
            }
            "--max" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_gmail_input("missing value for `--max`"))?;
                max = value.parse::<usize>().map_err(|_| {
                    AppError::invalid_gmail_input(format!("invalid --max value `{value}`"))
                })?;
            }
            "--page" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| AppError::invalid_gmail_input("missing value for `--page`"))?;
                page_token = Some(value.clone());
            }
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
                    "unknown gmail search flag `{value}`"
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
            AppError::invalid_gmail_input(
                "missing query; expected `gmail search <query>` or `gmail search --query <query>`",
            )
        })?;

    Ok(SearchRequest {
        query,
        max,
        page_token,
        format,
        headers,
    })
}

fn parse_get_args(args: &[String]) -> Result<GetRequest, AppError> {
    let Some(first) = args.first() else {
        return Err(AppError::invalid_gmail_input(
            "missing message id; expected `gmail get <messageId>`",
        ));
    };
    if first.starts_with('-') {
        return Err(AppError::invalid_gmail_input(
            "missing message id; expected positional <messageId>",
        ));
    }

    let message_id = first.clone();
    let mut format = MessageFormat::Full;
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
                    "unknown gmail get flag `{value}`"
                )));
            }
            value => {
                return Err(AppError::invalid_gmail_input(format!(
                    "unexpected positional argument `{value}` for gmail get"
                )));
            }
        }
        index += 1;
    }

    Ok(GetRequest {
        message_id,
        format,
        headers,
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
