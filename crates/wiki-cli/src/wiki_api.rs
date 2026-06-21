use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

use crate::config::RuntimeConfig;

const USER_AGENT: &str = "nils-alfredworkflow-wiki-search/0.1.5";
/// Bound every request so a stalled server cannot hang the Alfred workflow.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiSearchResult {
    pub title: String,
    pub snippet: String,
    pub pageid: u64,
}

pub fn search_articles(
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<WikiSearchResult>, WikiApiError> {
    let client = build_blocking_client(Some(USER_AGENT), Some(REQUEST_TIMEOUT))
        .map_err(|source| WikiApiError::Transport { source })?;
    let endpoint = build_endpoint(config);
    let params = build_query_params(config, query);

    let response = client
        .get(endpoint)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .query(&params)
        .send()
        .map_err(|source| WikiApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .map_err(|source| WikiApiError::Transport { source })?;

    parse_search_response(status_code, &body)
}

pub fn build_endpoint(config: &RuntimeConfig) -> String {
    format!("https://{}.wikipedia.org/w/api.php", config.language)
}

pub fn build_query_params(config: &RuntimeConfig, query: &str) -> Vec<(String, String)> {
    vec![
        ("action".to_string(), "query".to_string()),
        ("list".to_string(), "search".to_string()),
        ("format".to_string(), "json".to_string()),
        ("utf8".to_string(), "1".to_string()),
        ("srsearch".to_string(), query.to_string()),
        ("srlimit".to_string(), config.max_results.to_string()),
        ("srprop".to_string(), "snippet".to_string()),
    ]
}

pub fn parse_search_response(
    status_code: u16,
    body: &str,
) -> Result<Vec<WikiSearchResult>, WikiApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(WikiApiError::Http {
            status: status_code,
            message,
        });
    }

    let payload: SearchResponse =
        serde_json::from_str(body).map_err(WikiApiError::InvalidResponse)?;

    let results = payload
        .query
        .search
        .into_iter()
        .filter_map(|item| {
            let title = item.title.trim().to_string();
            if title.is_empty() || item.pageid == 0 {
                return None;
            }

            Some(WikiSearchResult {
                title,
                snippet: item.snippet.trim().to_string(),
                pageid: item.pageid,
            })
        })
        .collect();

    Ok(results)
}

fn extract_error_message(body: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;

    first_non_empty_string(&[
        value
            .get("error")
            .and_then(|error| error.get("info"))
            .and_then(serde_json::Value::as_str),
        value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str),
        value.get("message").and_then(serde_json::Value::as_str),
        value.get("detail").and_then(serde_json::Value::as_str),
    ])
}

fn first_non_empty_string(candidates: &[Option<&str>]) -> Option<String> {
    candidates
        .iter()
        .flatten()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[derive(Debug, Error)]
pub enum WikiApiError {
    #[error("wikipedia api request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("wikipedia api error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid wikipedia api response")]
    InvalidResponse(#[source] serde_json::Error),
}

#[derive(Debug, Default, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    query: QueryPayload,
}

#[derive(Debug, Default, Deserialize)]
struct QueryPayload {
    #[serde(default)]
    search: Vec<SearchItem>,
}

#[derive(Debug, Default, Deserialize)]
struct SearchItem {
    #[serde(default)]
    title: String,
    #[serde(default)]
    snippet: String,
    #[serde(default)]
    pageid: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_config(language: &str, max_results: u8) -> RuntimeConfig {
        RuntimeConfig {
            language: language.to_string(),
            language_options: vec![language.to_string()],
            max_results,
        }
    }

    #[test]
    fn wiki_api_build_endpoint_uses_runtime_language_host() {
        let endpoint = build_endpoint(&fixture_config("zh", 10));

        assert_eq!(endpoint, "https://zh.wikipedia.org/w/api.php");
    }

    #[test]
    fn wiki_api_build_query_params_follows_contract() {
        let params = build_query_params(&fixture_config("en", 7), "rust language");

        assert!(params.contains(&("action".to_string(), "query".to_string())));
        assert!(params.contains(&("list".to_string(), "search".to_string())));
        assert!(params.contains(&("format".to_string(), "json".to_string())));
        assert!(params.contains(&("utf8".to_string(), "1".to_string())));
        assert!(params.contains(&("srsearch".to_string(), "rust language".to_string())));
        assert!(params.contains(&("srlimit".to_string(), "7".to_string())));
        assert!(params.contains(&("srprop".to_string(), "snippet".to_string())));
    }

    #[test]
    fn wiki_api_parse_search_response_extracts_result_fields() {
        let body = r#"{
            "query": {
                "search": [
                    {
                        "title": "Rust (programming language)",
                        "snippet": "A language empowering everyone",
                        "pageid": 36192
                    }
                ]
            }
        }"#;

        let results = parse_search_response(200, body).expect("response should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust (programming language)");
        assert_eq!(results[0].snippet, "A language empowering everyone");
        assert_eq!(results[0].pageid, 36192);
    }

    #[test]
    fn wiki_api_parse_search_response_ignores_results_missing_required_fields() {
        let body = r#"{
            "query": {
                "search": [
                    {"title": "", "snippet": "missing title", "pageid": 12},
                    {"title": "missing page id", "snippet": "...", "pageid": 0},
                    {"title": "valid", "snippet": "ok", "pageid": 1}
                ]
            }
        }"#;

        let results = parse_search_response(200, body).expect("response should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "valid");
        assert_eq!(results[0].pageid, 1);
    }

    #[test]
    fn wiki_api_parse_search_response_surfaces_api_error_message() {
        let body = r#"{
            "error": {
                "code": "badrequest",
                "info": "Invalid value for parameter srlimit"
            }
        }"#;

        let err = parse_search_response(400, body).expect_err("non-2xx should fail");

        match err {
            WikiApiError::Http { status, message } => {
                assert_eq!(status, 400);
                assert_eq!(message, "Invalid value for parameter srlimit");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn wiki_api_parse_search_response_rejects_invalid_success_json() {
        let err =
            parse_search_response(200, "not-json").expect_err("invalid JSON payload should fail");

        assert!(matches!(err, WikiApiError::InvalidResponse(_)));
    }
}
