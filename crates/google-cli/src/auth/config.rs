use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const GOOGLE_CLI_CONFIG_DIR_ENV: &str = "GOOGLE_CLI_CONFIG_DIR";
const SCHEMA_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthPaths {
    pub root_dir: PathBuf,
    pub credentials_path: PathBuf,
    pub metadata_path: PathBuf,
    pub token_fallback_path: PathBuf,
    pub remote_state_path: PathBuf,
}

impl AuthPaths {
    pub fn resolve() -> Result<Self, AppError> {
        let root_dir = resolve_root_dir();
        fs::create_dir_all(&root_dir).map_err(|error| {
            AppError::auth_store_failure(format!(
                "failed to create auth config directory `{}`: {error}",
                root_dir.display()
            ))
        })?;

        Ok(Self {
            credentials_path: root_dir.join("credentials.v1.json"),
            metadata_path: root_dir.join("accounts.v1.json"),
            token_fallback_path: root_dir.join("tokens.v1.json"),
            remote_state_path: root_dir.join("remote-state.v1.json"),
            root_dir,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OAuthClientCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    pub redirect_uri: String,
}

impl OAuthClientCredentials {
    pub fn with_defaults(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            auth_uri: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_uri: "https://oauth2.googleapis.com/token".to_string(),
            redirect_uri: "http://127.0.0.1:51789/callback".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CredentialsFile {
    version: u32,
    credentials: OAuthClientCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountMetadata {
    pub version: u32,
    pub default_account: Option<String>,
    pub aliases: BTreeMap<String, String>,
    pub accounts: Vec<String>,
}

impl Default for AccountMetadata {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION_V1,
            default_account: None,
            aliases: BTreeMap::new(),
            accounts: Vec::new(),
        }
    }
}

impl AccountMetadata {
    pub fn normalize(&mut self) {
        let mut unique = BTreeSet::new();
        for account in &self.accounts {
            unique.insert(account.clone());
        }
        self.accounts = unique.into_iter().collect();

        self.aliases
            .retain(|alias, account| !alias.trim().is_empty() && self.accounts.contains(account));

        if let Some(default_account) = &self.default_account {
            if !self.accounts.contains(default_account) {
                self.default_account = None;
            }
        }
    }

    pub fn add_account(&mut self, account: &str) {
        if !self.accounts.iter().any(|candidate| candidate == account) {
            self.accounts.push(account.to_string());
        }
        if self.default_account.is_none() {
            self.default_account = Some(account.to_string());
        }
        self.normalize();
    }

    pub fn remove_account(&mut self, account: &str) {
        self.accounts.retain(|candidate| candidate != account);
        self.aliases.retain(|_, mapped| mapped != account);
        if self.default_account.as_deref() == Some(account) {
            self.default_account = self.accounts.first().cloned();
        }
        self.normalize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteAuthState {
    pub state: String,
    pub issued_at_epoch_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RemoteAuthStates {
    pub version: u32,
    pub by_account: BTreeMap<String, RemoteAuthState>,
}

pub fn load_credentials(paths: &AuthPaths) -> Result<Option<OAuthClientCredentials>, AppError> {
    if !paths.credentials_path.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(&paths.credentials_path).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to read credentials file `{}`: {error}",
            paths.credentials_path.display()
        ))
    })?;

    let file: CredentialsFile = serde_json::from_str(&text).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to parse credentials file `{}`: {error}",
            paths.credentials_path.display()
        ))
    })?;

    Ok(Some(file.credentials))
}

pub fn save_credentials(
    paths: &AuthPaths,
    credentials: &OAuthClientCredentials,
) -> Result<(), AppError> {
    let file = CredentialsFile {
        version: SCHEMA_VERSION_V1,
        credentials: credentials.clone(),
    };
    write_json(&paths.credentials_path, &file)
}

pub fn load_metadata(paths: &AuthPaths) -> Result<AccountMetadata, AppError> {
    if !paths.metadata_path.exists() {
        return Ok(AccountMetadata::default());
    }

    let text = fs::read_to_string(&paths.metadata_path).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to read account metadata `{}`: {error}",
            paths.metadata_path.display()
        ))
    })?;

    let mut metadata: AccountMetadata = serde_json::from_str(&text).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to parse account metadata `{}`: {error}",
            paths.metadata_path.display()
        ))
    })?;
    metadata.version = SCHEMA_VERSION_V1;
    metadata.normalize();
    Ok(metadata)
}

pub fn save_metadata(paths: &AuthPaths, metadata: &AccountMetadata) -> Result<(), AppError> {
    let mut sanitized = metadata.clone();
    sanitized.version = SCHEMA_VERSION_V1;
    sanitized.normalize();
    write_json(&paths.metadata_path, &sanitized)
}

pub fn load_remote_states(paths: &AuthPaths) -> Result<RemoteAuthStates, AppError> {
    if !paths.remote_state_path.exists() {
        return Ok(RemoteAuthStates {
            version: SCHEMA_VERSION_V1,
            by_account: BTreeMap::new(),
        });
    }

    let text = fs::read_to_string(&paths.remote_state_path).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to read remote auth state `{}`: {error}",
            paths.remote_state_path.display()
        ))
    })?;

    let mut states: RemoteAuthStates = serde_json::from_str(&text).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to parse remote auth state `{}`: {error}",
            paths.remote_state_path.display()
        ))
    })?;

    states.version = SCHEMA_VERSION_V1;
    Ok(states)
}

pub fn save_remote_states(paths: &AuthPaths, states: &RemoteAuthStates) -> Result<(), AppError> {
    let mut serialized = states.clone();
    serialized.version = SCHEMA_VERSION_V1;
    write_json(&paths.remote_state_path, &serialized)
}

pub fn now_epoch_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn resolve_root_dir() -> PathBuf {
    if let Some(path) = env::var_os(GOOGLE_CLI_CONFIG_DIR_ENV) {
        return PathBuf::from(path);
    }

    if let Some(project_dirs) = ProjectDirs::from("dev", "graysurf", "google-cli") {
        return project_dirs.config_dir().to_path_buf();
    }

    PathBuf::from(".google-cli")
}

fn write_json<T>(path: &PathBuf, value: &T) -> Result<(), AppError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to serialize auth state `{}`: {error}",
            path.display()
        ))
    })?;

    fs::write(path, bytes).map_err(|error| {
        AppError::auth_store_failure(format!(
            "failed to write auth state `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        AccountMetadata, AuthPaths, OAuthClientCredentials, load_credentials, load_metadata,
        save_credentials, save_metadata,
    };

    #[test]
    fn metadata_roundtrip_normalizes_duplicate_accounts() {
        let temp = tempdir().expect("tempdir");
        let paths = AuthPaths {
            root_dir: temp.path().to_path_buf(),
            credentials_path: temp.path().join("credentials.v1.json"),
            metadata_path: temp.path().join("accounts.v1.json"),
            token_fallback_path: temp.path().join("tokens.v1.json"),
            remote_state_path: temp.path().join("remote-state.v1.json"),
        };

        let mut metadata = AccountMetadata::default();
        metadata.accounts = vec!["a@example.com".into(), "a@example.com".into()];
        metadata.default_account = Some("a@example.com".into());
        save_metadata(&paths, &metadata).expect("save metadata");

        let loaded = load_metadata(&paths).expect("load metadata");
        assert_eq!(loaded.accounts, vec!["a@example.com".to_string()]);
    }

    #[test]
    fn credentials_roundtrip() {
        let temp = tempdir().expect("tempdir");
        let paths = AuthPaths {
            root_dir: temp.path().to_path_buf(),
            credentials_path: temp.path().join("credentials.v1.json"),
            metadata_path: temp.path().join("accounts.v1.json"),
            token_fallback_path: temp.path().join("tokens.v1.json"),
            remote_state_path: temp.path().join("remote-state.v1.json"),
        };

        let credentials = OAuthClientCredentials::with_defaults("id".into(), "secret".into());
        save_credentials(&paths, &credentials).expect("save credentials");
        let loaded = load_credentials(&paths).expect("load credentials");
        assert_eq!(loaded, Some(credentials));
    }
}
