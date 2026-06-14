use std::collections::HashSet;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

use crate::config::RuntimeConfig;

pub const SUGGEST_ENDPOINT: &str = "https://s.search.bilibili.com/main/suggest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionTerm {
    pub value: String,
}

pub fn search_suggestions(
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<SuggestionTerm>, BilibiliApiError> {
    let client = build_blocking_client(
        Some(config.user_agent.as_str()),
        Some(Duration::from_millis(config.timeout_ms)),
    )
    .map_err(|source| BilibiliApiError::Transport { source })?;

    let params = build_query_params(query, config);

    let response = client
        .get(SUGGEST_ENDPOINT)
        .query(&params)
        .send()
        .map_err(|source| BilibiliApiError::Transport { source })?;

    let status = response.status();
    let body = response
        .text()
        .map_err(|source| BilibiliApiError::Transport { source })?;

    parse_suggest_response(status.as_u16(), &body, config.max_results)
}

pub fn build_query_params(query: &str, config: &RuntimeConfig) -> Vec<(String, String)> {
    let mut params = vec![("term".to_string(), query.trim().to_string())];
    if let Some(uid) = &config.uid {
        params.push(("userid".to_string(), uid.clone()));
    }
    params
}

pub fn parse_suggest_response(
    http_status: u16,
    body: &str,
    max_results: u8,
) -> Result<Vec<SuggestionTerm>, BilibiliApiError> {
    if (400..=599).contains(&http_status) {
        return Err(BilibiliApiError::Http {
            status: http_status,
            message: "upstream suggest endpoint returned non-success status".to_string(),
        });
    }

    let payload: SuggestResponse =
        serde_json::from_str(body).map_err(BilibiliApiError::InvalidResponse)?;

    if payload.code != 0 {
        return Err(BilibiliApiError::Http {
            status: http_status,
            message: format!("bilibili suggest code {}", payload.code),
        });
    }

    let mut dedupe = HashSet::new();
    let mut results = Vec::new();
    let limit = usize::from(max_results.max(1));

    for row in payload
        .result
        .and_then(|result| result.tag)
        .unwrap_or_default()
        .into_iter()
    {
        let Some(value) = row
            .value
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
        else {
            continue;
        };

        let key = value.to_ascii_lowercase();
        if !dedupe.insert(key) {
            continue;
        }

        results.push(SuggestionTerm { value });
        if results.len() >= limit {
            break;
        }
    }

    Ok(results)
}

#[derive(Debug, Deserialize)]
struct SuggestResponse {
    #[serde(default)]
    code: i32,
    result: Option<SuggestResult>,
}

#[derive(Debug, Deserialize)]
struct SuggestResult {
    tag: Option<Vec<SuggestTag>>,
}

#[derive(Debug, Deserialize)]
struct SuggestTag {
    value: Option<String>,
}

#[derive(Debug, Error)]
pub enum BilibiliApiError {
    #[error("bilibili api request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("bilibili api error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid bilibili api response")]
    InvalidResponse(#[source] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RuntimeConfig;

    fn fixture_config() -> RuntimeConfig {
        RuntimeConfig {
            uid: None,
            max_results: 10,
            timeout_ms: 8000,
            user_agent: "nils-bilibili-cli/test".to_string(),
        }
    }

    #[test]
    fn bilibili_api_builds_expected_query_params() {
        let mut config = fixture_config();
        config.uid = Some("12345".to_string());

        let params = build_query_params(" naruto ", &config);
        assert_eq!(params[0], ("term".to_string(), "naruto".to_string()));
        assert_eq!(params[1], ("userid".to_string(), "12345".to_string()));
    }

    #[test]
    fn bilibili_api_builds_query_params_without_uid_when_missing() {
        let params = build_query_params("naruto", &fixture_config());
        assert_eq!(params, vec![("term".to_string(), "naruto".to_string())]);
    }

    #[test]
    fn bilibili_api_parser_dedupes_and_normalizes_terms() {
        let body = r#"{"code":0,"result":{"tag":[{"value":" naruto "},{"value":"Naruto"},{"value":"naruto mobile"},{"value":""}]}}"#;

        let results = parse_suggest_response(200, body, 10).expect("response should parse");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].value, "naruto");
        assert_eq!(results[1].value, "naruto mobile");
    }

    #[test]
    fn bilibili_api_parser_obeys_max_results_limit() {
        let body = r#"{"code":0,"result":{"tag":[{"value":"a"},{"value":"b"},{"value":"c"}]}}"#;

        let results = parse_suggest_response(200, body, 2).expect("response should parse");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].value, "a");
        assert_eq!(results[1].value, "b");
    }

    #[test]
    fn bilibili_api_parser_handles_missing_rows_as_empty_result() {
        let body = r#"{"code":0,"result":{}}"#;
        let results = parse_suggest_response(200, body, 10).expect("empty rows should be ok");
        assert!(results.is_empty());
    }

    #[test]
    fn bilibili_api_parser_surfaces_api_code_failures() {
        let body = r#"{"code":-1,"result":{}}"#;
        let err = parse_suggest_response(200, body, 10).expect_err("non-zero code should fail");
        assert!(matches!(err, BilibiliApiError::Http { .. }));
    }

    #[test]
    fn bilibili_api_parser_surfaces_http_status_failures() {
        let err = parse_suggest_response(503, "{}", 10).expect_err("http status should fail");
        match err {
            BilibiliApiError::Http { status, .. } => assert_eq!(status, 503),
            _ => panic!("expected http error"),
        }
    }

    #[test]
    fn bilibili_api_parser_rejects_invalid_json() {
        let err =
            parse_suggest_response(200, "not-json", 10).expect_err("invalid json should fail");
        assert!(matches!(err, BilibiliApiError::InvalidResponse(_)));
    }
}
