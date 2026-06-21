use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

use crate::config::RuntimeConfig;

pub const SEARCH_ENDPOINT: &str = "https://www.googleapis.com/youtube/v3/search";
/// Bound every request so a stalled server cannot hang the Alfred workflow.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoSearchResult {
    pub video_id: String,
    pub title: String,
    pub description: String,
}

pub fn search_videos(
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<VideoSearchResult>, YouTubeApiError> {
    let client = build_blocking_client(None, Some(REQUEST_TIMEOUT))
        .map_err(|source| YouTubeApiError::Transport { source })?;
    let params = build_query_params(config, query);

    let response = client
        .get(SEARCH_ENDPOINT)
        .query(&params)
        .send()
        .map_err(|source| YouTubeApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .map_err(|source| YouTubeApiError::Transport { source })?;

    parse_search_response(status_code, &body)
}

pub fn build_query_params(config: &RuntimeConfig, query: &str) -> Vec<(String, String)> {
    let mut params = vec![
        ("part".to_string(), "snippet".to_string()),
        ("type".to_string(), "video".to_string()),
        ("q".to_string(), query.to_string()),
        ("maxResults".to_string(), config.max_results.to_string()),
        ("key".to_string(), config.api_key.clone()),
    ];

    if let Some(region_code) = &config.region_code {
        params.push(("regionCode".to_string(), region_code.clone()));
    }

    params
}

pub fn parse_search_response(
    status_code: u16,
    body: &str,
) -> Result<Vec<VideoSearchResult>, YouTubeApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(YouTubeApiError::Http {
            status: status_code,
            message,
        });
    }

    let payload: SearchResponse =
        serde_json::from_str(body).map_err(YouTubeApiError::InvalidResponse)?;

    let videos = payload
        .items
        .into_iter()
        .filter_map(|item| {
            let video_id = item.id.video_id?.trim().to_string();
            if video_id.is_empty() {
                return None;
            }

            let title = item.snippet.title.trim().to_string();
            if title.is_empty() {
                return None;
            }

            Some(VideoSearchResult {
                video_id,
                title,
                description: item.snippet.description.trim().to_string(),
            })
        })
        .collect();

    Ok(videos)
}

fn extract_error_message(body: &str) -> Option<String> {
    let payload = serde_json::from_str::<ErrorEnvelope>(body).ok()?;
    let message = payload.error.message?.trim().to_string();
    if message.is_empty() {
        None
    } else {
        Some(message)
    }
}

#[derive(Debug, Error)]
pub enum YouTubeApiError {
    #[error("youtube api request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("youtube api error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid youtube api response")]
    InvalidResponse(#[source] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    items: Vec<SearchItem>,
}

#[derive(Debug, Default, Deserialize)]
struct SearchItem {
    #[serde(default)]
    id: SearchItemId,
    #[serde(default)]
    snippet: SearchSnippet,
}

#[derive(Debug, Default, Deserialize)]
struct SearchItemId {
    #[serde(rename = "videoId")]
    video_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SearchSnippet {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ErrorPayload,
}

#[derive(Debug, Deserialize)]
struct ErrorPayload {
    message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_config(region_code: Option<&str>) -> RuntimeConfig {
        RuntimeConfig {
            api_key: "demo-api-key".to_string(),
            max_results: 7,
            region_code: region_code.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn youtube_api_build_query_params_follows_contract() {
        let params = build_query_params(&fixture_config(None), "rust tutorial");

        assert!(
            params.contains(&("part".to_string(), "snippet".to_string())),
            "part should be snippet"
        );
        assert!(
            params.contains(&("type".to_string(), "video".to_string())),
            "type should be video"
        );
        assert!(
            params.contains(&("q".to_string(), "rust tutorial".to_string())),
            "query parameter should include search term"
        );
        assert!(
            params.contains(&("maxResults".to_string(), "7".to_string())),
            "maxResults should match runtime config"
        );
        assert!(
            params.contains(&("key".to_string(), "demo-api-key".to_string())),
            "key should include API key"
        );
        assert!(
            !params.iter().any(|(name, _)| name == "regionCode"),
            "regionCode should be omitted when not configured"
        );
    }

    #[test]
    fn youtube_api_build_query_params_includes_region_code_when_present() {
        let params = build_query_params(&fixture_config(Some("TW")), "rust tutorial");

        assert!(
            params.contains(&("regionCode".to_string(), "TW".to_string())),
            "regionCode should be included when present"
        );
    }

    #[test]
    fn youtube_api_parse_search_response_extracts_video_fields() {
        let body = r#"{
            "items": [
                {
                    "id": {"videoId": "abc123"},
                    "snippet": {
                        "title": "Rust Basics",
                        "description": " Learn Rust in one video. "
                    }
                }
            ]
        }"#;

        let videos = parse_search_response(200, body).expect("response should parse");
        assert_eq!(videos.len(), 1, "should parse one video");
        assert_eq!(videos[0].video_id, "abc123");
        assert_eq!(videos[0].title, "Rust Basics");
        assert_eq!(videos[0].description, "Learn Rust in one video.");
    }

    #[test]
    fn youtube_api_parse_search_response_ignores_items_missing_required_fields() {
        let body = r#"{
            "items": [
                {
                    "id": {},
                    "snippet": {"title": "Has no id", "description": "skip"}
                },
                {
                    "id": {"videoId": "abc123"},
                    "snippet": {"title": "", "description": "skip"}
                },
                {
                    "id": {"videoId": "xyz999"},
                    "snippet": {"title": "Keep", "description": "ok"}
                }
            ]
        }"#;

        let videos = parse_search_response(200, body).expect("response should parse");
        assert_eq!(videos.len(), 1, "only one valid item should remain");
        assert_eq!(videos[0].video_id, "xyz999");
    }

    #[test]
    fn youtube_api_parse_search_response_surfaces_api_error_message() {
        let body = r#"{
            "error": {
                "message": "API key not valid. Please pass a valid API key."
            }
        }"#;

        let err = parse_search_response(403, body).expect_err("non-2xx should fail");
        match err {
            YouTubeApiError::Http { status, message } => {
                assert_eq!(status, 403);
                assert_eq!(
                    message, "API key not valid. Please pass a valid API key.",
                    "error message should be extracted"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn youtube_api_parse_search_response_rejects_invalid_success_json() {
        let err =
            parse_search_response(200, "not-json").expect_err("invalid JSON payload should fail");

        assert!(
            matches!(err, YouTubeApiError::InvalidResponse(_)),
            "invalid success payload should produce InvalidResponse"
        );
    }
}
