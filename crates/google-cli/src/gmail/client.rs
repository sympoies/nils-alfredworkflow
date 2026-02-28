use std::collections::{BTreeMap, BTreeSet, hash_map::DefaultHasher};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};

use base64::Engine as _;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth::account::resolve_account;
use crate::auth::config::{AuthPaths, load_credentials, load_metadata};
use crate::auth::oauth;
use crate::auth::store::{load_token, persist_token};
use crate::cmd::common::GlobalOptions;
use crate::error::{AppError, redact_sensitive};

const GMAIL_API_BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const GOOGLE_CLI_GMAIL_FIXTURE_PATH_ENV: &str = "GOOGLE_CLI_GMAIL_FIXTURE_PATH";
const GOOGLE_CLI_GMAIL_FIXTURE_JSON_ENV: &str = "GOOGLE_CLI_GMAIL_FIXTURE_JSON";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageFormat {
    Full,
    Metadata,
    Minimal,
}

impl MessageFormat {
    pub fn parse(input: &str) -> Result<Self, AppError> {
        match input {
            "full" => Ok(Self::Full),
            "metadata" => Ok(Self::Metadata),
            "minimal" => Ok(Self::Minimal),
            unknown => Err(AppError::invalid_gmail_input(format!(
                "unsupported --format value `{unknown}`; expected full|metadata|minimal"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Metadata => "metadata",
            Self::Minimal => "minimal",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GmailMessage {
    pub id: String,
    #[serde(default)]
    pub thread_id: String,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub label_ids: Vec<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GmailFixtureStore {
    #[serde(default)]
    pub messages: Vec<GmailMessage>,
}

#[derive(Debug, Clone)]
pub struct GmailSession {
    pub account: String,
    pub account_source: String,
    pub access_token: String,
    client: Client,
    fixture: Option<GmailFixtureStore>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageView {
    pub id: String,
    pub thread_id: String,
    pub snippet: String,
    pub label_ids: Vec<String>,
    pub headers: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadView {
    pub id: String,
    pub message_count: usize,
    pub messages: Vec<MessageView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadModifyResult {
    pub id: String,
    pub modified_message_count: usize,
    pub added_labels: Vec<String>,
    pub removed_labels: Vec<String>,
    pub labels_by_message: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: String,
    pub max: usize,
    pub page_token: Option<String>,
    pub format: MessageFormat,
    pub headers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GetRequest {
    pub message_id: String,
    pub format: MessageFormat,
    pub headers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ThreadGetRequest {
    pub thread_id: String,
    pub format: MessageFormat,
    pub headers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ThreadModifyRequest {
    pub thread_id: String,
    pub add_labels: Vec<String>,
    pub remove_labels: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SentMessage {
    pub id: String,
    pub thread_id: String,
}

impl GmailSession {
    pub fn from_global(global: &GlobalOptions) -> Result<Self, AppError> {
        let paths = AuthPaths::resolve()?;
        let metadata = load_metadata(&paths)?;
        let resolved = resolve_account(global.account.as_deref(), &metadata)?;
        let token = load_token(&paths, &resolved.account)?.ok_or_else(|| {
            AppError::invalid_gmail_input(format!(
                "account `{}` has no token; run `auth add {}` first",
                resolved.account, resolved.account
            ))
        })?;

        let fixture = load_fixture_store()?;
        let active_token = if fixture.is_some() {
            token
        } else {
            let credentials = load_credentials(&paths)?.ok_or_else(|| {
                AppError::invalid_gmail_input(
                    "OAuth credentials are not configured; run `auth credentials set --client-id <id> --client-secret <secret>` first",
                )
            })?;

            let refreshed = oauth::refresh_access_token(&resolved.account, &credentials, &token)
                .map_err(|error| {
                    AppError::gmail_failure(format!(
                        "failed to refresh OAuth token for `{}`: {}",
                        resolved.account,
                        error.message()
                    ))
                })?;

            if refreshed != token {
                persist_token(&paths, &resolved.account, &refreshed).map_err(|error| {
                    AppError::gmail_failure(format!(
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

    pub fn is_fixture_mode(&self) -> bool {
        self.fixture.is_some()
    }

    pub fn search(&self, request: &SearchRequest) -> Result<Vec<MessageView>, AppError> {
        if let Some(fixture) = &self.fixture {
            let mut results = fixture
                .messages
                .iter()
                .filter(|message| message_matches(message, request.query.as_str()))
                .map(|message| {
                    view_for_fixture_message(message, request.format, &request.headers, false)
                })
                .collect::<Vec<_>>();
            results.truncate(request.max);
            return Ok(results);
        }

        let mut query = vec![
            ("q", request.query.clone()),
            ("maxResults", request.max.to_string()),
        ];
        if let Some(token) = &request.page_token {
            query.push(("pageToken", token.clone()));
        }

        let response = self.gmail_get_json("messages", &query, None)?;
        let mut messages = Vec::new();
        let candidates = response
            .get("messages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        for candidate in candidates {
            if let Some(id) = candidate.get("id").and_then(Value::as_str) {
                let message =
                    self.fetch_live_message(id, request.format, &request.headers, false)?;
                messages.push(message);
            }
        }

        Ok(messages)
    }

    pub fn get(&self, request: &GetRequest) -> Result<MessageView, AppError> {
        if let Some(fixture) = &self.fixture {
            let message = fixture
                .messages
                .iter()
                .find(|message| message.id == request.message_id)
                .ok_or_else(|| AppError::gmail_not_found("message", &request.message_id))?;
            return Ok(view_for_fixture_message(
                message,
                request.format,
                &request.headers,
                true,
            ));
        }

        self.fetch_live_message(&request.message_id, request.format, &request.headers, true)
    }

    pub fn thread_get(&self, request: &ThreadGetRequest) -> Result<ThreadView, AppError> {
        if let Some(fixture) = &self.fixture {
            let messages = fixture
                .messages
                .iter()
                .filter(|message| message.thread_id == request.thread_id)
                .collect::<Vec<_>>();

            if messages.is_empty() {
                return Err(AppError::gmail_not_found("thread", &request.thread_id));
            }

            let rendered = messages
                .iter()
                .map(|message| {
                    view_for_fixture_message(message, request.format, &request.headers, false)
                })
                .collect::<Vec<_>>();

            return Ok(ThreadView {
                id: request.thread_id.clone(),
                message_count: rendered.len(),
                messages: rendered,
            });
        }

        let mut query = vec![("format", request.format.as_str().to_string())];
        for header in &request.headers {
            query.push(("metadataHeaders", header.clone()));
        }

        let response = self.gmail_get_json(
            format!("threads/{}", request.thread_id).as_str(),
            &query,
            Some(("thread", request.thread_id.as_str())),
        )?;

        let messages = response
            .get("messages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|message| view_for_live_message(&message, request.format, &request.headers, false))
            .collect::<Vec<_>>();

        if messages.is_empty() {
            return Err(AppError::gmail_not_found("thread", &request.thread_id));
        }

        Ok(ThreadView {
            id: request.thread_id.clone(),
            message_count: messages.len(),
            messages,
        })
    }

    pub fn thread_modify(
        &self,
        request: &ThreadModifyRequest,
    ) -> Result<ThreadModifyResult, AppError> {
        if let Some(fixture) = &self.fixture {
            let messages = fixture
                .messages
                .iter()
                .filter(|message| message.thread_id == request.thread_id)
                .collect::<Vec<_>>();

            if messages.is_empty() {
                return Err(AppError::gmail_not_found("thread", &request.thread_id));
            }

            let mut labels_by_message = BTreeMap::new();
            for message in messages {
                let mut labels = message.label_ids.iter().cloned().collect::<BTreeSet<_>>();
                for label in &request.add_labels {
                    labels.insert(label.clone());
                }
                for label in &request.remove_labels {
                    labels.remove(label);
                }
                labels_by_message.insert(message.id.clone(), labels.into_iter().collect());
            }

            return Ok(ThreadModifyResult {
                id: request.thread_id.clone(),
                modified_message_count: labels_by_message.len(),
                added_labels: request.add_labels.clone(),
                removed_labels: request.remove_labels.clone(),
                labels_by_message,
            });
        }

        let payload = json!({
            "addLabelIds": request.add_labels,
            "removeLabelIds": request.remove_labels,
        });

        let response = self.gmail_post_json(
            format!("threads/{}/modify", request.thread_id).as_str(),
            payload,
            Some(("thread", request.thread_id.as_str())),
        )?;

        let mut labels_by_message = BTreeMap::new();
        let messages = response
            .get("messages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for message in messages {
            let id = message
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if id.is_empty() {
                continue;
            }
            let labels = message
                .get("labelIds")
                .and_then(Value::as_array)
                .map(|labels| {
                    labels
                        .iter()
                        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            labels_by_message.insert(id, labels);
        }

        Ok(ThreadModifyResult {
            id: request.thread_id.clone(),
            modified_message_count: labels_by_message.len(),
            added_labels: request.add_labels.clone(),
            removed_labels: request.remove_labels.clone(),
            labels_by_message,
        })
    }

    pub fn send_raw_message(
        &self,
        raw_rfc822: &[u8],
        thread_id: Option<&str>,
    ) -> Result<SentMessage, AppError> {
        if self.fixture.is_some() {
            let id = synthetic_message_id(&self.account, raw_rfc822);
            let thread_id = thread_id
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| format!("thread-{id}"));
            return Ok(SentMessage { id, thread_id });
        }

        let mut payload = json!({
            "raw": URL_SAFE_NO_PAD.encode(raw_rfc822),
        });
        if let Some(thread_id) = thread_id {
            payload["threadId"] = Value::String(thread_id.to_string());
        }

        let response = self.gmail_post_json("messages/send", payload, None)?;
        let id = response
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::gmail_failure("Gmail send response missing message id"))?
            .to_string();
        let thread_id = response
            .get("threadId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("thread-{id}"));
        Ok(SentMessage { id, thread_id })
    }

    fn fetch_live_message(
        &self,
        message_id: &str,
        format: MessageFormat,
        selected_headers: &[String],
        include_body_for_get: bool,
    ) -> Result<MessageView, AppError> {
        let mut query = vec![("format", format.as_str().to_string())];
        for header in selected_headers {
            query.push(("metadataHeaders", header.clone()));
        }

        let response = self.gmail_get_json(
            format!("messages/{message_id}").as_str(),
            &query,
            Some(("message", message_id)),
        )?;

        Ok(view_for_live_message(
            &response,
            format,
            selected_headers,
            include_body_for_get,
        ))
    }

    fn gmail_get_json(
        &self,
        path: &str,
        query: &[(&str, String)],
        not_found: Option<(&str, &str)>,
    ) -> Result<Value, AppError> {
        let url = format!("{GMAIL_API_BASE}/{path}");
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(query)
            .send()
            .map_err(|error| AppError::gmail_failure(format!("GET {url} failed: {error}")))?;
        parse_gmail_response(response, format!("GET {path}").as_str(), not_found)
    }

    fn gmail_post_json(
        &self,
        path: &str,
        payload: Value,
        not_found: Option<(&str, &str)>,
    ) -> Result<Value, AppError> {
        let url = format!("{GMAIL_API_BASE}/{path}");
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&payload)
            .send()
            .map_err(|error| AppError::gmail_failure(format!("POST {url} failed: {error}")))?;
        parse_gmail_response(response, format!("POST {path}").as_str(), not_found)
    }
}

fn parse_gmail_response(
    response: Response,
    context: &str,
    not_found: Option<(&str, &str)>,
) -> Result<Value, AppError> {
    let status = response.status();
    let body = response.text().map_err(|error| {
        AppError::gmail_failure(format!("{context} failed reading body: {error}"))
    })?;

    if status.as_u16() == 404
        && let Some((entity, id)) = not_found
    {
        return Err(AppError::gmail_not_found(entity, id));
    }

    if !status.is_success() {
        let detail = extract_error_message(&body).unwrap_or(body);
        return Err(AppError::gmail_failure(format!(
            "{context} failed with HTTP {}: {}",
            status.as_u16(),
            redact_sensitive(&detail)
        )));
    }

    serde_json::from_str::<Value>(&body).map_err(|error| {
        AppError::gmail_failure(format!("{context} returned invalid JSON: {error}"))
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

fn view_for_fixture_message(
    message: &GmailMessage,
    format: MessageFormat,
    selected_headers: &[String],
    include_body_for_get: bool,
) -> MessageView {
    let headers = match format {
        MessageFormat::Minimal => BTreeMap::new(),
        MessageFormat::Metadata => select_headers(&message.headers, selected_headers),
        MessageFormat::Full => message.headers.clone(),
    };

    let body = if include_body_for_get && matches!(format, MessageFormat::Full) {
        Some(message.body.clone())
    } else {
        None
    };

    MessageView {
        id: message.id.clone(),
        thread_id: message.thread_id.clone(),
        snippet: message.snippet.clone(),
        label_ids: message.label_ids.clone(),
        headers,
        body,
    }
}

fn view_for_live_message(
    message: &Value,
    format: MessageFormat,
    selected_headers: &[String],
    include_body_for_get: bool,
) -> MessageView {
    let headers = extract_headers(message.get("payload"));
    let headers = match format {
        MessageFormat::Minimal => BTreeMap::new(),
        MessageFormat::Metadata => select_headers(&headers, selected_headers),
        MessageFormat::Full => headers,
    };

    let body = if include_body_for_get && matches!(format, MessageFormat::Full) {
        message
            .get("payload")
            .and_then(extract_body_from_payload)
            .or_else(|| {
                message
                    .get("snippet")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
    } else {
        None
    };

    MessageView {
        id: message
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        thread_id: message
            .get("threadId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        snippet: message
            .get("snippet")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        label_ids: message
            .get("labelIds")
            .and_then(Value::as_array)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.as_str().map(ToOwned::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        headers,
        body,
    }
}

fn extract_headers(payload: Option<&Value>) -> BTreeMap<String, String> {
    let Some(payload) = payload else {
        return BTreeMap::new();
    };

    payload
        .get("headers")
        .and_then(Value::as_array)
        .map(|headers| {
            headers
                .iter()
                .filter_map(|header| {
                    let name = header.get("name").and_then(Value::as_str)?;
                    let value = header.get("value").and_then(Value::as_str)?;
                    Some((name.to_string(), value.to_string()))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default()
}

fn extract_body_from_payload(payload: &Value) -> Option<String> {
    if let Some(data) = payload
        .get("body")
        .and_then(|body| body.get("data"))
        .and_then(Value::as_str)
        .and_then(decode_base64_url)
    {
        return Some(data);
    }

    let parts = payload.get("parts").and_then(Value::as_array)?;
    for part in parts {
        if let Some(value) = extract_body_from_part(part, true) {
            return Some(value);
        }
    }
    for part in parts {
        if let Some(value) = extract_body_from_part(part, false) {
            return Some(value);
        }
    }
    None
}

fn extract_body_from_part(part: &Value, prefer_plain: bool) -> Option<String> {
    let mime = part
        .get("mimeType")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if (!prefer_plain || mime.eq_ignore_ascii_case("text/plain"))
        && let Some(data) = part
            .get("body")
            .and_then(|body| body.get("data"))
            .and_then(Value::as_str)
            .and_then(decode_base64_url)
    {
        return Some(data);
    }

    let parts = part.get("parts").and_then(Value::as_array)?;
    for nested in parts {
        if let Some(value) = extract_body_from_part(nested, prefer_plain) {
            return Some(value);
        }
    }
    None
}

fn decode_base64_url(input: &str) -> Option<String> {
    URL_SAFE_NO_PAD
        .decode(input)
        .or_else(|_| URL_SAFE.decode(input))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn message_matches(message: &GmailMessage, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }

    query
        .split_whitespace()
        .all(|token| match token.split_once(':') {
            Some(("from", value)) => contains_ignore_ascii_case(header(message, "from"), value),
            Some(("subject", value)) => {
                contains_ignore_ascii_case(header(message, "subject"), value)
            }
            Some(("label", value)) => message
                .label_ids
                .iter()
                .any(|label| contains_ignore_ascii_case(label, value)),
            Some(("thread", value)) => contains_ignore_ascii_case(&message.thread_id, value),
            _ => {
                contains_ignore_ascii_case(&message.snippet, token)
                    || contains_ignore_ascii_case(header(message, "subject"), token)
                    || contains_ignore_ascii_case(header(message, "from"), token)
                    || contains_ignore_ascii_case(&message.body, token)
            }
        })
}

fn header<'a>(message: &'a GmailMessage, name: &str) -> &'a str {
    message
        .headers
        .iter()
        .find_map(|(key, value)| {
            if key.eq_ignore_ascii_case(name) {
                Some(value.as_str())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn select_headers(
    headers: &BTreeMap<String, String>,
    selected_headers: &[String],
) -> BTreeMap<String, String> {
    if selected_headers.is_empty() {
        return headers.clone();
    }

    let selected = selected_headers
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();

    headers
        .iter()
        .filter_map(|(key, value)| {
            if selected.contains(&key.to_ascii_lowercase()) {
                Some((key.clone(), value.clone()))
            } else {
                None
            }
        })
        .collect()
}

fn load_fixture_store() -> Result<Option<GmailFixtureStore>, AppError> {
    if let Some(raw) = env::var_os(GOOGLE_CLI_GMAIL_FIXTURE_JSON_ENV) {
        let store = serde_json::from_str(raw.to_string_lossy().as_ref()).map_err(|error| {
            AppError::gmail_failure(format!(
                "failed to parse GOOGLE_CLI_GMAIL_FIXTURE_JSON: {error}"
            ))
        })?;
        return Ok(Some(store));
    }

    if let Some(path) = env::var_os(GOOGLE_CLI_GMAIL_FIXTURE_PATH_ENV) {
        let path = std::path::PathBuf::from(path);
        let text = fs::read_to_string(&path).map_err(|error| {
            AppError::gmail_failure(format!(
                "failed to read Gmail fixture `{}`: {error}",
                path.display()
            ))
        })?;
        let store = serde_json::from_str(&text).map_err(|error| {
            AppError::gmail_failure(format!(
                "failed to parse Gmail fixture `{}`: {error}",
                path.display()
            ))
        })?;
        return Ok(Some(store));
    }

    Ok(None)
}

fn synthetic_message_id(account: &str, raw_rfc822: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    account.hash(&mut hasher);
    raw_rfc822.hash(&mut hasher);
    format!("msg-{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::{GmailMessage, MessageFormat, message_matches, view_for_fixture_message};

    #[test]
    fn query_matches_common_predicates() {
        let message = GmailMessage {
            id: "msg-1".to_string(),
            thread_id: "thread-1".to_string(),
            snippet: "hello inbox".to_string(),
            label_ids: vec!["INBOX".to_string()],
            headers: [
                ("From".to_string(), "team@example.com".to_string()),
                ("Subject".to_string(), "Daily status".to_string()),
            ]
            .into_iter()
            .collect(),
            body: "body".to_string(),
        };

        assert!(message_matches(&message, "from:team@example.com"));
        assert!(message_matches(&message, "subject:status"));
        assert!(message_matches(&message, "label:inbox"));
        assert!(message_matches(&message, "thread:thread-1"));
        assert!(message_matches(&message, "hello"));
    }

    #[test]
    fn metadata_format_respects_header_selection() {
        let message = GmailMessage {
            id: "msg-1".to_string(),
            thread_id: "thread-1".to_string(),
            snippet: "hello".to_string(),
            label_ids: vec![],
            headers: [
                ("From".to_string(), "team@example.com".to_string()),
                ("Subject".to_string(), "Daily status".to_string()),
            ]
            .into_iter()
            .collect(),
            body: "body".to_string(),
        };

        let view = view_for_fixture_message(
            &message,
            MessageFormat::Metadata,
            &["Subject".to_string()],
            false,
        );
        assert_eq!(view.headers.len(), 1);
        assert_eq!(
            view.headers.get("Subject"),
            Some(&"Daily status".to_string())
        );
    }
}
