use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};

use reqwest::Url;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;

use super::browser;
use super::callback::parse_callback_url;
use super::config::{OAuthClientCredentials, now_epoch_secs};
use super::store::StoredToken;
use crate::error::{AppError, redact_sensitive};

pub const GOOGLE_CLI_AUTH_TEST_CODE_ENV: &str = "GOOGLE_CLI_AUTH_TEST_CODE";
pub const GOOGLE_CLI_AUTH_TEST_CALLBACK_ENV: &str = "GOOGLE_CLI_AUTH_TEST_CALLBACK";
pub const GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE_ENV: &str = "GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE";

const GOOGLE_SCOPE: &str =
    "https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/drive";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFlowMode {
    Loopback,
    Manual,
    Remote,
}

impl AuthFlowMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthFlowMode::Loopback => "loopback",
            AuthFlowMode::Manual => "manual",
            AuthFlowMode::Remote => "remote",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthFlowPlan {
    pub mode: AuthFlowMode,
    pub account_hint: Option<String>,
}

impl AuthFlowPlan {
    pub fn new(mode: AuthFlowMode, account_hint: Option<String>) -> Self {
        Self { mode, account_hint }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopbackOutcome {
    pub token: StoredToken,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteStepOne {
    pub state: String,
    pub authorization_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

pub fn run_loopback(
    account: &str,
    credentials: &OAuthClientCredentials,
) -> Result<LoopbackOutcome, AppError> {
    let state = generate_state(account);
    let url = build_authorization_url(credentials, account, &state)?;
    browser::maybe_launch(&url)?;

    let code = if let Ok(callback) = env::var(GOOGLE_CLI_AUTH_TEST_CALLBACK_ENV) {
        let payload = parse_callback_url(&callback)?;
        if payload.state != state {
            return Err(AppError::auth_state_mismatch(&state, &payload.state));
        }
        payload.code
    } else if let Ok(code) = env::var(GOOGLE_CLI_AUTH_TEST_CODE_ENV) {
        code
    } else {
        return Err(AppError::invalid_auth_input(
            "loopback callback not received; re-run with `--manual --code <code>` or use `--remote --step 1` / `--remote --step 2`",
        ));
    };

    let token = exchange_code(account, &code, AuthFlowMode::Loopback, credentials)?;
    Ok(LoopbackOutcome { token, state })
}

pub fn run_manual(
    account: &str,
    code: &str,
    credentials: &OAuthClientCredentials,
) -> Result<StoredToken, AppError> {
    exchange_code(account, code, AuthFlowMode::Manual, credentials)
}

pub fn begin_remote(
    account: &str,
    credentials: &OAuthClientCredentials,
) -> Result<RemoteStepOne, AppError> {
    let state = generate_state(account);
    let authorization_url = build_authorization_url(credentials, account, &state)?;
    Ok(RemoteStepOne {
        state,
        authorization_url,
    })
}

pub fn finish_remote(
    account: &str,
    expected_state: &str,
    provided_state: &str,
    code: &str,
    credentials: &OAuthClientCredentials,
) -> Result<StoredToken, AppError> {
    if expected_state != provided_state {
        return Err(AppError::auth_state_mismatch(
            expected_state,
            provided_state,
        ));
    }
    exchange_code(account, code, AuthFlowMode::Remote, credentials)
}

pub fn build_authorization_url(
    credentials: &OAuthClientCredentials,
    account: &str,
    state: &str,
) -> Result<String, AppError> {
    let mut url = Url::parse(&credentials.auth_uri).map_err(|error| {
        AppError::invalid_auth_input(format!(
            "invalid auth URI `{}`: {error}",
            credentials.auth_uri
        ))
    })?;
    url.query_pairs_mut()
        .append_pair("client_id", &credentials.client_id)
        .append_pair("redirect_uri", &credentials.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", GOOGLE_SCOPE)
        .append_pair("state", state)
        .append_pair("login_hint", account)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("include_granted_scopes", "true");

    Ok(url.to_string())
}

pub fn generate_state(account: &str) -> String {
    let mut hasher = DefaultHasher::new();
    account.hash(&mut hasher);
    now_epoch_secs().hash(&mut hasher);
    format!("state-{:x}", hasher.finish())
}

pub fn refresh_access_token(
    _account: &str,
    credentials: &OAuthClientCredentials,
    token: &StoredToken,
) -> Result<StoredToken, AppError> {
    if allow_fake_exchange() {
        return Ok(fake_refresh(token));
    }

    let client = Client::new();
    let response = client
        .post(&credentials.token_uri)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", token.refresh_token.as_str()),
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
        ])
        .send()
        .map_err(|error| {
            AppError::auth_store_failure(format!("failed to refresh OAuth token: {error}"))
        })?;

    let payload = parse_token_response(response, "OAuth token refresh")?;
    Ok(StoredToken {
        access_token: payload.access_token,
        refresh_token: payload
            .refresh_token
            .unwrap_or_else(|| token.refresh_token.clone()),
        mode: token.mode.clone(),
        issued_at_epoch_secs: now_epoch_secs(),
    })
}

fn exchange_code(
    account: &str,
    code: &str,
    mode: AuthFlowMode,
    credentials: &OAuthClientCredentials,
) -> Result<StoredToken, AppError> {
    if allow_fake_exchange() {
        return Ok(fake_exchange_code(account, code, mode));
    }

    let client = Client::new();
    let response = client
        .post(&credentials.token_uri)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
            ("redirect_uri", credentials.redirect_uri.as_str()),
        ])
        .send()
        .map_err(|error| {
            AppError::auth_store_failure(format!("failed to exchange OAuth code: {error}"))
        })?;

    let payload = parse_token_response(response, "OAuth code exchange")?;
    let refresh_token = payload.refresh_token.ok_or_else(|| {
        AppError::auth_store_failure(
            "token response missing refresh token; re-run `auth add` and grant offline access",
        )
    })?;

    Ok(StoredToken {
        access_token: payload.access_token,
        refresh_token,
        mode: mode.as_str().to_string(),
        issued_at_epoch_secs: now_epoch_secs(),
    })
}

fn parse_token_response(response: Response, context: &str) -> Result<OAuthTokenResponse, AppError> {
    let status = response.status();
    let text = response.text().map_err(|error| {
        AppError::auth_store_failure(format!("failed to read OAuth response body: {error}"))
    })?;

    if !status.is_success() {
        let detail = extract_error_message(&text).unwrap_or(text);
        return Err(AppError::auth_store_failure(format!(
            "{context} failed with HTTP {}: {}",
            status.as_u16(),
            redact_sensitive(&detail)
        )));
    }

    serde_json::from_str::<OAuthTokenResponse>(&text).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to parse OAuth token response JSON: {error}"
        ))
    })
}

fn extract_error_message(body: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
    let message = parsed
        .get("error_description")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            parsed
                .get("error")
                .and_then(serde_json::Value::as_object)
                .and_then(|error| error.get("message"))
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| parsed.get("error").and_then(serde_json::Value::as_str))?;
    Some(message.to_string())
}

fn allow_fake_exchange() -> bool {
    env::var(GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE_ENV)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn fake_refresh(token: &StoredToken) -> StoredToken {
    StoredToken {
        access_token: token.access_token.clone(),
        refresh_token: token.refresh_token.clone(),
        mode: token.mode.clone(),
        issued_at_epoch_secs: now_epoch_secs(),
    }
}

fn fake_exchange_code(account: &str, code: &str, mode: AuthFlowMode) -> StoredToken {
    let mut hasher = DefaultHasher::new();
    account.hash(&mut hasher);
    code.hash(&mut hasher);

    let digest = hasher.finish();
    StoredToken {
        access_token: format!("access-{digest:x}"),
        refresh_token: format!("refresh-{digest:x}"),
        mode: mode.as_str().to_string(),
        issued_at_epoch_secs: now_epoch_secs(),
    }
}
