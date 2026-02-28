use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::auth::account::resolve_account;
use crate::auth::config::{AuthPaths, load_metadata};
use crate::auth::store::load_token;
use crate::cmd::common::GlobalOptions;
use crate::error::AppError;

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
    fixture: GmailFixtureStore,
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
        Ok(Self {
            account: resolved.account,
            account_source: resolved.source.as_str().to_string(),
            access_token: token.access_token,
            fixture,
        })
    }

    pub fn search(&self, request: &SearchRequest) -> Vec<MessageView> {
        let mut results = self
            .fixture
            .messages
            .iter()
            .filter(|message| message_matches(message, request.query.as_str()))
            .map(|message| view_for_message(message, request.format, &request.headers, false))
            .collect::<Vec<_>>();

        results.truncate(request.max);
        results
    }

    pub fn get(&self, request: &GetRequest) -> Result<MessageView, AppError> {
        let message = self
            .fixture
            .messages
            .iter()
            .find(|message| message.id == request.message_id)
            .ok_or_else(|| AppError::gmail_not_found("message", &request.message_id))?;

        Ok(view_for_message(
            message,
            request.format,
            &request.headers,
            true,
        ))
    }

    pub fn thread_get(&self, request: &ThreadGetRequest) -> Result<ThreadView, AppError> {
        let messages = self
            .fixture
            .messages
            .iter()
            .filter(|message| message.thread_id == request.thread_id)
            .collect::<Vec<_>>();

        if messages.is_empty() {
            return Err(AppError::gmail_not_found("thread", &request.thread_id));
        }

        let rendered = messages
            .iter()
            .map(|message| view_for_message(message, request.format, &request.headers, false))
            .collect::<Vec<_>>();

        Ok(ThreadView {
            id: request.thread_id.clone(),
            message_count: rendered.len(),
            messages: rendered,
        })
    }

    pub fn thread_modify(
        &self,
        request: &ThreadModifyRequest,
    ) -> Result<ThreadModifyResult, AppError> {
        let messages = self
            .fixture
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

        Ok(ThreadModifyResult {
            id: request.thread_id.clone(),
            modified_message_count: labels_by_message.len(),
            added_labels: request.add_labels.clone(),
            removed_labels: request.remove_labels.clone(),
            labels_by_message,
        })
    }
}

fn view_for_message(
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

fn load_fixture_store() -> Result<GmailFixtureStore, AppError> {
    if let Some(raw) = env::var_os(GOOGLE_CLI_GMAIL_FIXTURE_JSON_ENV) {
        return serde_json::from_str(raw.to_string_lossy().as_ref()).map_err(|error| {
            AppError::gmail_failure(format!(
                "failed to parse GOOGLE_CLI_GMAIL_FIXTURE_JSON: {error}"
            ))
        });
    }

    if let Some(path) = env::var_os(GOOGLE_CLI_GMAIL_FIXTURE_PATH_ENV) {
        let path = std::path::PathBuf::from(path);
        let text = fs::read_to_string(&path).map_err(|error| {
            AppError::gmail_failure(format!(
                "failed to read Gmail fixture `{}`: {error}",
                path.display()
            ))
        })?;
        return serde_json::from_str(&text).map_err(|error| {
            AppError::gmail_failure(format!(
                "failed to parse Gmail fixture `{}`: {error}",
                path.display()
            ))
        });
    }

    Ok(GmailFixtureStore::default())
}

#[cfg(test)]
mod tests {
    use super::{GmailMessage, MessageFormat, message_matches, view_for_message};

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

        let view = view_for_message(
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
