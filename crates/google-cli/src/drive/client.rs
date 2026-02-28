use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use reqwest::blocking::{Client, Response, multipart};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::auth::account::resolve_account;
use crate::auth::config::{AuthPaths, load_credentials, load_metadata};
use crate::auth::oauth;
use crate::auth::store::{load_token, persist_token};
use crate::cmd::common::GlobalOptions;
use crate::error::{AppError, redact_sensitive};

use super::mime::resolve_mime_type;

const DRIVE_API_BASE: &str = "https://www.googleapis.com/drive/v3";
const DRIVE_UPLOAD_BASE: &str = "https://www.googleapis.com/upload/drive/v3";
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
    client: Client,
    fixture: Option<DriveFixtureStore>,
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
        let active_token = if fixture.is_some() {
            token
        } else {
            let credentials = load_credentials(&paths)?.ok_or_else(|| {
                AppError::invalid_drive_input(
                    "OAuth credentials are not configured; run `auth credentials set --client-id <id> --client-secret <secret>` first",
                )
            })?;

            let refreshed = oauth::refresh_access_token(&resolved.account, &credentials, &token)
                .map_err(|error| {
                    AppError::drive_failure(format!(
                        "failed to refresh OAuth token for `{}`: {}",
                        resolved.account,
                        error.message()
                    ))
                })?;
            if refreshed != token {
                persist_token(&paths, &resolved.account, &refreshed).map_err(|error| {
                    AppError::drive_failure(format!(
                        "failed to persist refreshed OAuth token for `{}`: {}",
                        resolved.account,
                        error.message()
                    ))
                })?;
            }
            refreshed
        };

        Ok(Self {
            account: resolved.account,
            account_source: resolved.source.as_str().to_string(),
            access_token: active_token.access_token,
            client: Client::new(),
            fixture,
        })
    }

    pub fn list(&self, request: &ListRequest) -> Result<Vec<FileView>, AppError> {
        if let Some(fixture) = &self.fixture {
            let mut files = fixture
                .files
                .iter()
                .filter(|file| parent_matches(file, request.parent.as_deref()))
                .filter(|file| query_matches(file, request.query.as_deref().unwrap_or_default()))
                .map(view_for_file)
                .collect::<Vec<_>>();
            files.truncate(request.max);
            return Ok(files);
        }

        let mut clauses = vec!["trashed = false".to_string()];
        if let Some(parent) = &request.parent {
            clauses.push(format!("'{}' in parents", escape_drive_literal(parent)));
        }
        if let Some(query) = &request.query
            && !query.trim().is_empty()
        {
            clauses.push(format!("({query})"));
        }
        let q = clauses.join(" and ");

        self.list_live(&q, request.max, request.page_token.as_deref())
    }

    pub fn search(&self, request: &SearchRequest) -> Result<Vec<FileView>, AppError> {
        if let Some(fixture) = &self.fixture {
            let mut files = fixture
                .files
                .iter()
                .filter(|file| query_matches(file, request.query.as_str()))
                .map(view_for_file)
                .collect::<Vec<_>>();
            files.truncate(request.max);
            return Ok(files);
        }

        let mut q = if request.raw_query {
            request.query.clone()
        } else {
            build_search_query(&request.query)
        };
        if q.trim().is_empty() {
            q = "trashed = false".to_string();
        } else {
            q = format!("({q}) and trashed = false");
        }

        self.list_live(&q, request.max, request.page_token.as_deref())
    }

    pub fn get(&self, request: &GetRequest) -> Result<FileView, AppError> {
        if let Some(fixture) = &self.fixture {
            let file = fixture
                .files
                .iter()
                .find(|file| file.id == request.file_id)
                .ok_or_else(|| AppError::drive_not_found("file", request.file_id.as_str()))?;
            return Ok(view_for_file(file));
        }

        let response = self.drive_get_json(
            format!(
                "files/{}?fields=id,name,mimeType,size,parents&supportsAllDrives=true",
                request.file_id
            )
            .as_str(),
            Some(("file", request.file_id.as_str())),
        )?;
        Ok(view_from_live_json(&response))
    }

    pub fn resolve_download(
        &self,
        file_id: &str,
        format: Option<&str>,
    ) -> Result<DownloadPayload, AppError> {
        if let Some(fixture) = &self.fixture {
            let file = fixture
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

            return Ok(DownloadPayload {
                file_id: file.id.clone(),
                file_name: file.name.clone(),
                mime_type: file.mime_type.clone(),
                format: None,
                source: "download",
                bytes: file.content.as_bytes().to_vec(),
            });
        }

        let metadata = self.drive_get_json(
            format!("files/{file_id}?fields=id,name,mimeType,exportLinks&supportsAllDrives=true")
                .as_str(),
            Some(("file", file_id)),
        )?;
        let file_name = metadata
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(file_id)
            .to_string();
        let mime_type = metadata
            .get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream")
            .to_string();

        if let Some(format) = format {
            let export_mime = resolve_export_mime_type(format).ok_or_else(|| {
                AppError::invalid_drive_input(format!("unsupported export format `{format}`"))
            })?;
            let response = self
                .client
                .get(format!("{DRIVE_API_BASE}/files/{file_id}/export"))
                .bearer_auth(&self.access_token)
                .query(&[
                    ("mimeType", export_mime.as_str()),
                    ("supportsAllDrives", "true"),
                ])
                .send()
                .map_err(|error| {
                    AppError::drive_failure(format!("GET files/{file_id}/export failed: {error}"))
                })?;
            let bytes = parse_drive_bytes_response(
                response,
                format!("GET files/{file_id}/export").as_str(),
                Some(("file", file_id)),
            )?;
            return Ok(DownloadPayload {
                file_id: file_id.to_string(),
                file_name,
                mime_type,
                format: Some(format.to_string()),
                source: "export",
                bytes,
            });
        }

        let bytes = self.drive_get_bytes(
            format!("files/{file_id}?alt=media&supportsAllDrives=true").as_str(),
            Some(("file", file_id)),
        )?;
        Ok(DownloadPayload {
            file_id: file_id.to_string(),
            file_name,
            mime_type,
            format: None,
            source: "download",
            bytes,
        })
    }

    pub fn upload(&self, request: &UploadRequest) -> Result<UploadResult, AppError> {
        if self.fixture.is_some() {
            return upload_to_fixture(self, request);
        }

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
        let source_bytes = fs::read(&source_path).map_err(|error| {
            AppError::drive_failure(format!(
                "failed to read upload source `{}`: {error}",
                source_path.display()
            ))
        })?;

        let replaced = if request.replace {
            self.find_existing_by_name(&upload_name, &parent)?
        } else {
            None
        };

        let endpoint = if let Some(existing) = &replaced {
            format!(
                "{DRIVE_UPLOAD_BASE}/files/{}?uploadType=multipart&supportsAllDrives=true",
                existing.id
            )
        } else {
            format!("{DRIVE_UPLOAD_BASE}/files?uploadType=multipart&supportsAllDrives=true")
        };

        let mut metadata = serde_json::Map::new();
        metadata.insert("name".to_string(), Value::String(upload_name.clone()));
        if replaced.is_none() {
            metadata.insert(
                "parents".to_string(),
                Value::Array(vec![Value::String(parent.clone())]),
            );
        }
        if request.convert
            && let Some(target) = convert_target_mime(&inferred_mime_type)
        {
            metadata.insert("mimeType".to_string(), Value::String(target.to_string()));
        }

        let metadata_part = multipart::Part::text(Value::Object(metadata).to_string())
            .mime_str("application/json; charset=utf-8")
            .map_err(|error| {
                AppError::drive_failure(format!("invalid upload metadata part: {error}"))
            })?;
        let file_part = multipart::Part::bytes(source_bytes)
            .file_name(upload_name.clone())
            .mime_str(inferred_mime_type.as_str())
            .map_err(|error| {
                AppError::drive_failure(format!("invalid upload MIME type: {error}"))
            })?;
        let form = multipart::Form::new()
            .part("metadata", metadata_part)
            .part("file", file_part);

        let request_builder = if replaced.is_some() {
            self.client.patch(&endpoint)
        } else {
            self.client.post(&endpoint)
        };
        let response = request_builder
            .bearer_auth(&self.access_token)
            .multipart(form)
            .send()
            .map_err(|error| AppError::drive_failure(format!("upload request failed: {error}")))?;
        let payload = parse_drive_json_response(response, "upload file", None)?;
        let file = view_from_live_json(&payload);

        Ok(UploadResult {
            replaced: replaced.is_some(),
            replaced_file_id: replaced.map(|file| file.id),
            inferred_mime_type,
            source_path: source_path.display().to_string(),
            convert_requested: request.convert,
            file,
        })
    }

    fn list_live(
        &self,
        query: &str,
        max: usize,
        page_token: Option<&str>,
    ) -> Result<Vec<FileView>, AppError> {
        let mut params = vec![
            ("q", query.to_string()),
            ("pageSize", max.to_string()),
            (
                "fields",
                "nextPageToken,files(id,name,mimeType,size,parents)".to_string(),
            ),
            ("supportsAllDrives", "true".to_string()),
            ("includeItemsFromAllDrives", "true".to_string()),
        ];
        if let Some(page_token) = page_token {
            params.push(("pageToken", page_token.to_string()));
        }

        let response = self
            .client
            .get(format!("{DRIVE_API_BASE}/files"))
            .bearer_auth(&self.access_token)
            .query(&params)
            .send()
            .map_err(|error| {
                AppError::drive_failure(format!("list files request failed: {error}"))
            })?;
        let payload = parse_drive_json_response(response, "list files", None)?;
        let files = payload
            .get("files")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|file| view_from_live_json(&file))
            .collect();
        Ok(files)
    }

    fn find_existing_by_name(
        &self,
        name: &str,
        parent: &str,
    ) -> Result<Option<FileView>, AppError> {
        let query = format!(
            "name = '{}' and '{}' in parents and trashed = false",
            escape_drive_literal(name),
            escape_drive_literal(parent),
        );
        let files = self.list_live(&query, 1, None)?;
        Ok(files.into_iter().next())
    }

    fn drive_get_json(
        &self,
        path_and_query: &str,
        not_found: Option<(&str, &str)>,
    ) -> Result<Value, AppError> {
        let url = format!("{DRIVE_API_BASE}/{path_and_query}");
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .map_err(|error| AppError::drive_failure(format!("GET {url} failed: {error}")))?;
        parse_drive_json_response(
            response,
            format!("GET {path_and_query}").as_str(),
            not_found,
        )
    }

    fn drive_get_bytes(
        &self,
        path_and_query: &str,
        not_found: Option<(&str, &str)>,
    ) -> Result<Vec<u8>, AppError> {
        let url = format!("{DRIVE_API_BASE}/{path_and_query}");
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .map_err(|error| AppError::drive_failure(format!("GET {url} failed: {error}")))?;
        parse_drive_bytes_response(
            response,
            format!("GET {path_and_query}").as_str(),
            not_found,
        )
    }
}

fn parse_drive_json_response(
    response: Response,
    context: &str,
    not_found: Option<(&str, &str)>,
) -> Result<Value, AppError> {
    let status = response.status();
    let body = response.text().map_err(|error| {
        AppError::drive_failure(format!("{context} failed reading body: {error}"))
    })?;

    if status.as_u16() == 404
        && let Some((entity, id)) = not_found
    {
        return Err(AppError::drive_not_found(entity, id));
    }

    if !status.is_success() {
        let detail = extract_error_message(&body).unwrap_or(body);
        return Err(AppError::drive_failure(format!(
            "{context} failed with HTTP {}: {}",
            status.as_u16(),
            redact_sensitive(&detail)
        )));
    }

    serde_json::from_str::<Value>(&body).map_err(|error| {
        AppError::drive_failure(format!("{context} returned invalid JSON: {error}"))
    })
}

fn parse_drive_bytes_response(
    response: Response,
    context: &str,
    not_found: Option<(&str, &str)>,
) -> Result<Vec<u8>, AppError> {
    let status = response.status();
    if status.as_u16() == 404
        && let Some((entity, id)) = not_found
    {
        return Err(AppError::drive_not_found(entity, id));
    }

    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        let detail = extract_error_message(&body).unwrap_or(body);
        return Err(AppError::drive_failure(format!(
            "{context} failed with HTTP {}: {}",
            status.as_u16(),
            redact_sensitive(&detail)
        )));
    }

    response
        .bytes()
        .map(|value| value.to_vec())
        .map_err(|error| {
            AppError::drive_failure(format!(
                "{context} succeeded but failed reading bytes: {error}"
            ))
        })
}

fn extract_error_message(body: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    let message = parsed
        .get("error")
        .and_then(Value::as_object)
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
        .or_else(|| parsed.get("error_description").and_then(Value::as_str))
        .or_else(|| parsed.get("error").and_then(Value::as_str))?;
    Some(message.to_string())
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

fn view_from_live_json(file: &Value) -> FileView {
    let size_bytes = file
        .get("size")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default();
    FileView {
        id: file
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: file
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        mime_type: file
            .get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream")
            .to_string(),
        size_bytes,
        parents: file
            .get("parents")
            .and_then(Value::as_array)
            .map(|parents| {
                parents
                    .iter()
                    .filter_map(|parent| parent.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
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

fn load_fixture_store() -> Result<Option<DriveFixtureStore>, AppError> {
    if let Ok(path) = env::var(GOOGLE_CLI_DRIVE_FIXTURE_PATH_ENV) {
        let bytes = fs::read(&path).map_err(|error| {
            AppError::drive_failure(format!(
                "failed to read Drive fixture file `{path}`: {error}"
            ))
        })?;
        let store = serde_json::from_slice::<DriveFixtureStore>(&bytes).map_err(|error| {
            AppError::drive_failure(format!(
                "failed to parse Drive fixture file `{path}`: {error}"
            ))
        })?;
        return Ok(Some(store));
    }

    if let Ok(raw) = env::var(GOOGLE_CLI_DRIVE_FIXTURE_JSON_ENV) {
        let store = serde_json::from_str::<DriveFixtureStore>(&raw).map_err(|error| {
            AppError::drive_failure(format!(
                "failed to parse Drive fixture JSON from {GOOGLE_CLI_DRIVE_FIXTURE_JSON_ENV}: {error}"
            ))
        })?;
        return Ok(Some(store));
    }

    Ok(None)
}

fn build_search_query(query: &str) -> String {
    let mut clauses = Vec::new();
    for token in query.split_whitespace() {
        if token.trim().is_empty() {
            continue;
        }
        let clause = match token.split_once(':') {
            Some(("name", value)) => format!("name contains '{}'", escape_drive_literal(value)),
            Some(("mime", value)) => {
                format!("mimeType contains '{}'", escape_drive_literal(value))
            }
            Some(("id", value)) => format!("id = '{}'", escape_drive_literal(value)),
            Some(("parent", value)) => format!("'{}' in parents", escape_drive_literal(value)),
            _ => {
                let escaped = escape_drive_literal(token);
                format!("(name contains '{escaped}' or fullText contains '{escaped}')")
            }
        };
        clauses.push(clause);
    }
    clauses.join(" and ")
}

fn escape_drive_literal(value: &str) -> String {
    value.replace('\'', "\\'")
}

fn resolve_export_mime_type(format: &str) -> Option<String> {
    let trimmed = format.trim();
    if trimmed.contains('/') {
        return Some(trimmed.to_string());
    }

    match trimmed.to_ascii_lowercase().as_str() {
        "pdf" => Some("application/pdf".to_string()),
        "txt" | "text" => Some("text/plain".to_string()),
        "html" => Some("text/html".to_string()),
        "csv" => Some("text/csv".to_string()),
        "json" => Some("application/json".to_string()),
        "docx" => Some(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
        ),
        "xlsx" => {
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string())
        }
        "pptx" => Some(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string(),
        ),
        _ => None,
    }
}

fn convert_target_mime(source_mime: &str) -> Option<&'static str> {
    match source_mime {
        "text/plain" => Some("application/vnd.google-apps.document"),
        "text/csv" => Some("application/vnd.google-apps.spreadsheet"),
        "application/vnd.ms-excel" => Some("application/vnd.google-apps.spreadsheet"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            Some("application/vnd.google-apps.spreadsheet")
        }
        "application/vnd.ms-powerpoint" => Some("application/vnd.google-apps.presentation"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            Some("application/vnd.google-apps.presentation")
        }
        "application/msword" => Some("application/vnd.google-apps.document"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            Some("application/vnd.google-apps.document")
        }
        _ => None,
    }
}

fn upload_to_fixture(
    session: &DriveSession,
    request: &UploadRequest,
) -> Result<UploadResult, AppError> {
    let Some(fixture) = &session.fixture else {
        return Err(AppError::drive_failure(
            "fixture upload requested without fixture store",
        ));
    };

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
        fixture
            .files
            .iter()
            .find(|file| file.name == upload_name && parent_matches(file, Some(parent.as_str())))
    } else {
        None
    };

    let id = replaced.map(|file| file.id.clone()).unwrap_or_else(|| {
        synthetic_file_id(&session.account, &upload_name, &parent, &inferred_mime_type)
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
