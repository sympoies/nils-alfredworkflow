use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response, multipart};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use sha2::{Digest, Sha256};
use workflow_common::http::build_blocking_client;

use crate::auth::account::resolve_account;
use crate::auth::config::{AuthPaths, load_credentials, load_metadata};
use crate::auth::oauth;
use crate::auth::store::{load_token, persist_token};
use crate::cmd::common::GlobalOptions;
use crate::error::{AppError, redact_sensitive};

use super::mime::resolve_mime_type;

const DRIVE_API_BASE: &str = "https://www.googleapis.com/drive/v3";
const DRIVE_UPLOAD_BASE: &str = "https://www.googleapis.com/upload/drive/v3";
// Bound Drive metadata calls so a stalled server cannot hang the CLI indefinitely.
const DRIVE_METADATA_TIMEOUT: Duration = Duration::from_secs(15);
// Drive media transfers can legitimately exceed metadata latency on slow links.
const DRIVE_MEDIA_TIMEOUT: Duration = Duration::from_secs(300);
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
    pub md5_checksum: Option<String>,
    #[serde(default)]
    pub sha256_checksum: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub modified_time: Option<String>,
    #[serde(default)]
    pub trashed: bool,
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
    pub md5_checksum: Option<String>,
    pub sha256_checksum: Option<String>,
    pub version: Option<String>,
    pub modified_time: Option<String>,
    pub trashed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DrivePage {
    pub files: Vec<FileView>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DriveSession {
    pub account: String,
    pub account_source: String,
    pub access_token: String,
    client: Client,
    fixture: Option<DriveFixtureStore>,
    api_base: String,
    upload_base: String,
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

        let client =
            build_blocking_client(None, Some(DRIVE_METADATA_TIMEOUT)).map_err(|error| {
                AppError::drive_failure(format!("failed to build Drive HTTP client: {error}"))
            })?;

        Ok(Self {
            account: resolved.account,
            account_source: resolved.source.as_str().to_string(),
            access_token: active_token.access_token,
            client,
            fixture,
            api_base: DRIVE_API_BASE.to_string(),
            upload_base: DRIVE_UPLOAD_BASE.to_string(),
        })
    }

    pub fn list(&self, request: &ListRequest) -> Result<Vec<FileView>, AppError> {
        Ok(self.list_page(request, false)?.files)
    }

    pub fn list_page(
        &self,
        request: &ListRequest,
        all_drives: bool,
    ) -> Result<DrivePage, AppError> {
        if let Some(fixture) = &self.fixture {
            let files = fixture
                .files
                .iter()
                .filter(|file| !file.trashed)
                .filter(|file| parent_matches(file, request.parent.as_deref()))
                .filter(|file| query_matches(file, request.query.as_deref().unwrap_or_default()))
                .map(view_for_file)
                .collect::<Vec<_>>();
            return fixture_page(files, request.max, request.page_token.as_deref());
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

        self.list_live(&q, request.max, request.page_token.as_deref(), all_drives)
    }

    pub fn search(&self, request: &SearchRequest) -> Result<Vec<FileView>, AppError> {
        Ok(self.search_page(request, false)?.files)
    }

    pub fn search_page(
        &self,
        request: &SearchRequest,
        all_drives: bool,
    ) -> Result<DrivePage, AppError> {
        if let Some(fixture) = &self.fixture {
            let files = fixture
                .files
                .iter()
                .filter(|file| !file.trashed)
                .filter(|file| query_matches(file, request.query.as_str()))
                .map(view_for_file)
                .collect::<Vec<_>>();
            return fixture_page(files, request.max, request.page_token.as_deref());
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

        self.list_live(&q, request.max, request.page_token.as_deref(), all_drives)
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
                "files/{}?fields=id,name,mimeType,size,parents,md5Checksum,sha256Checksum,version,modifiedTime,trashed&supportsAllDrives=true",
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
        max_bytes: Option<usize>,
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

                let bytes = content.as_bytes();
                check_download_size(bytes.len(), max_bytes)?;
                return Ok(DownloadPayload {
                    file_id: file.id.clone(),
                    file_name: file.name.clone(),
                    mime_type: file.mime_type.clone(),
                    format: Some(format.to_string()),
                    source: "export",
                    bytes: bytes.to_vec(),
                });
            }

            let bytes = file.content.as_bytes();
            check_download_size(bytes.len(), max_bytes)?;
            return Ok(DownloadPayload {
                file_id: file.id.clone(),
                file_name: file.name.clone(),
                mime_type: file.mime_type.clone(),
                format: None,
                source: "download",
                bytes: bytes.to_vec(),
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
            let response = media_request(
                self.client
                    .get(format!("{}/files/{file_id}/export", self.api_base))
                    .bearer_auth(&self.access_token)
                    .query(&[
                        ("mimeType", export_mime.as_str()),
                        ("supportsAllDrives", "true"),
                    ]),
            )
            .send()
            .map_err(|error| {
                AppError::drive_failure(format!("GET files/{file_id}/export failed: {error}"))
            })?;
            let bytes = parse_drive_bytes_response(
                response,
                format!("GET files/{file_id}/export").as_str(),
                Some(("file", file_id)),
                max_bytes,
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
            max_bytes,
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
                "{}/files/{}?uploadType=multipart&supportsAllDrives=true",
                self.upload_base, existing.id
            )
        } else {
            format!(
                "{}/files?uploadType=multipart&supportsAllDrives=true",
                self.upload_base
            )
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
        let response = media_request(
            request_builder
                .bearer_auth(&self.access_token)
                .multipart(form),
        )
        .send()
        .map_err(|error| AppError::drive_failure(format!("upload request failed: {error}")))?;
        let payload = parse_drive_json_response(response, "upload file", None)?;
        let file_id = payload.get("id").and_then(Value::as_str).ok_or_else(|| {
            AppError::drive_failure("upload succeeded without a file ID; read-back is unavailable")
        })?;
        let file = self.get(&GetRequest {
            file_id: file_id.to_string(),
        })?;

        Ok(UploadResult {
            replaced: replaced.is_some(),
            replaced_file_id: replaced.map(|file| file.id),
            inferred_mime_type,
            source_path: source_path.display().to_string(),
            convert_requested: request.convert,
            file,
        })
    }

    pub fn create_folder(&self, name: &str, parent: &str) -> Result<FileView, AppError> {
        if self.fixture.is_some() {
            return Ok(FileView {
                id: synthetic_file_id(
                    &self.account,
                    name,
                    parent,
                    "application/vnd.google-apps.folder",
                ),
                name: name.to_string(),
                mime_type: "application/vnd.google-apps.folder".to_string(),
                size_bytes: 0,
                parents: vec![parent.to_string()],
                md5_checksum: None,
                sha256_checksum: None,
                version: Some("1".to_string()),
                modified_time: None,
                trashed: Some(false),
            });
        }
        let response = metadata_request(
            self.client.post(format!("{}/files", self.api_base))
                .bearer_auth(&self.access_token)
                .query(&[("supportsAllDrives", "true")])
                .json(&json!({"name": name, "mimeType": "application/vnd.google-apps.folder", "parents": [parent]})),
        ).send().map_err(|error| AppError::drive_failure(format!("create folder request failed: {error}")))?;
        self.write_and_read(response, "create folder", None, None)
    }

    pub fn update_content(
        &self,
        file_id: &str,
        local_path: PathBuf,
        mime_type: Option<&str>,
    ) -> Result<FileView, AppError> {
        if !local_path.is_file() {
            return Err(AppError::invalid_drive_input("update source is not a file"));
        }
        let mime_type = resolve_mime_type(&local_path, mime_type)?;
        if self.fixture.is_some() {
            let bytes = fs::read(&local_path).map_err(|error| {
                AppError::drive_failure(format!("failed to read update source: {error}"))
            })?;
            let mut file = self.get(&GetRequest {
                file_id: file_id.to_string(),
            })?;
            file.size_bytes = bytes.len() as u64;
            file.mime_type = mime_type;
            file.sha256_checksum = Some(hex::encode(Sha256::digest(&bytes)));
            file.version = Some(next_fixture_version(file.version.as_deref()));
            return Ok(file);
        }
        let source = fs::File::open(&local_path).map_err(|error| {
            AppError::drive_failure(format!("failed to open update source: {error}"))
        })?;
        let source_len = source
            .metadata()
            .map_err(|error| {
                AppError::drive_failure(format!("failed to inspect update source: {error}"))
            })?
            .len();
        let file_part = multipart::Part::reader_with_length(source, source_len)
            .mime_str(&mime_type)
            .map_err(|error| {
                AppError::invalid_drive_input(format!("invalid update MIME type: {error}"))
            })?;
        let metadata_part = multipart::Part::text("{}")
            .mime_str("application/json; charset=utf-8")
            .map_err(|error| {
                AppError::drive_failure(format!("invalid update metadata: {error}"))
            })?;
        let response = media_request(
            self.client
                .patch(format!("{}/files/{file_id}", self.upload_base))
                .bearer_auth(&self.access_token)
                .query(&[("uploadType", "multipart"), ("supportsAllDrives", "true")])
                .multipart(
                    multipart::Form::new()
                        .part("metadata", metadata_part)
                        .part("file", file_part),
                ),
        )
        .send()
        .map_err(|error| {
            AppError::drive_failure(format!("update content request failed: {error}"))
        })?;
        self.write_and_read(response, "update content", Some(file_id), Some(file_id))
    }

    pub fn rename(&self, file_id: &str, name: &str) -> Result<FileView, AppError> {
        if self.fixture.is_some() {
            let mut file = self.get(&GetRequest {
                file_id: file_id.to_string(),
            })?;
            file.name = name.to_string();
            file.version = Some(next_fixture_version(file.version.as_deref()));
            return Ok(file);
        }
        self.patch_metadata(
            file_id,
            json!({"name": name}),
            &[],
            "rename file",
            Some(file_id),
        )
    }

    pub fn move_file(&self, file_id: &str, parent: &str, from: &str) -> Result<FileView, AppError> {
        let current = self.get(&GetRequest {
            file_id: file_id.to_string(),
        })?;
        if !current.parents.iter().any(|existing| existing == from) {
            return Err(AppError::invalid_drive_input(
                "--from is not a current parent",
            ));
        }
        if self.fixture.is_some() {
            let mut file = current;
            file.parents = vec![parent.to_string()];
            file.version = Some(next_fixture_version(file.version.as_deref()));
            return Ok(file);
        }
        self.patch_metadata(
            file_id,
            json!({}),
            &[("addParents", parent), ("removeParents", from)],
            "move file",
            None,
        )
    }

    pub fn copy_file(
        &self,
        file_id: &str,
        parent: &str,
        name: Option<&str>,
    ) -> Result<FileView, AppError> {
        if self.fixture.is_some() {
            let mut file = self.get(&GetRequest {
                file_id: file_id.to_string(),
            })?;
            file.name = name.unwrap_or(&file.name).to_string();
            file.id = synthetic_file_id(&self.account, &file.name, parent, &file.mime_type);
            file.parents = vec![parent.to_string()];
            file.version = Some("1".to_string());
            return Ok(file);
        }
        let mut body = json!({"parents": [parent]});
        if let Some(name) = name {
            body["name"] = Value::String(name.to_string());
        }
        let response = metadata_request(
            self.client
                .post(format!("{}/files/{file_id}/copy", self.api_base))
                .bearer_auth(&self.access_token)
                .query(&[("supportsAllDrives", "true")])
                .json(&body),
        )
        .send()
        .map_err(|error| AppError::drive_failure(format!("copy file request failed: {error}")))?;
        self.write_and_read(response, "copy file", None, None)
    }

    pub fn set_trashed(&self, file_id: &str, trashed: bool) -> Result<FileView, AppError> {
        if self.fixture.is_some() {
            let mut file = self.get(&GetRequest {
                file_id: file_id.to_string(),
            })?;
            file.trashed = Some(trashed);
            file.version = Some(next_fixture_version(file.version.as_deref()));
            return Ok(file);
        }
        self.patch_metadata(
            file_id,
            json!({"trashed": trashed}),
            &[],
            if trashed {
                "trash file"
            } else {
                "untrash file"
            },
            Some(file_id),
        )
    }

    fn patch_metadata(
        &self,
        file_id: &str,
        body: Value,
        extra_query: &[(&str, &str)],
        context: &str,
        not_found_id: Option<&str>,
    ) -> Result<FileView, AppError> {
        let mut query = vec![("supportsAllDrives", "true")];
        query.extend_from_slice(extra_query);
        let response = metadata_request(
            self.client
                .patch(format!("{}/files/{file_id}", self.api_base))
                .bearer_auth(&self.access_token)
                .query(&query)
                .json(&body),
        )
        .send()
        .map_err(|error| AppError::drive_failure(format!("{context} request failed: {error}")))?;
        self.write_and_read(response, context, Some(file_id), not_found_id)
    }

    fn write_and_read(
        &self,
        response: Response,
        context: &str,
        fallback_id: Option<&str>,
        not_found_id: Option<&str>,
    ) -> Result<FileView, AppError> {
        let payload =
            parse_drive_json_response(response, context, not_found_id.map(|id| ("file", id)))?;
        let file_id = payload
            .get("id")
            .and_then(Value::as_str)
            .or(fallback_id)
            .ok_or_else(|| {
                AppError::drive_failure(format!(
                    "{context} succeeded without a file ID; read-back is unavailable"
                ))
            })?;
        self.get(&GetRequest {
            file_id: file_id.to_string(),
        })
    }

    fn list_live(
        &self,
        query: &str,
        max: usize,
        page_token: Option<&str>,
        all_drives: bool,
    ) -> Result<DrivePage, AppError> {
        let params = list_params(query, max, page_token, all_drives);

        let response = metadata_request(
            self.client
                .get(format!("{}/files", self.api_base))
                .bearer_auth(&self.access_token)
                .query(&params),
        )
        .send()
        .map_err(|error| AppError::drive_failure(format!("list files request failed: {error}")))?;
        let payload = parse_drive_json_response(response, "list files", None)?;
        parse_list_payload(&payload)
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
        let page = self.list_live(&query, 1, None, false)?;
        Ok(page.files.into_iter().next())
    }

    fn drive_get_json(
        &self,
        path_and_query: &str,
        not_found: Option<(&str, &str)>,
    ) -> Result<Value, AppError> {
        let url = format!("{}/{path_and_query}", self.api_base);
        let response = metadata_request(self.client.get(&url).bearer_auth(&self.access_token))
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
        max_bytes: Option<usize>,
    ) -> Result<Vec<u8>, AppError> {
        let url = format!("{}/{path_and_query}", self.api_base);
        let response = media_request(self.client.get(&url).bearer_auth(&self.access_token))
            .send()
            .map_err(|error| AppError::drive_failure(format!("GET {url} failed: {error}")))?;
        parse_drive_bytes_response(
            response,
            format!("GET {path_and_query}").as_str(),
            not_found,
            max_bytes,
        )
    }
}

fn next_fixture_version(current: Option<&str>) -> String {
    current
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .saturating_add(1)
        .to_string()
}

fn metadata_request(request: RequestBuilder) -> RequestBuilder {
    request.timeout(DRIVE_METADATA_TIMEOUT)
}

fn media_request(request: RequestBuilder) -> RequestBuilder {
    request.timeout(DRIVE_MEDIA_TIMEOUT)
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
    mut response: Response,
    context: &str,
    not_found: Option<(&str, &str)>,
    max_bytes: Option<usize>,
) -> Result<Vec<u8>, AppError> {
    let status = response.status();
    if status.as_u16() == 404
        && let Some((entity, id)) = not_found
    {
        return Err(AppError::drive_not_found(entity, id));
    }

    if !status.is_success() {
        let mut body_bytes = Vec::new();
        let _ = response.take(16_384).read_to_end(&mut body_bytes);
        let body = String::from_utf8_lossy(&body_bytes).into_owned();
        let detail = extract_error_message(&body).unwrap_or(body);
        return Err(AppError::drive_failure(format!(
            "{context} failed with HTTP {}: {}",
            status.as_u16(),
            redact_sensitive(&detail)
        )));
    }

    if let Some(max) = max_bytes {
        if response
            .content_length()
            .is_some_and(|length| length > max as u64)
        {
            return Err(AppError::drive_size_limit(max));
        }
        return read_bounded_bytes(&mut response, max, context);
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

fn check_download_size(length: usize, max_bytes: Option<usize>) -> Result<(), AppError> {
    if let Some(max) = max_bytes
        && length > max
    {
        return Err(AppError::drive_size_limit(max));
    }
    Ok(())
}

fn read_bounded_bytes(
    reader: &mut impl Read,
    max: usize,
    context: &str,
) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    reader
        .take((max as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            AppError::drive_failure(format!(
                "{context} succeeded but failed reading bytes: {error}"
            ))
        })?;
    check_download_size(bytes.len(), Some(max))?;
    Ok(bytes)
}

#[cfg(test)]
mod bounded_download_tests {
    use std::io::{self, Read};

    use super::read_bounded_bytes;

    struct CountingReader {
        remaining: usize,
        consumed: usize,
    }

    impl Read for CountingReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let count = buf.len().min(self.remaining);
            buf[..count].fill(b'x');
            self.remaining -= count;
            self.consumed += count;
            Ok(count)
        }
    }

    #[test]
    fn bounded_reader_never_consumes_more_than_limit_plus_one() {
        let mut reader = CountingReader {
            remaining: 10_000_000,
            consumed: 0,
        };
        assert!(read_bounded_bytes(&mut reader, 120_000, "fixture").is_err());
        assert_eq!(reader.consumed, 120_001);
    }
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
        md5_checksum: file.md5_checksum.clone(),
        sha256_checksum: file.sha256_checksum.clone(),
        version: file.version.clone(),
        modified_time: file.modified_time.clone(),
        trashed: Some(file.trashed),
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
        md5_checksum: file
            .get("md5Checksum")
            .and_then(Value::as_str)
            .map(str::to_owned),
        sha256_checksum: file
            .get("sha256Checksum")
            .and_then(Value::as_str)
            .map(str::to_owned),
        version: file
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_owned),
        modified_time: file
            .get("modifiedTime")
            .and_then(Value::as_str)
            .map(str::to_owned),
        trashed: file.get("trashed").and_then(Value::as_bool),
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

fn list_params(
    query: &str,
    max: usize,
    page_token: Option<&str>,
    all_drives: bool,
) -> Vec<(&'static str, String)> {
    let mut params = vec![
        ("q", query.to_string()),
        ("pageSize", max.to_string()),
        (
            "fields",
            "nextPageToken,incompleteSearch,files(id,name,mimeType,size,parents,md5Checksum,sha256Checksum,version,modifiedTime,trashed)".to_string(),
        ),
        ("supportsAllDrives", "true".to_string()),
        ("includeItemsFromAllDrives", "true".to_string()),
    ];
    if let Some(token) = page_token {
        params.push(("pageToken", token.to_string()));
    }
    if all_drives {
        params.push(("corpora", "allDrives".to_string()));
    }
    params
}

fn parse_list_payload(payload: &Value) -> Result<DrivePage, AppError> {
    if payload.get("incompleteSearch").and_then(Value::as_bool) == Some(true) {
        return Err(AppError::drive_failure(
            "Drive search incomplete across shared drives; account-wide completeness unavailable for this response",
        ));
    }
    let files = payload
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|file| view_from_live_json(&file))
        .collect();
    Ok(DrivePage {
        files,
        next_page_token: payload
            .get("nextPageToken")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

fn fixture_page(
    files: Vec<FileView>,
    max: usize,
    page_token: Option<&str>,
) -> Result<DrivePage, AppError> {
    let offset = page_token
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| AppError::invalid_drive_input("invalid Drive fixture page token"))
        })
        .transpose()?
        .unwrap_or(0);
    if offset > files.len() {
        return Err(AppError::invalid_drive_input(
            "invalid Drive fixture page token",
        ));
    }
    let end = offset.saturating_add(max).min(files.len());
    Ok(DrivePage {
        files: files[offset..end].to_vec(),
        next_page_token: (end < files.len()).then(|| end.to_string()),
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
        md5_checksum: None,
        sha256_checksum: Some(hex::encode(Sha256::digest(
            fs::read(&source_path).map_err(|error| {
                AppError::drive_failure(format!("failed to read upload source: {error}"))
            })?,
        ))),
        version: Some(next_fixture_version(
            replaced.and_then(|file| file.version.as_deref()),
        )),
        modified_time: None,
        trashed: Some(false),
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

#[cfg(test)]
mod tests {
    use super::{
        DRIVE_MEDIA_TIMEOUT, DRIVE_METADATA_TIMEOUT, DriveSession, UploadRequest, list_params,
        media_request, metadata_request, parse_list_payload, view_from_live_json,
    };

    use reqwest::blocking::Client;
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    fn mock_session(base: String) -> DriveSession {
        DriveSession {
            account: "test@example.com".to_string(),
            account_source: "explicit".to_string(),
            access_token: "test-token".to_string(),
            client: Client::new(),
            fixture: None,
            api_base: base.clone(),
            upload_base: base,
        }
    }

    #[tokio::test]
    async fn live_mkdir_returns_get_metadata_not_create_echo() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/files"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"id": "new-id", "parents": []})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/files/new-id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "new-id", "name": "new", "mimeType": "application/vnd.google-apps.folder",
                "parents": ["parent-id"], "version": "7", "trashed": false
            })))
            .expect(1)
            .mount(&server)
            .await;

        let base = server.uri();
        let file = tokio::task::spawn_blocking(move || {
            mock_session(base).create_folder("new", "parent-id")
        })
        .await
        .expect("join")
        .expect("create folder");
        assert_eq!(file.parents, ["parent-id"]);
        assert_eq!(file.version.as_deref(), Some("7"));
        let requests = server.received_requests().await.expect("recorded requests");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method.as_str(), "POST");
        assert_eq!(requests[0].url.path(), "/files");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&requests[0].body).expect("body"),
            json!({"name": "new", "mimeType": "application/vnd.google-apps.folder", "parents": ["parent-id"]})
        );
        assert_eq!(requests[1].method.as_str(), "GET");
    }

    #[tokio::test]
    async fn live_upload_rechecks_file_metadata() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/files"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"id": "uploaded-id", "parents": [], "size": "0"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/files/uploaded-id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "uploaded-id", "name": "report.txt", "mimeType": "text/plain",
                "parents": ["parent-id"], "size": "9", "version": "3", "trashed": false
            })))
            .expect(1)
            .mount(&server)
            .await;
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("report.txt");
        std::fs::write(&source, b"new bytes").expect("source");
        let base = server.uri();
        let result = tokio::task::spawn_blocking(move || {
            mock_session(base).upload(&UploadRequest {
                local_path: source,
                parent: Some("parent-id".to_string()),
                name: None,
                mime_type: Some("text/plain".to_string()),
                replace: false,
                convert: false,
            })
        })
        .await
        .expect("join")
        .expect("upload");
        assert_eq!(result.file.parents, ["parent-id"]);
        assert_eq!(result.file.size_bytes, 9);
        let requests = server.received_requests().await.expect("recorded requests");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method.as_str(), "POST");
        assert_eq!(
            requests[0]
                .url
                .query_pairs()
                .find(|(key, _)| key == "uploadType")
                .map(|(_, value)| value.into_owned())
                .as_deref(),
            Some("multipart")
        );
        assert_eq!(requests[1].method.as_str(), "GET");
    }

    #[tokio::test]
    async fn live_write_methods_recheck_server_metadata() {
        for action in ["rename", "copy", "trash", "untrash", "update"] {
            let server = MockServer::start().await;
            let copied = action == "copy";
            let target = if copied { "copied-id" } else { "file-1" };
            let method_name = if copied { "POST" } else { "PATCH" };
            let mutation_path = if copied {
                "/files/file-1/copy"
            } else {
                "/files/file-1"
            };
            Mock::given(method(method_name))
                .and(path(mutation_path))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(json!({"id": target, "name": "stale"})),
                )
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path(format!("/files/{target}")))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "id": target, "name": "authoritative", "mimeType": "text/plain",
                    "parents": ["parent-id"], "size": "9", "version": "77", "trashed": action == "trash"
                })))
                .expect(1)
                .mount(&server).await;
            let temp = tempfile::tempdir().expect("tempdir");
            let source = temp.path().join("replacement.txt");
            std::fs::write(&source, b"new bytes").expect("source");
            let base = server.uri();
            let file = tokio::task::spawn_blocking(move || {
                let session = mock_session(base);
                match action {
                    "rename" => session.rename("file-1", "new-name"),
                    "copy" => session.copy_file("file-1", "parent-id", Some("new-copy")),
                    "trash" => session.set_trashed("file-1", true),
                    "untrash" => session.set_trashed("file-1", false),
                    "update" => session.update_content("file-1", source, Some("text/plain")),
                    _ => unreachable!(),
                }
            })
            .await
            .expect("join")
            .expect("write");
            assert_eq!(file.name, "authoritative", "{action}");
            assert_eq!(file.version.as_deref(), Some("77"), "{action}");
            let requests = server.received_requests().await.expect("recorded requests");
            assert_eq!(requests.len(), 2, "{action}");
            assert_eq!(requests[0].method.as_str(), method_name, "{action}");
            assert_eq!(requests[0].url.path(), mutation_path, "{action}");
            assert_eq!(requests[1].method.as_str(), "GET", "{action}");
            if action == "update" {
                assert_eq!(
                    requests[0]
                        .url
                        .query_pairs()
                        .find(|(key, _)| key == "uploadType")
                        .map(|(_, value)| value.into_owned())
                        .as_deref(),
                    Some("multipart")
                );
                assert!(String::from_utf8_lossy(&requests[0].body).contains("new bytes"));
            } else {
                let body = serde_json::from_slice::<serde_json::Value>(&requests[0].body)
                    .expect("JSON body");
                match action {
                    "rename" => assert_eq!(body, json!({"name": "new-name"})),
                    "copy" => {
                        assert_eq!(body, json!({"name": "new-copy", "parents": ["parent-id"]}))
                    }
                    "trash" => assert_eq!(body, json!({"trashed": true})),
                    "untrash" => assert_eq!(body, json!({"trashed": false})),
                    _ => unreachable!(),
                }
            }
        }
    }

    #[tokio::test]
    async fn live_move_checks_old_parent_and_uses_paired_parent_update() {
        let server = MockServer::start().await;
        let reads = Arc::new(AtomicUsize::new(0));
        let counts = reads.clone();
        Mock::given(method("GET"))
            .and(path("/files/file-1"))
            .respond_with(move |_: &wiremock::Request| {
                let first = counts.fetch_add(1, Ordering::SeqCst) == 0;
                ResponseTemplate::new(200).set_body_json(json!({
                    "id": "file-1", "name": "file.txt", "mimeType": "text/plain",
                    "parents": [if first { "old-id" } else { "new-id" }],
                    "version": if first { "10" } else { "11" }, "trashed": false
                }))
            })
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/files/file-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "file-1"})))
            .expect(1)
            .mount(&server)
            .await;
        let base = server.uri();
        let file = tokio::task::spawn_blocking(move || {
            mock_session(base).move_file("file-1", "new-id", "old-id")
        })
        .await
        .expect("join")
        .expect("move");
        assert_eq!(file.parents, ["new-id"]);
        assert_eq!(file.version.as_deref(), Some("11"));
        let requests = server.received_requests().await.expect("recorded requests");
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            ["GET", "PATCH", "GET"]
        );
        let query = requests[1]
            .url
            .query_pairs()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            query.get("addParents").map(|value| value.as_ref()),
            Some("new-id")
        );
        assert_eq!(
            query.get("removeParents").map(|value| value.as_ref()),
            Some("old-id")
        );
    }

    #[tokio::test]
    async fn live_write_failures_preserve_missing_target_and_readback_uncertainty() {
        let missing = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/files/file-1"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_json(json!({"error": {"message": "not found"}})),
            )
            .expect(1)
            .mount(&missing)
            .await;
        let base = missing.uri();
        let error = tokio::task::spawn_blocking(move || mock_session(base).rename("file-1", "new"))
            .await
            .expect("join")
            .expect_err("missing target");
        assert_eq!(error.code(), "NILS_GOOGLE_013");

        let absent_id = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"name": "stale"})))
            .expect(1)
            .mount(&absent_id)
            .await;
        let base = absent_id.uri();
        let error = tokio::task::spawn_blocking(move || {
            mock_session(base).create_folder("new", "parent-id")
        })
        .await
        .expect("join")
        .expect_err("missing created ID");
        assert_eq!(error.code(), "NILS_GOOGLE_014");
        assert_eq!(
            absent_id.received_requests().await.expect("requests").len(),
            1
        );

        let failed_readback = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/files/file-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "file-1"})))
            .expect(1)
            .mount(&failed_readback)
            .await;
        Mock::given(method("GET"))
            .and(path("/files/file-1"))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&failed_readback)
            .await;
        let base = failed_readback.uri();
        let error = tokio::task::spawn_blocking(move || mock_session(base).rename("file-1", "new"))
            .await
            .expect("join")
            .expect_err("readback failed");
        assert_eq!(error.code(), "NILS_GOOGLE_014");
        assert_eq!(
            failed_readback
                .received_requests()
                .await
                .expect("requests")
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn live_move_and_copy_do_not_attribute_ambiguous_404_to_source() {
        let move_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/files/file-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "file-1", "name": "source.txt", "mimeType": "text/plain",
                "parents": ["old-id"]
            })))
            .expect(1)
            .mount(&move_server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/files/file-1"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": {"message": "destination parent not found"}
            })))
            .expect(1)
            .mount(&move_server)
            .await;
        let base = move_server.uri();
        let error = tokio::task::spawn_blocking(move || {
            mock_session(base).move_file("file-1", "missing-id", "old-id")
        })
        .await
        .expect("join")
        .expect_err("missing destination");
        assert_eq!(error.code(), "NILS_GOOGLE_014");

        let copy_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/files/file-1/copy"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": {"message": "destination parent not found"}
            })))
            .expect(1)
            .mount(&copy_server)
            .await;
        let base = copy_server.uri();
        let error = tokio::task::spawn_blocking(move || {
            mock_session(base).copy_file("file-1", "missing-id", None)
        })
        .await
        .expect("join")
        .expect_err("missing destination");
        assert_eq!(error.code(), "NILS_GOOGLE_014");
    }

    #[test]
    fn get_metadata_preserves_write_verification_fields() {
        let view = view_from_live_json(&json!({
            "id": "file-1",
            "name": "result.txt",
            "mimeType": "text/plain",
            "size": "7",
            "parents": ["folder-1"],
            "md5Checksum": "md5-value",
            "sha256Checksum": "sha256-value",
            "version": "42",
            "modifiedTime": "2026-09-24T00:00:00Z",
            "trashed": false
        }));
        let value = serde_json::to_value(view).expect("view JSON");
        assert_eq!(value["md5_checksum"], "md5-value");
        assert_eq!(value["sha256_checksum"], "sha256-value");
        assert_eq!(value["version"], "42");
        assert_eq!(value["modified_time"], "2026-09-24T00:00:00Z");
        assert_eq!(value["trashed"], false);
    }

    #[test]
    fn drive_media_requests_use_longer_timeout_than_metadata_requests() {
        let client = Client::new();
        let metadata = metadata_request(client.get("https://example.invalid/metadata"))
            .build()
            .expect("metadata request");
        let media = media_request(client.get("https://example.invalid/media"))
            .build()
            .expect("media request");

        assert_eq!(metadata.timeout().copied(), Some(DRIVE_METADATA_TIMEOUT));
        assert_eq!(media.timeout().copied(), Some(DRIVE_MEDIA_TIMEOUT));
        assert!(DRIVE_MEDIA_TIMEOUT > DRIVE_METADATA_TIMEOUT);
    }

    #[test]
    fn live_list_parameters_bind_corpus_and_continuation() {
        let first = list_params("trashed = false", 100, None, true);
        assert!(first.contains(&("corpora", "allDrives".to_string())));
        assert!(!first.iter().any(|(key, _)| *key == "pageToken"));
        let second = list_params("trashed = false", 100, Some("opaque-page"), true);
        assert!(second.contains(&("pageToken", "opaque-page".to_string())));
        let default = list_params("trashed = false", 100, None, false);
        assert!(!default.iter().any(|(key, _)| *key == "corpora"));
    }

    #[test]
    fn live_list_payload_preserves_continuation_and_refuses_incomplete_search() {
        let page = parse_list_payload(&json!({
            "files": [{"id": "file-1", "name": "note", "mimeType": "text/plain"}],
            "nextPageToken": "opaque-page",
        }))
        .expect("complete first page");
        assert_eq!(page.files.len(), 1);
        assert_eq!(page.next_page_token.as_deref(), Some("opaque-page"));
        let error = parse_list_payload(&json!({
            "files": [], "incompleteSearch": true,
        }))
        .expect_err("incomplete account-wide search must fail");
        assert!(
            error
                .message()
                .contains("account-wide completeness unavailable")
        );
    }
}
