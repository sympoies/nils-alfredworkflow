use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

const ENDPOINT: &str = "https://zenquotes.io/api/random";
const USER_AGENT: &str = "nils-alfredworkflow-quote-feed/0.1.5";

pub fn fetch_quotes(fetch_count: usize, timeout_secs: u64) -> Result<Vec<String>, ZenQuotesError> {
    let client = build_blocking_client(None, Some(Duration::from_secs(timeout_secs)))
        .map_err(ZenQuotesError::BuildClient)?;

    let mut quotes = Vec::new();
    for _ in 0..fetch_count {
        if let Some(line) = fetch_single(&client)? {
            quotes.push(line);
        }
    }

    Ok(quotes)
}

fn fetch_single(client: &reqwest::blocking::Client) -> Result<Option<String>, ZenQuotesError> {
    let response = client
        .get(ENDPOINT)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .map_err(|source| ZenQuotesError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .map_err(|source| ZenQuotesError::Transport { source })?;

    parse_quote_line(status_code, &body)
}

pub fn parse_quote_line(status_code: u16, body: &str) -> Result<Option<String>, ZenQuotesError> {
    if !(200..=299).contains(&status_code) {
        return Err(ZenQuotesError::Http {
            status: status_code,
            message: format!("HTTP {status_code}"),
        });
    }

    let parsed: Vec<ApiQuote> =
        serde_json::from_str(body).map_err(ZenQuotesError::InvalidResponse)?;
    let Some(first) = parsed.first() else {
        return Ok(None);
    };

    let quote = collapse_spaces(first.q.trim());
    let author = collapse_spaces(first.a.trim());
    if quote.is_empty() || author.is_empty() {
        return Ok(None);
    }

    Ok(Some(format!("\"{quote}\" — {author}")))
}

fn collapse_spaces(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Error)]
pub enum ZenQuotesError {
    #[error("failed to build zenquotes client")]
    BuildClient(#[source] reqwest::Error),
    #[error("zenquotes request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("zenquotes api error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid zenquotes response")]
    InvalidResponse(#[source] serde_json::Error),
}

#[derive(Debug, Default, Deserialize)]
struct ApiQuote {
    #[serde(default)]
    q: String,
    #[serde(default)]
    a: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zenquotes_parse_extracts_quote_and_author_line() {
        let body = r#"[{"q":"Stay hungry","a":"Steve Jobs"}]"#;

        let parsed = parse_quote_line(200, body).expect("parse should succeed");
        assert_eq!(parsed, Some("\"Stay hungry\" — Steve Jobs".to_string()));
    }

    #[test]
    fn zenquotes_parse_returns_none_for_missing_fields() {
        let body = r#"[{"q":"","a":"Steve Jobs"}]"#;

        let parsed = parse_quote_line(200, body).expect("parse should succeed");
        assert_eq!(parsed, None);
    }

    #[test]
    fn zenquotes_parse_rejects_non_success_http_status() {
        let err = parse_quote_line(503, "{}").expect_err("503 should fail");
        assert!(matches!(err, ZenQuotesError::Http { status: 503, .. }));
    }

    #[test]
    fn zenquotes_parse_rejects_invalid_json() {
        let err = parse_quote_line(200, "not-json").expect_err("invalid json should fail");
        assert!(matches!(err, ZenQuotesError::InvalidResponse(_)));
    }
}
