use serde::Deserialize;
use thiserror::Error;

use crate::config::RuntimeConfig;

pub const SEARCH_ENDPOINT: &str = "https://store.steampowered.com/api/storesearch";
const SEARCH_ENDPOINT_ENV: &str = "STEAM_STORE_SEARCH_ENDPOINT";
const USER_AGENT: &str = "nils-alfredworkflow-steam-search/0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamSearchResult {
    pub app_id: u32,
    pub name: String,
    pub price: Option<SteamPrice>,
    pub platforms: SteamPlatforms,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamPrice {
    pub final_price_cents: Option<u32>,
    pub final_formatted: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SteamPlatforms {
    pub windows: bool,
    pub mac: bool,
    pub linux: bool,
}

pub fn search_apps(
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    let client = reqwest::blocking::Client::new();
    let endpoint = resolve_endpoint();
    let params = build_query_params(config, query);

    let response = client
        .get(endpoint)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .query(&params)
        .send()
        .map_err(|source| SteamStoreApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .map_err(|source| SteamStoreApiError::Transport { source })?;

    parse_search_response(status_code, &body)
}

pub fn build_query_params(config: &RuntimeConfig, query: &str) -> Vec<(String, String)> {
    vec![
        ("term".to_string(), query.to_string()),
        ("cc".to_string(), config.region.clone()),
        ("l".to_string(), config.language.clone()),
        ("json".to_string(), "1".to_string()),
        ("max_results".to_string(), config.max_results.to_string()),
    ]
}

pub fn parse_search_response(
    status_code: u16,
    body: &str,
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(SteamStoreApiError::Http {
            status: status_code,
            message,
        });
    }

    let payload: SearchResponse =
        serde_json::from_str(body).map_err(SteamStoreApiError::InvalidResponse)?;

    let results = payload
        .items
        .into_iter()
        .filter_map(|item| {
            let app_id = item.id?;
            if app_id == 0 {
                return None;
            }

            let name = item.name.trim().to_string();
            if name.is_empty() {
                return None;
            }

            let price = item.price.map(|price| SteamPrice {
                final_price_cents: price.final_price_cents,
                final_formatted: price
                    .final_formatted
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
            });

            let platforms = SteamPlatforms {
                windows: item.platforms.windows,
                mac: item.platforms.mac,
                linux: item.platforms.linux,
            };

            Some(SteamSearchResult {
                app_id,
                name,
                price,
                platforms,
            })
        })
        .collect();

    Ok(results)
}

fn resolve_endpoint() -> String {
    std::env::var(SEARCH_ENDPOINT_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| SEARCH_ENDPOINT.to_string())
}

fn extract_error_message(body: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;

    first_non_empty_string(&[
        value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str),
        value
            .get("error")
            .and_then(|error| error.get("detail"))
            .and_then(serde_json::Value::as_str),
        value.get("message").and_then(serde_json::Value::as_str),
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
pub enum SteamStoreApiError {
    #[error("steam store request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("steam store api error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid steam store response")]
    InvalidResponse(#[source] serde_json::Error),
}

#[derive(Debug, Default, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    items: Vec<SearchItem>,
}

#[derive(Debug, Default, Deserialize)]
struct SearchItem {
    #[serde(default)]
    id: Option<u32>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    price: Option<PricePayload>,
    #[serde(default)]
    platforms: PlatformPayload,
}

#[derive(Debug, Default, Deserialize)]
struct PricePayload {
    #[serde(default, rename = "final")]
    final_price_cents: Option<u32>,
    #[serde(default)]
    final_formatted: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PlatformPayload {
    #[serde(default)]
    windows: bool,
    #[serde(default)]
    mac: bool,
    #[serde(default)]
    linux: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_config(region: &str, language: &str, max_results: u8) -> RuntimeConfig {
        RuntimeConfig {
            region: region.to_string(),
            region_options: vec![region.to_string()],
            max_results,
            language: language.to_string(),
        }
    }

    #[test]
    fn steam_store_api_build_query_params_includes_query_region_and_language() {
        let params = build_query_params(&fixture_config("jp", "schinese", 7), "persona");

        assert!(params.contains(&("term".to_string(), "persona".to_string())));
        assert!(params.contains(&("cc".to_string(), "jp".to_string())));
        assert!(params.contains(&("l".to_string(), "schinese".to_string())));
        assert!(params.contains(&("json".to_string(), "1".to_string())));
        assert!(params.contains(&("max_results".to_string(), "7".to_string())));
    }

    #[test]
    fn steam_store_api_parse_search_response_extracts_expected_fields() {
        let body = r#"{
            "items": [
                {
                    "id": 730,
                    "name": "Counter-Strike 2",
                    "price": {"final": 0, "final_formatted": "Free"},
                    "platforms": {"windows": true, "mac": false, "linux": true}
                }
            ]
        }"#;

        let results = parse_search_response(200, body).expect("response should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, 730);
        assert_eq!(results[0].name, "Counter-Strike 2");
        assert_eq!(
            results[0].price,
            Some(SteamPrice {
                final_price_cents: Some(0),
                final_formatted: Some("Free".to_string()),
            })
        );
        assert!(results[0].platforms.windows);
        assert!(!results[0].platforms.mac);
        assert!(results[0].platforms.linux);
    }

    #[test]
    fn steam_store_api_parse_search_response_ignores_partial_items() {
        let body = r#"{
            "items": [
                {"id": 0, "name": "skip-id"},
                {"id": 10, "name": ""},
                {"name": "missing-id"},
                {"id": 570, "name": "Dota 2"}
            ]
        }"#;

        let results = parse_search_response(200, body).expect("response should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, 570);
        assert_eq!(results[0].name, "Dota 2");
    }

    #[test]
    fn steam_store_api_parse_search_response_supports_empty_items() {
        let body = r#"{"items":[]}"#;
        let results = parse_search_response(200, body).expect("empty payload should parse");

        assert!(results.is_empty());
    }

    #[test]
    fn steam_store_api_parse_search_response_surfaces_api_error_message() {
        let body = r#"{"message":"upstream unavailable"}"#;
        let err = parse_search_response(503, body).expect_err("non-2xx should fail");

        match err {
            SteamStoreApiError::Http { status, message } => {
                assert_eq!(status, 503);
                assert_eq!(message, "upstream unavailable");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn steam_store_api_parse_search_response_rejects_malformed_success_json() {
        let err =
            parse_search_response(200, "not-json").expect_err("invalid JSON payload should fail");

        assert!(matches!(err, SteamStoreApiError::InvalidResponse(_)));
    }
}
