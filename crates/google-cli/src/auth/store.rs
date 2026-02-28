use std::collections::BTreeMap;
use std::env;
use std::fs;

use keyring::Entry;
use serde::{Deserialize, Serialize};

use super::config::AuthPaths;
use crate::error::AppError;

pub const GOOGLE_CLI_KEYRING_MODE_ENV: &str = "GOOGLE_CLI_KEYRING_MODE";
const KEYRING_SERVICE_NAME: &str = "nils.google-cli.auth";
const TOKENS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: String,
    pub mode: String,
    pub issued_at_epoch_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenBackend {
    Keyring,
    FileOnly,
    FileFallback,
}

impl TokenBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenBackend::Keyring => "keyring",
            TokenBackend::FileOnly => "file",
            TokenBackend::FileFallback => "file-fallback",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistOutcome {
    pub backend: TokenBackend,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct FileTokenStore {
    version: u32,
    tokens: BTreeMap<String, StoredToken>,
}

pub fn persist_token(
    paths: &AuthPaths,
    account: &str,
    token: &StoredToken,
) -> Result<PersistOutcome, AppError> {
    match mode().as_str() {
        "fail" => Err(AppError::auth_store_failure(
            "keyring storage disabled by GOOGLE_CLI_KEYRING_MODE=fail",
        )),
        "file" => {
            write_file_token(paths, account, token)?;
            Ok(PersistOutcome {
                backend: TokenBackend::FileOnly,
            })
        }
        _ => match set_keyring_token(account, token) {
            Ok(()) => Ok(PersistOutcome {
                backend: TokenBackend::Keyring,
            }),
            Err(error) => {
                write_file_token(paths, account, token)?;
                if mode() == "keyring-strict" {
                    Err(AppError::auth_store_failure(format!(
                        "failed to persist token in keyring: {error}"
                    )))
                } else {
                    Ok(PersistOutcome {
                        backend: TokenBackend::FileFallback,
                    })
                }
            }
        },
    }
}

pub fn load_token(paths: &AuthPaths, account: &str) -> Result<Option<StoredToken>, AppError> {
    match mode().as_str() {
        "fail" => Err(AppError::auth_store_failure(
            "keyring storage disabled by GOOGLE_CLI_KEYRING_MODE=fail",
        )),
        "file" => read_file_token(paths, account),
        _ => match get_keyring_token(account) {
            Ok(Some(token)) => Ok(Some(token)),
            Ok(None) => read_file_token(paths, account),
            Err(_) => read_file_token(paths, account),
        },
    }
}

pub fn remove_token(paths: &AuthPaths, account: &str) -> Result<bool, AppError> {
    match mode().as_str() {
        "fail" => Err(AppError::auth_store_failure(
            "keyring storage disabled by GOOGLE_CLI_KEYRING_MODE=fail",
        )),
        "file" => remove_file_token(paths, account),
        _ => {
            let removed_keyring = delete_keyring_token(account).unwrap_or(false);
            let removed_file = remove_file_token(paths, account)?;
            Ok(removed_keyring || removed_file)
        }
    }
}

pub fn list_accounts(paths: &AuthPaths) -> Result<Vec<String>, AppError> {
    let store = load_file_store(paths)?;
    Ok(store.tokens.keys().cloned().collect())
}

fn mode() -> String {
    env::var(GOOGLE_CLI_KEYRING_MODE_ENV).unwrap_or_else(|_| "keyring".to_string())
}

fn set_keyring_token(account: &str, token: &StoredToken) -> Result<(), String> {
    let payload = serde_json::to_string(token).map_err(|error| error.to_string())?;
    let entry = Entry::new(KEYRING_SERVICE_NAME, account).map_err(|error| error.to_string())?;
    entry
        .set_password(&payload)
        .map_err(|error| error.to_string())
}

fn get_keyring_token(account: &str) -> Result<Option<StoredToken>, String> {
    let entry = Entry::new(KEYRING_SERVICE_NAME, account).map_err(|error| error.to_string())?;
    match entry.get_password() {
        Ok(payload) => {
            let token: StoredToken =
                serde_json::from_str(&payload).map_err(|error| error.to_string())?;
            Ok(Some(token))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn delete_keyring_token(account: &str) -> Result<bool, String> {
    let entry = Entry::new(KEYRING_SERVICE_NAME, account).map_err(|error| error.to_string())?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn write_file_token(paths: &AuthPaths, account: &str, token: &StoredToken) -> Result<(), AppError> {
    let mut store = load_file_store(paths)?;
    store.version = TOKENS_SCHEMA_VERSION;
    store.tokens.insert(account.to_string(), token.clone());
    save_file_store(paths, &store)
}

fn read_file_token(paths: &AuthPaths, account: &str) -> Result<Option<StoredToken>, AppError> {
    let store = load_file_store(paths)?;
    Ok(store.tokens.get(account).cloned())
}

fn remove_file_token(paths: &AuthPaths, account: &str) -> Result<bool, AppError> {
    let mut store = load_file_store(paths)?;
    let removed = store.tokens.remove(account).is_some();
    if removed {
        save_file_store(paths, &store)?;
    }
    Ok(removed)
}

fn load_file_store(paths: &AuthPaths) -> Result<FileTokenStore, AppError> {
    if !paths.token_fallback_path.exists() {
        return Ok(FileTokenStore {
            version: TOKENS_SCHEMA_VERSION,
            tokens: BTreeMap::new(),
        });
    }

    let text = fs::read_to_string(&paths.token_fallback_path).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to read token fallback store `{}`: {error}",
            paths.token_fallback_path.display()
        ))
    })?;

    let mut store: FileTokenStore = serde_json::from_str(&text).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to parse token fallback store `{}`: {error}",
            paths.token_fallback_path.display()
        ))
    })?;
    store.version = TOKENS_SCHEMA_VERSION;
    Ok(store)
}

fn save_file_store(paths: &AuthPaths, store: &FileTokenStore) -> Result<(), AppError> {
    let text = serde_json::to_string_pretty(store).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to serialize token fallback store `{}`: {error}",
            paths.token_fallback_path.display()
        ))
    })?;

    fs::write(&paths.token_fallback_path, text).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to write token fallback store `{}`: {error}",
            paths.token_fallback_path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        GOOGLE_CLI_KEYRING_MODE_ENV, StoredToken, TokenBackend, load_token, persist_token,
        remove_token,
    };
    use crate::auth::config::AuthPaths;

    fn token(mode: &str) -> StoredToken {
        StoredToken {
            access_token: format!("access-{mode}"),
            refresh_token: format!("refresh-{mode}"),
            mode: mode.to_string(),
            issued_at_epoch_secs: 1,
        }
    }

    #[test]
    fn file_mode_roundtrip() {
        let temp = tempdir().expect("tempdir");
        let paths = AuthPaths {
            root_dir: temp.path().to_path_buf(),
            credentials_path: temp.path().join("credentials.v1.json"),
            metadata_path: temp.path().join("accounts.v1.json"),
            token_fallback_path: temp.path().join("tokens.v1.json"),
            remote_state_path: temp.path().join("remote-state.v1.json"),
        };
        // Test-only env override to force deterministic file backend behavior.
        unsafe {
            std::env::set_var(GOOGLE_CLI_KEYRING_MODE_ENV, "file");
        }

        let outcome = persist_token(&paths, "me@example.com", &token("manual")).expect("persist");
        assert_eq!(outcome.backend, TokenBackend::FileOnly);

        let loaded = load_token(&paths, "me@example.com").expect("load");
        assert!(loaded.is_some());

        let removed = remove_token(&paths, "me@example.com").expect("remove");
        assert!(removed);
    }
}
