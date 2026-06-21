use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

use crate::config::RuntimeConfig;

pub const SEARCH_ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";
const AUTH_HEADER: &str = "X-Subscription-Token";
/// Bound every request so a stalled server cannot hang the Alfred workflow.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub description: String,
}

pub fn search_web(
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<WebSearchResult>, BraveApiError> {
    let client = build_blocking_client(None, Some(REQUEST_TIMEOUT))
        .map_err(|source| BraveApiError::Transport { source })?;
    let params = build_query_params(config, query);

    let response = client
        .get(SEARCH_ENDPOINT)
        .header(AUTH_HEADER, config.api_key.as_str())
        .query(&params)
        .send()
        .map_err(|source| BraveApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .map_err(|source| BraveApiError::Transport { source })?;

    parse_search_response(status_code, &body)
}

pub fn build_query_params(config: &RuntimeConfig, query: &str) -> Vec<(String, String)> {
    let mut params = vec![
        ("q".to_string(), query.to_string()),
        ("count".to_string(), config.count.to_string()),
        (
            "safesearch".to_string(),
            config.safesearch.as_str().to_string(),
        ),
    ];

    if let Some(country) = &config.country {
        params.push(("country".to_string(), country.clone()));
    }

    params
}

pub fn parse_search_response(
    status_code: u16,
    body: &str,
) -> Result<Vec<WebSearchResult>, BraveApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(BraveApiError::Http {
            status: status_code,
            message,
        });
    }

    let payload: SearchResponse =
        serde_json::from_str(body).map_err(BraveApiError::InvalidResponse)?;

    let results = payload
        .web
        .results
        .into_iter()
        .filter_map(|item| {
            let title = item.title.trim().to_string();
            let url = item.url.trim().to_string();
            if title.is_empty() || url.is_empty() {
                return None;
            }

            Some(WebSearchResult {
                title,
                url,
                description: item.description.trim().to_string(),
            })
        })
        .collect();

    Ok(results)
}

fn extract_error_message(body: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;

    first_non_empty_string(&[
        value.get("message").and_then(serde_json::Value::as_str),
        value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str),
        value
            .get("error")
            .and_then(|error| error.get("detail"))
            .and_then(serde_json::Value::as_str),
        value.get("detail").and_then(serde_json::Value::as_str),
        value.get("error").and_then(serde_json::Value::as_str),
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
pub enum BraveApiError {
    #[error("brave api request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("brave api error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid brave api response")]
    InvalidResponse(#[source] serde_json::Error),
}

#[derive(Debug, Default, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    web: WebPayload,
}

#[derive(Debug, Default, Deserialize)]
struct WebPayload {
    #[serde(default)]
    results: Vec<WebResultItem>,
}

#[derive(Debug, Default, Deserialize)]
struct WebResultItem {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    description: String,
}

#[cfg(test)]
mod tests {
    use crate::config::{RuntimeConfig, SafeSearch};

    use super::*;

    fn fixture_config(country: Option<&str>) -> RuntimeConfig {
        RuntimeConfig {
            api_key: "demo-api-key".to_string(),
            count: 7,
            safesearch: SafeSearch::Moderate,
            country: country.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn brave_api_build_query_params_follows_contract() {
        let params = build_query_params(&fixture_config(None), "rust tutorial");

        assert!(
            params.contains(&("q".to_string(), "rust tutorial".to_string())),
            "query parameter should include search term"
        );
        assert!(
            params.contains(&("count".to_string(), "7".to_string())),
            "count should match runtime config"
        );
        assert!(
            params.contains(&("safesearch".to_string(), "moderate".to_string())),
            "safesearch should match runtime config"
        );
        assert!(
            !params.iter().any(|(name, _)| name == "country"),
            "country should be omitted when not configured"
        );
    }

    #[test]
    fn brave_api_build_query_params_includes_country_when_present() {
        let params = build_query_params(&fixture_config(Some("US")), "rust tutorial");

        assert!(
            params.contains(&("country".to_string(), "US".to_string())),
            "country should be included when present"
        );
    }

    #[test]
    fn brave_api_parse_search_response_extracts_result_fields() {
        let body = r#"{
            "web": {
                "results": [
                    {
                        "title": "Rust Book",
                        "url": "https://doc.rust-lang.org/book/",
                        "description": " Learn Rust from official docs. "
                    }
                ]
            }
        }"#;

        let results = parse_search_response(200, body).expect("response should parse");
        assert_eq!(results.len(), 1, "should parse one result");
        assert_eq!(results[0].title, "Rust Book");
        assert_eq!(results[0].url, "https://doc.rust-lang.org/book/");
        assert_eq!(results[0].description, "Learn Rust from official docs.");
    }

    #[test]
    fn brave_api_parse_search_response_ignores_results_missing_required_fields() {
        let body = r#"{
            "web": {
                "results": [
                    {
                        "title": "",
                        "url": "https://example.com/skip-title",
                        "description": "skip"
                    },
                    {
                        "title": "Skip URL",
                        "url": "",
                        "description": "skip"
                    },
                    {
                        "title": "Keep",
                        "url": "https://example.com/keep",
                        "description": "ok"
                    }
                ]
            }
        }"#;

        let results = parse_search_response(200, body).expect("response should parse");
        assert_eq!(results.len(), 1, "only one valid item should remain");
        assert_eq!(results[0].title, "Keep");
    }

    #[test]
    fn brave_api_parse_search_response_surfaces_api_error_message() {
        let body = r#"{
            "error": {
                "message": "Invalid subscription token"
            }
        }"#;

        let err = parse_search_response(401, body).expect_err("non-2xx should fail");
        match err {
            BraveApiError::Http { status, message } => {
                assert_eq!(status, 401);
                assert_eq!(message, "Invalid subscription token");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn brave_api_parse_search_response_rejects_invalid_success_json() {
        let err =
            parse_search_response(200, "not-json").expect_err("invalid JSON payload should fail");

        assert!(
            matches!(err, BraveApiError::InvalidResponse(_)),
            "invalid success payload should produce InvalidResponse"
        );
    }
}
