pub mod account;
pub mod browser;
pub mod callback;
pub mod config;
pub mod credentials;
pub mod defaults;
pub mod oauth;
pub mod store;

use std::env;
use std::ffi::OsString;

use serde_json::{Value, json};

use crate::cmd::common::{GlobalOptions, Invocation};
use crate::error::AppError;

use self::account::resolve_account;
use self::config::{
    AuthPaths, RemoteAuthState, load_credentials, load_metadata, load_remote_states,
    now_epoch_secs, save_credentials, save_metadata, save_remote_states,
};
use self::oauth::AuthFlowMode;
use self::store::{load_token, persist_token, remove_token};

/// Native auth account-manager stance for Sprint 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManageBehavior {
    SummaryOnly,
}

pub fn manage_behavior() -> ManageBehavior {
    ManageBehavior::SummaryOnly
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeAuthResponse {
    pub payload: Value,
    pub text: String,
}

pub fn execute_native(
    global: &GlobalOptions,
    invocation: &Invocation,
) -> Result<NativeAuthResponse, AppError> {
    let Some(subcommand) = invocation.path.get(1) else {
        return Err(AppError::invalid_auth_input(
            "missing auth subcommand; expected one of credentials/add/list/status/remove/alias/manage",
        ));
    };

    let subcommand = subcommand.to_string_lossy().to_string();
    let args = os_strings_to_strings(&invocation.args);
    let paths = AuthPaths::resolve()?;

    match subcommand.as_str() {
        "credentials" => execute_credentials(&paths, &args),
        "add" => execute_add(&paths, &args),
        "list" => execute_list(&paths),
        "status" => execute_status(&paths, global),
        "remove" => execute_remove(&paths, &args),
        "alias" => execute_alias(&paths, &args),
        "manage" => execute_manage(&paths),
        unknown => Err(AppError::invalid_auth_input(format!(
            "unknown auth subcommand `{unknown}`"
        ))),
    }
}

fn execute_credentials(paths: &AuthPaths, args: &[String]) -> Result<NativeAuthResponse, AppError> {
    let Some(action) = args.first() else {
        return Err(AppError::invalid_auth_input(
            "missing credentials action; expected `set` or `list`",
        ));
    };

    match action.as_str() {
        "set" => {
            let parsed = credentials::parse_set_args(&args[1..])?;
            save_credentials(paths, &parsed)?;
            Ok(response(
                json!({
                    "updated": true,
                    "client_id": parsed.client_id,
                    "auth_uri": parsed.auth_uri,
                    "token_uri": parsed.token_uri,
                    "redirect_uri": parsed.redirect_uri,
                }),
                "Saved native OAuth credentials.",
            ))
        }
        "list" => {
            let configured = load_credentials(paths)?;
            if let Some(credentials) = configured {
                Ok(response(
                    json!({
                        "configured": true,
                        "client_id": credentials.client_id,
                        "client_secret_masked": mask_secret(&credentials.client_secret),
                        "auth_uri": credentials.auth_uri,
                        "token_uri": credentials.token_uri,
                        "redirect_uri": credentials.redirect_uri,
                    }),
                    "Native OAuth credentials are configured.",
                ))
            } else {
                Ok(response(
                    json!({
                        "configured": false,
                    }),
                    "Native OAuth credentials are not configured.",
                ))
            }
        }
        unknown => Err(AppError::invalid_auth_input(format!(
            "unknown credentials action `{unknown}`; expected `set` or `list`"
        ))),
    }
}

fn execute_add(paths: &AuthPaths, args: &[String]) -> Result<NativeAuthResponse, AppError> {
    let Some(account) = args.first() else {
        return Err(AppError::invalid_auth_input(
            "missing account; expected `auth add <email> [--manual|--remote ...]`",
        ));
    };
    let account = account.clone();
    let options = parse_add_options(&args[1..])?;

    let credentials = load_credentials(paths)?.ok_or_else(|| {
        AppError::invalid_auth_input(
            "OAuth credentials are not configured; run `auth credentials set --client-id <id> --client-secret <secret>` first",
        )
    })?;

    let mut metadata = load_metadata(paths)?;
    let mut remote_states = load_remote_states(paths)?;

    if options.remote {
        let step = options.step.unwrap_or(1);
        match step {
            1 => {
                let remote = oauth::begin_remote(&account, &credentials)?;
                remote_states.by_account.insert(
                    account.clone(),
                    RemoteAuthState {
                        state: remote.state.clone(),
                        issued_at_epoch_secs: now_epoch_secs(),
                    },
                );
                save_remote_states(paths, &remote_states)?;
                return Ok(response(
                    json!({
                        "account": account,
                        "mode": "remote",
                        "step": 1,
                        "state": remote.state,
                        "authorization_url": remote.authorization_url,
                    }),
                    "Remote auth step 1 generated. Continue with --remote --step 2.",
                ));
            }
            2 => {
                let expected = remote_states.by_account.get(&account).ok_or_else(|| {
                    AppError::invalid_auth_input(
                        "remote auth step 2 is missing saved state; restart with `auth add <email> --remote --step 1`",
                    )
                })?;
                let provided_state = options.state.as_deref().ok_or_else(|| {
                    AppError::invalid_auth_input("remote step 2 requires `--state <state>`")
                })?;
                let code = options.code.as_deref().ok_or_else(|| {
                    AppError::invalid_auth_input(
                        "remote step 2 requires `--code <authorization_code>`",
                    )
                })?;

                let token = oauth::finish_remote(&account, &expected.state, provided_state, code)?;
                let persist = persist_token(paths, &account, &token)?;
                metadata.add_account(&account);
                save_metadata(paths, &metadata)?;
                remote_states.by_account.remove(&account);
                save_remote_states(paths, &remote_states)?;

                return Ok(response(
                    json!({
                        "account": account,
                        "mode": "remote",
                        "step": 2,
                        "backend": persist.backend.as_str(),
                        "stored": true,
                    }),
                    "Remote auth token exchange completed.",
                ));
            }
            value => {
                return Err(AppError::invalid_auth_input(format!(
                    "unsupported remote step `{value}`; expected `1` or `2`"
                )));
            }
        }
    }

    let (mode, token) = if options.manual {
        let code = options
            .code
            .or_else(|| env::var(oauth::GOOGLE_CLI_AUTH_TEST_CODE_ENV).ok())
            .ok_or_else(|| {
                AppError::invalid_auth_input("manual auth requires `--code <authorization_code>`")
            })?;
        (AuthFlowMode::Manual, oauth::run_manual(&account, &code))
    } else {
        let outcome = oauth::run_loopback(&account, &credentials)?;
        (AuthFlowMode::Loopback, outcome.token)
    };

    let persist = persist_token(paths, &account, &token)?;
    metadata.add_account(&account);
    save_metadata(paths, &metadata)?;

    Ok(response(
        json!({
            "account": account,
            "mode": mode.as_str(),
            "backend": persist.backend.as_str(),
            "stored": true,
        }),
        format!(
            "Auth token stored via {} backend.",
            persist.backend.as_str()
        ),
    ))
}

fn execute_list(paths: &AuthPaths) -> Result<NativeAuthResponse, AppError> {
    let metadata = load_metadata(paths)?;
    Ok(response(
        json!({
            "accounts": metadata.accounts,
            "default_account": metadata.default_account,
            "aliases": metadata.aliases,
        }),
        "Listed native auth accounts.",
    ))
}

fn execute_status(
    paths: &AuthPaths,
    global: &GlobalOptions,
) -> Result<NativeAuthResponse, AppError> {
    let metadata = load_metadata(paths)?;
    let resolved = resolve_account(global.account.as_deref(), &metadata)?;
    let token = load_token(paths, &resolved.account)?;

    Ok(response(
        json!({
            "account": resolved.account,
            "source": resolved.source.as_str(),
            "has_token": token.is_some(),
            "mode": token.as_ref().map(|value| value.mode.clone()),
            "default_account": metadata.default_account,
            "account_count": metadata.accounts.len(),
        }),
        "Resolved native auth status.",
    ))
}

fn execute_remove(paths: &AuthPaths, args: &[String]) -> Result<NativeAuthResponse, AppError> {
    let Some(target) = args.first() else {
        return Err(AppError::invalid_auth_input(
            "missing account; expected `auth remove <email-or-alias>`",
        ));
    };

    let mut metadata = load_metadata(paths)?;
    let resolved = resolve_account(Some(target), &metadata)?;
    let removed_token = remove_token(paths, &resolved.account)?;
    metadata.remove_account(&resolved.account);
    save_metadata(paths, &metadata)?;

    let mut remote_states = load_remote_states(paths)?;
    remote_states.by_account.remove(&resolved.account);
    save_remote_states(paths, &remote_states)?;

    Ok(response(
        json!({
            "account": resolved.account,
            "removed_token": removed_token,
            "remaining_accounts": metadata.accounts.len(),
        }),
        "Removed native auth account.",
    ))
}

fn execute_alias(paths: &AuthPaths, args: &[String]) -> Result<NativeAuthResponse, AppError> {
    let Some(action) = args.first() else {
        return Err(AppError::invalid_auth_input(
            "missing alias action; expected `set`, `remove`, or `list`",
        ));
    };

    let mut metadata = load_metadata(paths)?;
    match action.as_str() {
        "list" => Ok(response(
            json!({
                "aliases": metadata.aliases,
                "default_account": metadata.default_account,
            }),
            "Listed auth aliases.",
        )),
        "set" => {
            if args.len() < 3 {
                return Err(AppError::invalid_auth_input(
                    "alias set requires `auth alias set <alias> <account>`",
                ));
            }
            let alias = args[1].trim();
            if alias.is_empty() {
                return Err(AppError::invalid_auth_input("alias cannot be empty"));
            }
            let resolved = resolve_account(Some(&args[2]), &metadata)?;
            metadata
                .aliases
                .insert(alias.to_string(), resolved.account.clone());
            save_metadata(paths, &metadata)?;
            Ok(response(
                json!({
                    "alias": alias,
                    "account": resolved.account,
                    "updated": true,
                }),
                "Updated alias mapping.",
            ))
        }
        "remove" | "unset" => {
            if args.len() < 2 {
                return Err(AppError::invalid_auth_input(
                    "alias remove requires `auth alias remove <alias>`",
                ));
            }
            let alias = args[1].trim().to_string();
            let removed = metadata.aliases.remove(&alias).is_some();
            save_metadata(paths, &metadata)?;
            Ok(response(
                json!({
                    "alias": alias,
                    "removed": removed,
                }),
                "Removed alias mapping.",
            ))
        }
        unknown => Err(AppError::invalid_auth_input(format!(
            "unknown alias action `{unknown}`; expected `set`, `remove`, or `list`"
        ))),
    }
}

fn execute_manage(paths: &AuthPaths) -> Result<NativeAuthResponse, AppError> {
    let metadata = load_metadata(paths)?;
    Ok(response(
        json!({
            "behavior": "summary-only",
            "accounts": metadata.accounts,
            "default_account": metadata.default_account,
            "note": "`auth manage` is terminal-native only; browser account-manager UI is intentionally unsupported",
        }),
        "Auth manage is summary-only in native mode (no browser manager UI).",
    ))
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct AddOptions {
    manual: bool,
    remote: bool,
    step: Option<u8>,
    code: Option<String>,
    state: Option<String>,
}

fn parse_add_options(args: &[String]) -> Result<AddOptions, AppError> {
    let mut options = AddOptions::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--manual" => {
                options.manual = true;
                index += 1;
            }
            "--remote" => {
                options.remote = true;
                index += 1;
            }
            "--step" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(AppError::invalid_auth_input("missing value for `--step`"));
                };
                options.step = Some(value.parse::<u8>().map_err(|_| {
                    AppError::invalid_auth_input("`--step` must be a number (1 or 2)")
                })?);
                index += 2;
            }
            "--code" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(AppError::invalid_auth_input("missing value for `--code`"));
                };
                options.code = Some(value.clone());
                index += 2;
            }
            "--state" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(AppError::invalid_auth_input("missing value for `--state`"));
                };
                options.state = Some(value.clone());
                index += 2;
            }
            unknown => {
                return Err(AppError::invalid_auth_input(format!(
                    "unknown auth add option `{unknown}`"
                )));
            }
        }
    }

    if options.manual && options.remote {
        return Err(AppError::invalid_auth_input(
            "choose one auth mode: `--manual` or `--remote`",
        ));
    }

    if options.step.is_some() && !options.remote {
        return Err(AppError::invalid_auth_input(
            "`--step` is only valid with `--remote`",
        ));
    }

    Ok(options)
}

fn os_strings_to_strings(values: &[OsString]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.to_string_lossy().to_string())
        .collect()
}

fn response(payload: Value, text: impl Into<String>) -> NativeAuthResponse {
    NativeAuthResponse {
        payload,
        text: text.into(),
    }
}

fn mask_secret(secret: &str) -> String {
    let visible = secret.chars().take(4).collect::<String>();
    if visible.is_empty() {
        "***".to_string()
    } else {
        format!("{visible}***")
    }
}
