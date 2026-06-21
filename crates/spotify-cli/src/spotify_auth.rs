use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

use crate::config::RuntimeConfig;

pub const TOKEN_ENDPOINT: &str = "https://accounts.spotify.com/api/token";
/// Bound every request so a stalled server cannot hang the Alfred workflow.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifyAccessToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

pub fn request_access_token(
    config: &RuntimeConfig,
) -> Result<SpotifyAccessToken, SpotifyAuthError> {
    let client = build_blocking_client(None, Some(REQUEST_TIMEOUT))
        .map_err(|source| SpotifyAuthError::Transport { source })?;

    let response = client
        .post(TOKEN_ENDPOINT)
        .basic_auth(
            config.client_id.as_str(),
            Some(config.client_secret.as_str()),
        )
        .form(&[("grant_type", "client_credentials")])
        .send()
        .map_err(|source| SpotifyAuthError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .map_err(|source| SpotifyAuthError::Transport { source })?;

    parse_token_response(status_code, &body)
}

pub fn parse_token_response(
    status_code: u16,
    body: &str,
) -> Result<SpotifyAccessToken, SpotifyAuthError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(SpotifyAuthError::Http {
            status: status_code,
            message,
        });
    }

    let payload: TokenResponse =
        serde_json::from_str(body).map_err(SpotifyAuthError::InvalidResponse)?;

    Ok(SpotifyAccessToken {
        access_token: payload.access_token,
        token_type: payload.token_type,
        expires_in: payload.expires_in,
    })
}

fn extract_error_message(body: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;

    first_non_empty_string(&[
        value
            .get("error_description")
            .and_then(serde_json::Value::as_str),
        value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str),
        value.get("message").and_then(serde_json::Value::as_str),
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
pub enum SpotifyAuthError {
    #[error("spotify auth request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("spotify auth error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid spotify auth response")]
    InvalidResponse(#[source] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spotify_auth_parse_token_response_extracts_access_token_fields() {
        let body = r#"{
            "access_token": "demo-token",
            "token_type": "Bearer",
            "expires_in": 3600
        }"#;

        let token = parse_token_response(200, body).expect("token response should parse");
        assert_eq!(token.access_token, "demo-token");
        assert_eq!(token.token_type, "Bearer");
        assert_eq!(token.expires_in, 3600);
    }

    #[test]
    fn spotify_auth_parse_token_response_surfaces_error_description() {
        let body = r#"{
            "error": "invalid_client",
            "error_description": "Invalid client"
        }"#;

        let err = parse_token_response(401, body).expect_err("non-2xx should fail");
        match err {
            SpotifyAuthError::Http { status, message } => {
                assert_eq!(status, 401);
                assert_eq!(message, "Invalid client");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn spotify_auth_parse_token_response_supports_nested_error_message() {
        let body = r#"{
            "error": {
                "message": "Service temporarily unavailable"
            }
        }"#;

        let err = parse_token_response(503, body).expect_err("non-2xx should fail");
        match err {
            SpotifyAuthError::Http { status, message } => {
                assert_eq!(status, 503);
                assert_eq!(message, "Service temporarily unavailable");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn spotify_auth_parse_token_response_uses_http_fallback_message() {
        let err = parse_token_response(500, "{}").expect_err("non-2xx should fail");
        match err {
            SpotifyAuthError::Http { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "HTTP 500");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn spotify_auth_parse_token_response_rejects_invalid_success_json() {
        let err =
            parse_token_response(200, "not-json").expect_err("invalid JSON payload should fail");

        assert!(
            matches!(err, SpotifyAuthError::InvalidResponse(_)),
            "invalid success payload should produce InvalidResponse"
        );
    }
}
