use base64::Engine as _;
use prost::Message;
use serde::Deserialize;
use thiserror::Error;

use crate::config::{RuntimeConfig, SteamSearchApi};

pub const SEARCH_SUGGESTIONS_ENDPOINT: &str =
    "https://api.steampowered.com/IStoreQueryService/SearchSuggestions/v1";
pub const STORE_SEARCH_ENDPOINT: &str = "https://store.steampowered.com/api/storesearch";
const SEARCH_SUGGESTIONS_ENDPOINT_ENV: &str = "STEAM_SEARCH_SUGGESTIONS_ENDPOINT";
const STORE_SEARCH_ENDPOINT_ENV: &str = "STEAM_STORE_SEARCH_ENDPOINT";
const SEARCH_ORIGIN: &str = "https://store.steampowered.com";
const USER_AGENT: &str = "nils-alfredworkflow-steam-search/0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamSearchResult {
    pub app_id: u32,
    pub name: String,
    pub price: Option<SteamPrice>,
    pub item_type: SteamItemType,
    pub platforms: SteamPlatforms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamItemType {
    Game,
    Demo,
    Dlc,
    Tool,
    Soundtrack,
    Application,
    Unknown,
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

impl SteamItemType {
    pub fn from_search_suggestions_code(code: u32) -> Self {
        match code {
            0 => Self::Game,
            1 => Self::Demo,
            4 => Self::Dlc,
            6 => Self::Tool,
            11 => Self::Soundtrack,
            _ => Self::Unknown,
        }
    }

    pub fn from_storesearch_type(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "app" => Self::Application,
            "demo" => Self::Demo,
            "dlc" => Self::Dlc,
            "tool" => Self::Tool,
            "soundtrack" => Self::Soundtrack,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Game => "Game",
            Self::Demo => "Demo",
            Self::Dlc => "DLC",
            Self::Tool => "Tool",
            Self::Soundtrack => "Soundtrack",
            Self::Application => "App",
            Self::Unknown => "Unknown",
        }
    }
}

pub fn search_apps(
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    let client = reqwest::blocking::Client::new();

    match config.search_api {
        SteamSearchApi::SearchSuggestions => {
            search_apps_with_search_suggestions(&client, config, query)
        }
        SteamSearchApi::StoreSearch => search_apps_with_store_search(&client, config, query),
    }
}

pub fn build_query_params(config: &RuntimeConfig, query: &str) -> Vec<(String, String)> {
    build_search_suggestions_query_params(config, query)
}

pub fn parse_search_response(
    status_code: u16,
    body: &[u8],
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    parse_search_suggestions_response(status_code, body)
}

fn search_apps_with_search_suggestions(
    client: &reqwest::blocking::Client,
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    let endpoint = resolve_endpoint(SEARCH_SUGGESTIONS_ENDPOINT_ENV, SEARCH_SUGGESTIONS_ENDPOINT);
    let params = build_search_suggestions_query_params(config, query);

    let response = client
        .get(endpoint)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::ORIGIN, SEARCH_ORIGIN)
        .query(&params)
        .send()
        .map_err(|source| SteamStoreApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .bytes()
        .map_err(|source| SteamStoreApiError::Transport { source })?
        .to_vec();

    parse_search_suggestions_response(status_code, &body)
}

fn search_apps_with_store_search(
    client: &reqwest::blocking::Client,
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    let endpoint = resolve_endpoint(STORE_SEARCH_ENDPOINT_ENV, STORE_SEARCH_ENDPOINT);
    let params = build_store_search_query_params(config, query);

    let response = client
        .get(endpoint)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .query(&params)
        .send()
        .map_err(|source| SteamStoreApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .bytes()
        .map_err(|source| SteamStoreApiError::Transport { source })?
        .to_vec();

    parse_store_search_response(status_code, &body)
}

fn build_search_suggestions_query_params(
    config: &RuntimeConfig,
    query: &str,
) -> Vec<(String, String)> {
    let request_payload = SearchSuggestionsRequest {
        context: Some(SearchBrowseContext {
            language: config.language.clone(),
            country_code: config.region.to_ascii_uppercase(),
        }),
        query: query.to_string(),
        max_results: config.max_results.into(),
        scope: String::new(),
        options: Some(SearchSuggestionsOptions {
            include_apps: true,
            include_associated_packages: true,
        }),
    };

    let encoded_payload =
        base64::engine::general_purpose::STANDARD.encode(request_payload.encode_to_vec());

    vec![
        ("origin".to_string(), SEARCH_ORIGIN.to_string()),
        ("input_protobuf_encoded".to_string(), encoded_payload),
    ]
}

fn build_store_search_query_params(config: &RuntimeConfig, query: &str) -> Vec<(String, String)> {
    let mut params = vec![
        ("term".to_string(), query.to_string()),
        ("cc".to_string(), config.region.clone()),
        ("json".to_string(), "1".to_string()),
        ("max_results".to_string(), config.max_results.to_string()),
    ];

    if !config.language.is_empty() {
        params.push(("l".to_string(), config.language.clone()));
    }

    params
}

fn parse_search_suggestions_response(
    status_code: u16,
    body: &[u8],
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(SteamStoreApiError::Http {
            status: status_code,
            message,
        });
    }

    let payload = SearchSuggestionsResponse::decode(body).map_err(|source| {
        SteamStoreApiError::InvalidResponse(ResponseDecodeError::Protobuf(source))
    })?;

    let results = payload
        .results
        .into_iter()
        .filter_map(|item| {
            let app_id = item.app_id?;
            if app_id == 0 {
                return None;
            }

            let name = item.name.trim().to_string();
            if name.is_empty() {
                return None;
            }

            let price = item.prices.into_iter().find_map(|candidate| {
                let final_formatted = candidate
                    .final_formatted
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());

                if candidate.final_price_cents.is_none() && final_formatted.is_none() {
                    return None;
                }

                Some(SteamPrice {
                    final_price_cents: candidate.final_price_cents,
                    final_formatted,
                })
            });

            let item_type =
                SteamItemType::from_search_suggestions_code(item.item_type_code.unwrap_or(0));

            // SearchSuggestions does not publish a stable platform contract.
            let platforms = SteamPlatforms::default();

            Some(SteamSearchResult {
                app_id,
                name,
                price,
                item_type,
                platforms,
            })
        })
        .collect();

    Ok(results)
}

fn parse_store_search_response(
    status_code: u16,
    body: &[u8],
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(SteamStoreApiError::Http {
            status: status_code,
            message,
        });
    }

    let body_text = std::str::from_utf8(body)
        .map_err(|source| SteamStoreApiError::InvalidResponse(ResponseDecodeError::Utf8(source)))?;
    let payload: StoreSearchResponse = serde_json::from_str(body_text)
        .map_err(|source| SteamStoreApiError::InvalidResponse(ResponseDecodeError::Json(source)))?;

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
            let item_type = SteamItemType::from_storesearch_type(&item.item_type);

            Some(SteamSearchResult {
                app_id,
                name,
                price,
                item_type,
                platforms,
            })
        })
        .collect();

    Ok(results)
}

fn resolve_endpoint(env_key: &str, default_endpoint: &str) -> String {
    std::env::var(env_key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_endpoint.to_string())
}

fn extract_error_message(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?.trim();
    if text.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return first_non_empty_string(&[
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
        ]);
    }

    Some(text.to_string())
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
    InvalidResponse(#[source] ResponseDecodeError),
}

#[derive(Debug, Error)]
pub enum ResponseDecodeError {
    #[error(transparent)]
    Protobuf(#[from] prost::DecodeError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionsRequest {
    #[prost(message, optional, tag = "2")]
    context: Option<SearchBrowseContext>,
    #[prost(string, tag = "3")]
    query: String,
    #[prost(uint32, tag = "4")]
    max_results: u32,
    #[prost(string, tag = "5")]
    scope: String,
    #[prost(message, optional, tag = "6")]
    options: Option<SearchSuggestionsOptions>,
}

#[derive(Clone, PartialEq, Message)]
struct SearchBrowseContext {
    #[prost(string, tag = "1")]
    language: String,
    #[prost(string, tag = "3")]
    country_code: String,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionsOptions {
    #[prost(bool, tag = "1")]
    include_apps: bool,
    #[prost(bool, tag = "16")]
    include_associated_packages: bool,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionsResponse {
    #[prost(message, repeated, tag = "3")]
    results: Vec<SearchSuggestionResult>,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionResult {
    #[prost(optional, uint32, tag = "2")]
    app_id: Option<u32>,
    #[prost(string, tag = "6")]
    name: String,
    #[prost(optional, uint32, tag = "10")]
    item_type_code: Option<u32>,
    #[prost(message, repeated, tag = "40")]
    prices: Vec<SearchSuggestionPrice>,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionPrice {
    #[prost(optional, uint32, tag = "5")]
    final_price_cents: Option<u32>,
    #[prost(optional, string, tag = "8")]
    final_formatted: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct StoreSearchResponse {
    #[serde(default)]
    items: Vec<StoreSearchItem>,
}

#[derive(Debug, Default, Deserialize)]
struct StoreSearchItem {
    #[serde(default)]
    id: Option<u32>,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    item_type: String,
    #[serde(default)]
    price: Option<StoreSearchPrice>,
    #[serde(default)]
    platforms: StoreSearchPlatformPayload,
}

#[derive(Debug, Default, Deserialize)]
struct StoreSearchPrice {
    #[serde(default, rename = "final")]
    final_price_cents: Option<u32>,
    #[serde(default)]
    final_formatted: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct StoreSearchPlatformPayload {
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
            show_region_options: false,
            max_results,
            language: language.to_string(),
            search_api: SteamSearchApi::SearchSuggestions,
        }
    }

    #[test]
    fn steam_store_api_build_query_params_includes_query_region_and_language_when_configured() {
        let params = build_query_params(&fixture_config("jp", "schinese", 7), "persona");
        let encoded_payload = params
            .iter()
            .find(|(key, _)| key == "input_protobuf_encoded")
            .map(|(_, value)| value)
            .expect("input_protobuf_encoded must exist");

        let payload_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded_payload)
            .expect("base64 payload should decode");
        let payload = SearchSuggestionsRequest::decode(payload_bytes.as_slice())
            .expect("protobuf should decode");
        let context = payload.context.expect("context must exist");

        assert!(params.contains(&("origin".to_string(), SEARCH_ORIGIN.to_string())));
        assert_eq!(context.language, "schinese");
        assert_eq!(context.country_code, "JP");
        assert_eq!(payload.query, "persona");
        assert_eq!(payload.max_results, 7);
        assert_eq!(payload.scope, "");

        let options = payload.options.expect("options must exist");
        assert!(options.include_apps);
        assert!(options.include_associated_packages);
    }

    #[test]
    fn steam_store_api_build_query_params_uses_empty_language_when_not_configured() {
        let params = build_query_params(&fixture_config("jp", "", 7), "persona");
        let encoded_payload = params
            .iter()
            .find(|(key, _)| key == "input_protobuf_encoded")
            .map(|(_, value)| value)
            .expect("input_protobuf_encoded must exist");

        let payload_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded_payload)
            .expect("base64 payload should decode");
        let payload = SearchSuggestionsRequest::decode(payload_bytes.as_slice())
            .expect("protobuf should decode");
        let context = payload.context.expect("context must exist");

        assert_eq!(context.language, "");
        assert_eq!(context.country_code, "JP");
    }

    #[test]
    fn steam_store_api_build_store_search_query_params_omits_language_when_empty() {
        let config = RuntimeConfig {
            language: "".to_string(),
            search_api: SteamSearchApi::StoreSearch,
            ..fixture_config("us", "english", 8)
        };
        let params = build_store_search_query_params(&config, "dota");

        assert!(params.contains(&("term".to_string(), "dota".to_string())));
        assert!(params.contains(&("cc".to_string(), "us".to_string())));
        assert!(params.contains(&("max_results".to_string(), "8".to_string())));
        assert!(!params.iter().any(|(key, _)| key == "l"));
    }

    #[test]
    fn steam_store_api_parse_search_response_extracts_expected_fields() {
        let body = SearchSuggestionsResponse {
            results: vec![SearchSuggestionResult {
                app_id: Some(730),
                name: "Counter-Strike 2".to_string(),
                item_type_code: Some(0),
                prices: vec![SearchSuggestionPrice {
                    final_price_cents: Some(0),
                    final_formatted: Some("Free".to_string()),
                }],
            }],
        }
        .encode_to_vec();

        let results = parse_search_response(200, body.as_slice()).expect("response should parse");

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
        assert_eq!(results[0].item_type, SteamItemType::Game);
        assert!(!results[0].platforms.windows);
        assert!(!results[0].platforms.mac);
        assert!(!results[0].platforms.linux);
    }

    #[test]
    fn steam_store_api_parse_store_search_response_extracts_expected_fields() {
        let body = br#"{
            "items": [
                {
                    "id": 730,
                    "name": "Counter-Strike 2",
                    "type": "app",
                    "price": {"final": 0, "final_formatted": "Free"},
                    "platforms": {"windows": true, "mac": false, "linux": true}
                }
            ]
        }"#;

        let results = parse_store_search_response(200, body).expect("response should parse");

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
        assert_eq!(results[0].item_type, SteamItemType::Application);
        assert!(results[0].platforms.windows);
        assert!(!results[0].platforms.mac);
        assert!(results[0].platforms.linux);
    }

    #[test]
    fn steam_store_api_parse_search_response_ignores_partial_items() {
        let body = SearchSuggestionsResponse {
            results: vec![
                SearchSuggestionResult {
                    app_id: Some(0),
                    name: "skip-id".to_string(),
                    item_type_code: Some(0),
                    prices: vec![],
                },
                SearchSuggestionResult {
                    app_id: Some(10),
                    name: "".to_string(),
                    item_type_code: Some(0),
                    prices: vec![],
                },
                SearchSuggestionResult {
                    app_id: None,
                    name: "missing-id".to_string(),
                    item_type_code: Some(0),
                    prices: vec![],
                },
                SearchSuggestionResult {
                    app_id: Some(570),
                    name: "Dota 2".to_string(),
                    item_type_code: Some(0),
                    prices: vec![],
                },
            ],
        }
        .encode_to_vec();

        let results = parse_search_response(200, body.as_slice()).expect("response should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, 570);
        assert_eq!(results[0].name, "Dota 2");
    }

    #[test]
    fn steam_store_api_parse_search_response_supports_empty_items() {
        let body = SearchSuggestionsResponse { results: vec![] }.encode_to_vec();
        let results =
            parse_search_response(200, body.as_slice()).expect("empty payload should parse");

        assert!(results.is_empty());
    }

    #[test]
    fn steam_store_api_parse_search_response_surfaces_api_error_message() {
        let body = br#"{"message":"upstream unavailable"}"#;
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
    fn steam_store_api_maps_search_suggestions_type_codes() {
        assert_eq!(
            SteamItemType::from_search_suggestions_code(0),
            SteamItemType::Game
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(1),
            SteamItemType::Demo
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(4),
            SteamItemType::Dlc
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(6),
            SteamItemType::Tool
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(11),
            SteamItemType::Soundtrack
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(999),
            SteamItemType::Unknown
        );
    }

    #[test]
    fn steam_store_api_maps_storesearch_type_values() {
        assert_eq!(
            SteamItemType::from_storesearch_type("app"),
            SteamItemType::Application
        );
        assert_eq!(
            SteamItemType::from_storesearch_type("demo"),
            SteamItemType::Demo
        );
        assert_eq!(
            SteamItemType::from_storesearch_type("dlc"),
            SteamItemType::Dlc
        );
        assert_eq!(
            SteamItemType::from_storesearch_type("unknown-type"),
            SteamItemType::Unknown
        );
    }

    #[test]
    fn steam_store_api_parse_search_response_rejects_malformed_success_payload() {
        let err =
            parse_search_response(200, b"not-protobuf").expect_err("invalid payload should fail");

        assert!(matches!(err, SteamStoreApiError::InvalidResponse(_)));
    }
}
