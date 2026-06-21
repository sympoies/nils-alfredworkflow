use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

use crate::config::RuntimeConfig;

pub const SEARCH_ENDPOINT: &str = "https://api.spotify.com/v1/search";
/// Bound every request so a stalled server cannot hang the Alfred workflow.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackSearchResult {
    pub name: String,
    pub artists: Vec<String>,
    pub album_name: String,
    pub external_url: String,
}

pub fn search_tracks(
    config: &RuntimeConfig,
    access_token: &str,
    query: &str,
) -> Result<Vec<TrackSearchResult>, SpotifyApiError> {
    let client = build_blocking_client(None, Some(REQUEST_TIMEOUT))
        .map_err(|source| SpotifyApiError::Transport { source })?;
    let params = build_query_params(config, query);

    let response = client
        .get(SEARCH_ENDPOINT)
        .bearer_auth(access_token)
        .query(&params)
        .send()
        .map_err(|source| SpotifyApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .map_err(|source| SpotifyApiError::Transport { source })?;

    parse_search_response(status_code, &body)
}

pub fn build_query_params(config: &RuntimeConfig, query: &str) -> Vec<(String, String)> {
    let mut params = vec![
        ("q".to_string(), query.to_string()),
        ("type".to_string(), "track".to_string()),
        ("limit".to_string(), config.max_results.to_string()),
    ];

    if let Some(market) = &config.market {
        params.push(("market".to_string(), market.clone()));
    }

    params
}

pub fn parse_search_response(
    status_code: u16,
    body: &str,
) -> Result<Vec<TrackSearchResult>, SpotifyApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(SpotifyApiError::Http {
            status: status_code,
            message,
        });
    }

    let payload: SearchResponse =
        serde_json::from_str(body).map_err(SpotifyApiError::InvalidResponse)?;

    let tracks = payload
        .tracks
        .items
        .into_iter()
        .filter_map(|item| {
            let name = item.name.trim().to_string();
            if name.is_empty() {
                return None;
            }

            let external_url = item.external_urls.spotify?.trim().to_string();
            if external_url.is_empty() {
                return None;
            }

            let artists = item
                .artists
                .into_iter()
                .map(|artist| artist.name.trim().to_string())
                .filter(|artist| !artist.is_empty())
                .collect();

            Some(TrackSearchResult {
                name,
                artists,
                album_name: item.album.name.trim().to_string(),
                external_url,
            })
        })
        .collect();

    Ok(tracks)
}

fn extract_error_message(body: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;

    first_non_empty_string(&[
        value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str),
        value
            .get("error_description")
            .and_then(serde_json::Value::as_str),
        value.get("message").and_then(serde_json::Value::as_str),
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
pub enum SpotifyApiError {
    #[error("spotify api request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("spotify api error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid spotify api response")]
    InvalidResponse(#[source] serde_json::Error),
}

#[derive(Debug, Default, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    tracks: TrackPayload,
}

#[derive(Debug, Default, Deserialize)]
struct TrackPayload {
    #[serde(default)]
    items: Vec<TrackItem>,
}

#[derive(Debug, Default, Deserialize)]
struct TrackItem {
    #[serde(default)]
    name: String,
    #[serde(default)]
    artists: Vec<TrackArtist>,
    #[serde(default)]
    album: TrackAlbum,
    #[serde(default)]
    external_urls: ExternalUrls,
}

#[derive(Debug, Default, Deserialize)]
struct TrackArtist {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct TrackAlbum {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct ExternalUrls {
    #[serde(default)]
    spotify: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_config(market: Option<&str>) -> RuntimeConfig {
        RuntimeConfig {
            client_id: "demo-client".to_string(),
            client_secret: "demo-secret".to_string(),
            max_results: 7,
            market: market.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn spotify_api_build_query_params_follows_contract() {
        let params = build_query_params(&fixture_config(None), "daft punk");

        assert!(
            params.contains(&("q".to_string(), "daft punk".to_string())),
            "query parameter should include search term"
        );
        assert!(
            params.contains(&("type".to_string(), "track".to_string())),
            "type should be track"
        );
        assert!(
            params.contains(&("limit".to_string(), "7".to_string())),
            "limit should match runtime config"
        );
        assert!(
            !params.iter().any(|(name, _)| name == "market"),
            "market should be omitted when not configured"
        );
    }

    #[test]
    fn spotify_api_build_query_params_includes_market_when_present() {
        let params = build_query_params(&fixture_config(Some("US")), "daft punk");

        assert!(
            params.contains(&("market".to_string(), "US".to_string())),
            "market should be included when present"
        );
    }

    #[test]
    fn spotify_api_parse_search_response_extracts_track_fields() {
        let body = r#"{
            "tracks": {
                "items": [
                    {
                        "name": "Harder, Better, Faster, Stronger",
                        "artists": [{"name": "Daft Punk"}],
                        "album": {"name": "Discovery"},
                        "external_urls": {"spotify": "https://open.spotify.com/track/abc123"}
                    }
                ]
            }
        }"#;

        let tracks = parse_search_response(200, body).expect("response should parse");
        assert_eq!(tracks.len(), 1, "should parse one track");
        assert_eq!(tracks[0].name, "Harder, Better, Faster, Stronger");
        assert_eq!(tracks[0].artists, vec!["Daft Punk".to_string()]);
        assert_eq!(tracks[0].album_name, "Discovery");
        assert_eq!(
            tracks[0].external_url,
            "https://open.spotify.com/track/abc123"
        );
    }

    #[test]
    fn spotify_api_parse_search_response_ignores_tracks_missing_required_fields() {
        let body = r#"{
            "tracks": {
                "items": [
                    {
                        "name": "",
                        "artists": [{"name": "Daft Punk"}],
                        "album": {"name": "Discovery"},
                        "external_urls": {"spotify": "https://open.spotify.com/track/skip1"}
                    },
                    {
                        "name": "Skip URL",
                        "artists": [{"name": "Daft Punk"}],
                        "album": {"name": "Discovery"},
                        "external_urls": {}
                    },
                    {
                        "name": "Keep",
                        "artists": [{"name": "Daft Punk"}],
                        "album": {"name": "Discovery"},
                        "external_urls": {"spotify": "https://open.spotify.com/track/keep"}
                    }
                ]
            }
        }"#;

        let tracks = parse_search_response(200, body).expect("response should parse");
        assert_eq!(tracks.len(), 1, "only one valid track should remain");
        assert_eq!(tracks[0].name, "Keep");
    }

    #[test]
    fn spotify_api_parse_search_response_supports_empty_track_list() {
        let body = r#"{"tracks":{"items":[]}}"#;
        let tracks = parse_search_response(200, body).expect("empty payload should parse");
        assert!(
            tracks.is_empty(),
            "empty API response should map to empty list"
        );
    }

    #[test]
    fn spotify_api_parse_search_response_surfaces_api_error_message() {
        let body = r#"{
            "error": {
                "status": 429,
                "message": "API rate limit exceeded"
            }
        }"#;

        let err = parse_search_response(429, body).expect_err("non-2xx should fail");
        match err {
            SpotifyApiError::Http { status, message } => {
                assert_eq!(status, 429);
                assert_eq!(message, "API rate limit exceeded");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn spotify_api_parse_search_response_rejects_invalid_success_json() {
        let err =
            parse_search_response(200, "not-json").expect_err("invalid JSON payload should fail");

        assert!(
            matches!(err, SpotifyApiError::InvalidResponse(_)),
            "invalid success payload should produce InvalidResponse"
        );
    }
}
