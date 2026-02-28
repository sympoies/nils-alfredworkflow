use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};

use super::browser;
use super::callback::parse_callback_url;
use super::config::{OAuthClientCredentials, now_epoch_secs};
use super::store::StoredToken;
use crate::error::AppError;

pub const GOOGLE_CLI_AUTH_TEST_CODE_ENV: &str = "GOOGLE_CLI_AUTH_TEST_CODE";
pub const GOOGLE_CLI_AUTH_TEST_CALLBACK_ENV: &str = "GOOGLE_CLI_AUTH_TEST_CALLBACK";

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

pub fn run_loopback(
    account: &str,
    credentials: &OAuthClientCredentials,
) -> Result<LoopbackOutcome, AppError> {
    let state = generate_state(account);
    let url = build_authorization_url(credentials, account, &state);
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
            "loopback callback not received; re-run with `--manual --code <code>` or provide callback",
        ));
    };

    let token = exchange_code(account, &code, AuthFlowMode::Loopback);
    Ok(LoopbackOutcome { token, state })
}

pub fn run_manual(account: &str, code: &str) -> StoredToken {
    exchange_code(account, code, AuthFlowMode::Manual)
}

pub fn begin_remote(
    account: &str,
    credentials: &OAuthClientCredentials,
) -> Result<RemoteStepOne, AppError> {
    let state = generate_state(account);
    let authorization_url = build_authorization_url(credentials, account, &state);
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
) -> Result<StoredToken, AppError> {
    if expected_state != provided_state {
        return Err(AppError::auth_state_mismatch(
            expected_state,
            provided_state,
        ));
    }
    Ok(exchange_code(account, code, AuthFlowMode::Remote))
}

pub fn build_authorization_url(
    credentials: &OAuthClientCredentials,
    account: &str,
    state: &str,
) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&login_hint={}",
        credentials.auth_uri,
        credentials.client_id,
        credentials.redirect_uri,
        "https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/drive",
        state,
        account,
    )
}

pub fn generate_state(account: &str) -> String {
    let mut hasher = DefaultHasher::new();
    account.hash(&mut hasher);
    now_epoch_secs().hash(&mut hasher);
    format!("state-{:x}", hasher.finish())
}

fn exchange_code(account: &str, code: &str, mode: AuthFlowMode) -> StoredToken {
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
