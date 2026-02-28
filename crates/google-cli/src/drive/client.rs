use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::auth::account::resolve_account;
use crate::auth::config::{AuthPaths, load_metadata};
use crate::auth::store::load_token;
use crate::cmd::common::GlobalOptions;
use crate::error::AppError;

use super::mime::resolve_mime_type;

const GOOGLE_CLI_DRIVE_FIXTURE_PATH_ENV: &str = "GOOGLE_CLI_DRIVE_FIXTURE_PATH";
const GOOGLE_CLI_DRIVE_FIXTURE_JSON_ENV: &str = "GOOGLE_CLI_DRIVE_FIXTURE_JSON";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriveFile {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub export_formats: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DriveFixtureStore {
    #[serde(default)]
    pub files: Vec<DriveFile>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileView {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DriveSession {
    pub account: String,
    pub account_source: String,
    pub access_token: String,
    fixture: DriveFixtureStore,
}

#[derive(Debug, Clone)]
pub struct ListRequest {
    pub parent: Option<String>,
    pub query: Option<String>,
    pub max: usize,
    pub page_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: String,
    pub max: usize,
    pub page_token: Option<String>,
    pub raw_query: bool,
}

#[derive(Debug, Clone)]
pub struct GetRequest {
    pub file_id: String,
}

#[derive(Debug, Clone)]
pub struct UploadRequest {
    pub local_path: PathBuf,
    pub parent: Option<String>,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub replace: bool,
    pub convert: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UploadResult {
    pub file: FileView,
    pub replaced: bool,
    pub replaced_file_id: Option<String>,
    pub inferred_mime_type: String,
    pub source_path: String,
    pub convert_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadPayload {
    pub file_id: String,
    pub file_name: String,
    pub mime_type: String,
    pub format: Option<String>,
    pub source: &'static str,
    pub bytes: Vec<u8>,
}

impl DriveSession {
    pub fn from_global(global: &GlobalOptions) -> Result<Self, AppError> {
        let paths = AuthPaths::resolve()?;
        let metadata = load_metadata(&paths)?;
        let resolved = resolve_account(global.account.as_deref(), &metadata)?;
        let token = load_token(&paths, &resolved.account)?.ok_or_else(|| {
            AppError::invalid_drive_input(format!(
                "account `{}` has no token; run `auth add {}` first",
                resolved.account, resolved.account
            ))
        })?;

        let fixture = load_fixture_store()?;
        Ok(Self {
            account: resolved.account,
            account_source: resolved.source.as_str().to_string(),
            access_token: token.access_token,
            fixture,
        })
    }

    pub fn list(&self, request: &ListRequest) -> Vec<FileView> {
        let mut files = self
            .fixture
            .files
            .iter()
            .filter(|file| parent_matches(file, request.parent.as_deref()))
            .filter(|file| query_matches(file, request.query.as_deref().unwrap_or_default()))
            .map(view_for_file)
            .collect::<Vec<_>>();

        files.truncate(request.max);
        files
    }

    pub fn search(&self, request: &SearchRequest) -> Vec<FileView> {
        let mut files = self
            .fixture
            .files
            .iter()
            .filter(|file| query_matches(file, request.query.as_str()))
            .map(view_for_file)
            .collect::<Vec<_>>();

        files.truncate(request.max);
        files
    }

    pub fn get(&self, request: &GetRequest) -> Result<FileView, AppError> {
        let file = self
            .fixture
            .files
            .iter()
            .find(|file| file.id == request.file_id)
            .ok_or_else(|| AppError::drive_not_found("file", request.file_id.as_str()))?;
        Ok(view_for_file(file))
    }

    pub fn resolve_download(
        &self,
        file_id: &str,
        format: Option<&str>,
    ) -> Result<DownloadPayload, AppError> {
        let file = self
            .fixture
            .files
            .iter()
            .find(|candidate| candidate.id == file_id)
            .ok_or_else(|| AppError::drive_not_found("file", file_id))?;

        if let Some(format) = format {
            let Some(content) = file.export_formats.get(format) else {
                return Err(AppError::invalid_drive_input(format!(
                    "file `{file_id}` does not support export format `{format}`"
                )));
            };

            return Ok(DownloadPayload {
                file_id: file.id.clone(),
                file_name: file.name.clone(),
                mime_type: file.mime_type.clone(),
                format: Some(format.to_string()),
                source: "export",
                bytes: content.as_bytes().to_vec(),
            });
        }

        Ok(DownloadPayload {
            file_id: file.id.clone(),
            file_name: file.name.clone(),
            mime_type: file.mime_type.clone(),
            format: None,
            source: "download",
            bytes: file.content.as_bytes().to_vec(),
        })
    }

    pub fn upload(&self, request: &UploadRequest) -> Result<UploadResult, AppError> {
        let source_path = request.local_path.clone();
        if !source_path.is_file() {
            return Err(AppError::invalid_drive_input(format!(
                "upload source `{}` is not a file",
                source_path.display()
            )));
        }

        let inferred_mime_type =
            resolve_mime_type(source_path.as_path(), request.mime_type.as_deref())?;
        let source_name = source_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .ok_or_else(|| AppError::invalid_drive_input("upload source path has no file name"))?;
        let upload_name = request.name.clone().unwrap_or(source_name);
        let parent = request.parent.clone().unwrap_or_else(|| "root".to_string());

        let replaced = if request.replace {
            self.fixture.files.iter().find(|file| {
                file.name == upload_name && parent_matches(file, Some(parent.as_str()))
            })
        } else {
            None
        };

        let id = replaced.map(|file| file.id.clone()).unwrap_or_else(|| {
            synthetic_file_id(&self.account, &upload_name, &parent, &inferred_mime_type)
        });

        let size_bytes = fs::metadata(&source_path)
            .map(|metadata| metadata.len())
            .map_err(|error| {
                AppError::drive_failure(format!(
                    "failed to stat upload source `{}`: {error}",
                    source_path.display()
                ))
            })?;

        let file = FileView {
            id,
            name: upload_name,
            mime_type: inferred_mime_type.clone(),
            size_bytes,
            parents: vec![parent],
        };

        Ok(UploadResult {
            replaced: replaced.is_some(),
            replaced_file_id: replaced.map(|file| file.id.clone()),
            inferred_mime_type,
            source_path: source_path.display().to_string(),
            convert_requested: request.convert,
            file,
        })
    }
}

fn view_for_file(file: &DriveFile) -> FileView {
    FileView {
        id: file.id.clone(),
        name: file.name.clone(),
        mime_type: file.mime_type.clone(),
        size_bytes: file.size_bytes,
        parents: file.parents.clone(),
    }
}

fn parent_matches(file: &DriveFile, parent: Option<&str>) -> bool {
    match parent {
        Some(parent) => file.parents.iter().any(|value| value == parent),
        None => true,
    }
}

fn query_matches(file: &DriveFile, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }

    query
        .split_whitespace()
        .all(|token| match token.split_once(':') {
            Some(("name", value)) => contains_ignore_ascii_case(&file.name, value),
            Some(("mime", value)) => contains_ignore_ascii_case(&file.mime_type, value),
            Some(("id", value)) => contains_ignore_ascii_case(&file.id, value),
            Some(("parent", value)) => file
                .parents
                .iter()
                .any(|parent| contains_ignore_ascii_case(parent, value)),
            _ => {
                contains_ignore_ascii_case(&file.name, token)
                    || contains_ignore_ascii_case(&file.mime_type, token)
                    || contains_ignore_ascii_case(&file.id, token)
            }
        })
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn synthetic_file_id(account: &str, name: &str, parent: &str, mime_type: &str) -> String {
    let mut hasher = DefaultHasher::new();
    account.hash(&mut hasher);
    name.hash(&mut hasher);
    parent.hash(&mut hasher);
    mime_type.hash(&mut hasher);
    format!("drive-{:x}", hasher.finish())
}

fn load_fixture_store() -> Result<DriveFixtureStore, AppError> {
    if let Ok(path) = env::var(GOOGLE_CLI_DRIVE_FIXTURE_PATH_ENV) {
        let bytes = fs::read(&path).map_err(|error| {
            AppError::drive_failure(format!(
                "failed to read Drive fixture file `{path}`: {error}"
            ))
        })?;
        return serde_json::from_slice::<DriveFixtureStore>(&bytes).map_err(|error| {
            AppError::drive_failure(format!(
                "failed to parse Drive fixture file `{path}`: {error}"
            ))
        });
    }

    if let Ok(raw) = env::var(GOOGLE_CLI_DRIVE_FIXTURE_JSON_ENV) {
        return serde_json::from_str::<DriveFixtureStore>(&raw).map_err(|error| {
            AppError::drive_failure(format!(
                "failed to parse Drive fixture JSON from {GOOGLE_CLI_DRIVE_FIXTURE_JSON_ENV}: {error}"
            ))
        });
    }

    Ok(DriveFixtureStore {
        files: vec![DriveFile {
            id: "file-sample-1".to_string(),
            name: "sample.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 12,
            parents: vec!["root".to_string()],
            content: "sample".to_string(),
            export_formats: BTreeMap::new(),
        }],
    })
}
