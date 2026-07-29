use std::env;
use std::path::PathBuf;

use alfred_core::{Feedback, Item, ItemModifier};
use memo::errors::AppError as MemoCliError;
use memo::output::{format_item_id, parse_item_id};
use memo::storage::{Storage, repository, search};
use serde::Serialize;
use thiserror::Error;

pub const DB_INIT_TOKEN: &str = "db-init";
pub const ADD_TOKEN_PREFIX: &str = "add::";
pub const COPY_TOKEN_PREFIX: &str = "copy::";
pub const COPY_JSON_TOKEN_PREFIX: &str = "copy-json::";
pub const UPDATE_TOKEN_PREFIX: &str = "update::";
pub const DELETE_TOKEN_PREFIX: &str = "delete::";
const UPDATE_TOKEN_DELIMITER: &str = "::";
pub const DEFAULT_SOURCE: &str = "alfred";
pub const DEFAULT_MAX_INPUT_BYTES: usize = 4096;
pub const DEFAULT_RECENT_LIMIT: usize = 8;
pub const DEFAULT_SEARCH_MATCH_MODE: SearchMatchMode = SearchMatchMode::Fts;
const MAX_INPUT_BYTES_LIMIT: usize = 1024 * 1024;
const MAX_RECENT_LIMIT: usize = 50;
const MAX_LIST_LIMIT: usize = 200;
const MAX_SEARCH_LIMIT: usize = 200;
const MAX_SEARCH_FETCH_LIMIT: usize = 500;
const SEARCH_INTENT_USAGE: &str = "Use: search <query> (optional: --match fts|prefix|contains)";
const SEARCH_MATCH_USAGE: &str = "Use: search --match <fts|prefix|contains> <query>";
const ACTION_TITLE_MAX_CHARS: usize = 84;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub db_path: PathBuf,
    pub source: String,
    pub require_confirm: bool,
    pub max_input_bytes: usize,
    pub recent_limit: usize,
    pub search_match_mode: SearchMatchMode,
}

impl RuntimeConfig {
    pub fn from_env() -> Result<Self, AppError> {
        let db_path = resolve_db_path();
        let source = resolve_source()?;
        let require_confirm = resolve_require_confirm()?;
        let max_input_bytes = resolve_max_input_bytes()?;
        let recent_limit = resolve_recent_limit()?;
        let search_match_mode = resolve_search_match_mode()?;

        Ok(Self {
            db_path,
            source,
            require_confirm,
            max_input_bytes,
            recent_limit,
            search_match_mode,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMatchMode {
    Fts,
    Prefix,
    Contains,
}

impl SearchMatchMode {
    pub fn parse_token(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "fts" => Some(Self::Fts),
            "prefix" => Some(Self::Prefix),
            "contains" => Some(Self::Contains),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    User(String),
    #[error("{0}")]
    Runtime(String),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::User(_) => 2,
            AppError::Runtime(_) => 1,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            AppError::User(message) | AppError::Runtime(message) => message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddResult {
    pub item_id: String,
    pub created_at: String,
    pub source: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateResult {
    pub item_id: String,
    pub updated_at: String,
    pub text: String,
    pub state: String,
    pub cleared_derivations: i64,
    pub cleared_workflow_anchors: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeleteResult {
    pub item_id: String,
    pub deleted: bool,
    pub deleted_at: String,
    pub removed_derivations: i64,
    pub removed_workflow_anchors: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InitResult {
    pub db_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListResult {
    pub item_id: String,
    pub created_at: String,
    pub state: String,
    pub text_preview: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchResult {
    pub item_id: String,
    pub created_at: String,
    pub score: f64,
    pub matched_fields: Vec<String>,
    pub text_preview: String,
    pub content_type: Option<String>,
    pub validation_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemDetailResult {
    pub item_id: String,
    pub created_at: String,
    pub source: String,
    pub text: String,
    pub state: String,
    pub content_type: Option<String>,
    pub validation_status: Option<String>,
}

pub fn build_script_filter(query: &str, config: &RuntimeConfig) -> Result<Feedback, AppError> {
    let normalized = query.trim();
    if normalized.is_empty() {
        return build_empty_query_feedback(config);
    }

    if let Some(rest) = strip_intent(normalized, "item") {
        return build_item_action_feedback(rest, config);
    }

    if let Some(rest) = strip_intent(normalized, "update") {
        return build_update_feedback(rest, config);
    }

    if let Some(rest) = strip_intent(normalized, "delete") {
        return build_delete_feedback(rest, config);
    }

    if let Some(rest) = strip_intent(normalized, "copy") {
        return build_copy_feedback(rest, config);
    }

    if let Some(rest) = strip_intent(normalized, "search") {
        return build_search_feedback(rest, config);
    }

    if normalized.len() > config.max_input_bytes {
        return Ok(Feedback::new(vec![
            Item::new("Input exceeds MEMO_MAX_INPUT_BYTES")
                .with_subtitle(format!(
                    "Current {} bytes, limit {} bytes.",
                    normalized.len(),
                    config.max_input_bytes
                ))
                .with_valid(false),
        ]));
    }

    let preview = truncate_title(normalized, 64);
    let add_token = build_add_token(normalized);

    if config.require_confirm {
        return Ok(Feedback::new(vec![
            Item::new(format!("Preview: {preview}"))
                .with_subtitle("Confirmation required. Choose the row below to save.")
                .with_valid(false),
            Item::new("Confirm add memo")
                .with_subtitle(format!(
                    "Source: {} | DB: {}",
                    config.source,
                    config.db_path.display()
                ))
                .with_arg(add_token)
                .with_valid(true),
        ]));
    }

    Ok(Feedback::new(vec![
        Item::new(format!("Add memo: {preview}"))
            .with_subtitle(format!(
                "Press Enter to save ({}/{} bytes).",
                normalized.len(),
                config.max_input_bytes
            ))
            .with_arg(add_token)
            .with_valid(true),
    ]))
}

pub fn execute_db_init(
    db_override: Option<PathBuf>,
    config: &RuntimeConfig,
) -> Result<InitResult, AppError> {
    let db_path = db_override.unwrap_or_else(|| config.db_path.clone());
    let storage = Storage::new(db_path.clone());
    storage
        .init()
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    Ok(InitResult {
        db_path: db_path.display().to_string(),
    })
}

pub fn execute_add(
    text: &str,
    source_override: Option<&str>,
    db_override: Option<PathBuf>,
    config: &RuntimeConfig,
) -> Result<AddResult, AppError> {
    let normalized_text = text.trim();
    if normalized_text.is_empty() {
        return Err(AppError::User(
            "add requires a non-empty memo text".to_string(),
        ));
    }

    if normalized_text.len() > config.max_input_bytes {
        return Err(AppError::User(format!(
            "memo text exceeds MEMO_MAX_INPUT_BYTES: {} > {}",
            normalized_text.len(),
            config.max_input_bytes
        )));
    }

    let source = source_override
        .map(str::trim)
        .unwrap_or(config.source.as_str())
        .to_string();
    if source.is_empty() {
        return Err(AppError::User("MEMO_SOURCE must be non-empty".to_string()));
    }

    let db_path = db_override.unwrap_or_else(|| config.db_path.clone());
    let storage = Storage::new(db_path);
    storage
        .init()
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    let added = storage
        .with_transaction(|tx| repository::add_item(tx, normalized_text, &source, None))
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    Ok(AddResult {
        item_id: format_item_id(added.item_id),
        created_at: added.created_at,
        source: added.source,
        text: added.text,
    })
}

pub fn execute_update(
    item_id_raw: &str,
    text: &str,
    db_override: Option<PathBuf>,
    config: &RuntimeConfig,
) -> Result<UpdateResult, AppError> {
    let item_id = parse_item_id(item_id_raw)
        .ok_or_else(|| AppError::User("update requires a valid item_id".to_string()))?;
    let normalized_text = text.trim();
    if normalized_text.is_empty() {
        return Err(AppError::User(
            "update requires a non-empty text argument".to_string(),
        ));
    }

    if normalized_text.len() > config.max_input_bytes {
        return Err(AppError::User(format!(
            "memo text exceeds MEMO_MAX_INPUT_BYTES: {} > {}",
            normalized_text.len(),
            config.max_input_bytes
        )));
    }

    let db_path = db_override.unwrap_or_else(|| config.db_path.clone());
    let storage = Storage::new(db_path);
    storage
        .init()
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    let updated = storage
        .with_transaction(|tx| repository::update_item(tx, item_id, normalized_text))
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    Ok(UpdateResult {
        item_id: format_item_id(updated.item_id),
        updated_at: updated.updated_at,
        text: updated.text,
        state: "pending".to_string(),
        cleared_derivations: updated.cleared_derivations,
        cleared_workflow_anchors: updated.cleared_workflow_anchors,
    })
}

pub fn execute_delete(
    item_id_raw: &str,
    db_override: Option<PathBuf>,
    config: &RuntimeConfig,
) -> Result<DeleteResult, AppError> {
    let item_id = parse_item_id(item_id_raw)
        .ok_or_else(|| AppError::User("delete requires a valid item_id".to_string()))?;
    let db_path = db_override.unwrap_or_else(|| config.db_path.clone());
    let storage = Storage::new(db_path);
    storage
        .init()
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    let deleted = storage
        .with_transaction(|tx| repository::delete_item_hard(tx, item_id))
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    Ok(DeleteResult {
        item_id: format_item_id(deleted.item_id),
        deleted: true,
        deleted_at: deleted.deleted_at,
        removed_derivations: deleted.removed_derivations,
        removed_workflow_anchors: deleted.removed_workflow_anchors,
    })
}

pub fn execute_list(
    db_override: Option<PathBuf>,
    limit: usize,
    offset: usize,
    config: &RuntimeConfig,
) -> Result<Vec<ListResult>, AppError> {
    if !(1..=MAX_LIST_LIMIT).contains(&limit) {
        return Err(AppError::User(format!(
            "invalid list limit: {limit} (must be integer in range 1..={MAX_LIST_LIMIT})"
        )));
    }

    let db_path = db_override.unwrap_or_else(|| config.db_path.clone());
    let storage = Storage::new(db_path);
    storage
        .init()
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    let rows = storage
        .with_connection(|conn| {
            repository::list_items(conn, repository::QueryState::All, limit, offset)
        })
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| ListResult {
            item_id: format_item_id(row.item_id),
            created_at: row.created_at,
            state: row.state,
            text_preview: row.text_preview,
        })
        .collect())
}

pub fn execute_search(
    db_override: Option<PathBuf>,
    query: &str,
    match_mode: SearchMatchMode,
    limit: usize,
    offset: usize,
    config: &RuntimeConfig,
) -> Result<Vec<SearchResult>, AppError> {
    let normalized_query = query.trim();
    if normalized_query.is_empty() {
        return Err(AppError::User(
            "search requires a non-empty query".to_string(),
        ));
    }

    if !(1..=MAX_SEARCH_LIMIT).contains(&limit) {
        return Err(AppError::User(format!(
            "invalid search limit: {limit} (must be integer in range 1..={MAX_SEARCH_LIMIT})"
        )));
    }

    let fetch_limit = limit.checked_add(offset).ok_or_else(|| {
        AppError::User("invalid search window: limit + offset overflow".to_string())
    })?;
    if fetch_limit > MAX_SEARCH_FETCH_LIMIT {
        return Err(AppError::User(format!(
            "invalid search window: limit + offset must be <= {MAX_SEARCH_FETCH_LIMIT}"
        )));
    }

    let db_path = db_override.unwrap_or_else(|| config.db_path.clone());
    let storage = Storage::new(db_path);
    storage
        .init()
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    let search_fields = [
        search::SearchField::Raw,
        search::SearchField::Derived,
        search::SearchField::Tags,
    ];
    let upstream_match_mode = match match_mode {
        SearchMatchMode::Fts => search::SearchMatchMode::Fts,
        SearchMatchMode::Prefix => search::SearchMatchMode::Prefix,
        SearchMatchMode::Contains => search::SearchMatchMode::Contains,
    };

    let rows = storage
        .with_connection(|conn| {
            search::search_items(
                conn,
                normalized_query,
                repository::QueryState::All,
                &search_fields,
                upstream_match_mode,
                fetch_limit,
            )
        })
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    Ok(rows
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|row| SearchResult {
            item_id: format_item_id(row.item_id),
            created_at: row.created_at,
            score: row.score,
            matched_fields: row.matched_fields,
            text_preview: row.preview,
            content_type: row.content_type,
            validation_status: row.validation_status,
        })
        .collect())
}

pub fn execute_fetch_item(
    item_id_raw: &str,
    db_override: Option<PathBuf>,
    config: &RuntimeConfig,
) -> Result<ItemDetailResult, AppError> {
    let item_id = parse_item_id(item_id_raw)
        .ok_or_else(|| AppError::User("copy requires a valid item_id".to_string()))?;
    let db_path = db_override.unwrap_or_else(|| config.db_path.clone());
    let storage = Storage::new(db_path);
    storage
        .init()
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;

    let cursor = storage
        .with_connection(|conn| repository::lookup_fetch_cursor(conn, item_id))
        .map_err(|error| AppError::Runtime(error.message().to_string()))?;
    if cursor.is_none() {
        return Err(AppError::User("item_id does not exist".to_string()));
    }

    storage
        .with_connection(|conn| {
            conn.query_row(
                "select
                    i.item_id,
                    i.created_at,
                    i.source,
                    i.raw_text,
                    case
                        when ad.derivation_id is not null then 'enriched'
                        else 'pending'
                    end as state,
                    json_extract(ad.payload_json, '$.content_type') as content_type,
                    json_extract(ad.payload_json, '$.validation_status') as validation_status
                from inbox_items i
                left join item_derivations ad
                  on ad.derivation_id = (
                    select d.derivation_id
                    from item_derivations d
                    where d.item_id = i.item_id
                      and d.is_active = 1
                      and d.status = 'accepted'
                    order by d.derivation_version desc, d.derivation_id desc
                    limit 1
                  )
                where i.item_id = ?1",
                [item_id],
                |row| {
                    Ok(ItemDetailResult {
                        item_id: format_item_id(row.get::<_, i64>(0)?),
                        created_at: row.get(1)?,
                        source: row.get(2)?,
                        text: row.get(3)?,
                        state: row.get(4)?,
                        content_type: row.get(5)?,
                        validation_status: row.get(6)?,
                    })
                },
            )
            .map_err(MemoCliError::db_query)
        })
        .map_err(|error| AppError::Runtime(error.message().to_string()))
}

pub fn parse_add_token(arg: &str) -> Option<String> {
    arg.strip_prefix(ADD_TOKEN_PREFIX).map(str::to_string)
}

pub fn build_add_token(text: &str) -> String {
    format!("{ADD_TOKEN_PREFIX}{text}")
}

pub fn parse_copy_token(arg: &str) -> Option<String> {
    let payload = arg.strip_prefix(COPY_TOKEN_PREFIX)?;
    let item_id = parse_item_id(payload.trim())?;
    Some(format_item_id(item_id))
}

pub fn build_copy_token(item_id: &str) -> String {
    format!("{COPY_TOKEN_PREFIX}{item_id}")
}

pub fn parse_copy_json_token(arg: &str) -> Option<String> {
    let payload = arg.strip_prefix(COPY_JSON_TOKEN_PREFIX)?;
    let item_id = parse_item_id(payload.trim())?;
    Some(format_item_id(item_id))
}

pub fn build_copy_json_token(item_id: &str) -> String {
    format!("{COPY_JSON_TOKEN_PREFIX}{item_id}")
}

pub fn parse_update_token(arg: &str) -> Option<(String, String)> {
    let payload = arg.strip_prefix(UPDATE_TOKEN_PREFIX)?;
    let (item_id_raw, text_raw) = payload.split_once(UPDATE_TOKEN_DELIMITER)?;
    let item_id = parse_item_id(item_id_raw)?;
    let text = text_raw.trim();
    if text.is_empty() {
        return None;
    }

    Some((format_item_id(item_id), text.to_string()))
}

pub fn build_update_token(item_id: &str, text: &str) -> String {
    format!("{UPDATE_TOKEN_PREFIX}{item_id}{UPDATE_TOKEN_DELIMITER}{text}")
}

pub fn parse_delete_token(arg: &str) -> Option<String> {
    let payload = arg.strip_prefix(DELETE_TOKEN_PREFIX)?;
    let item_id = parse_item_id(payload.trim())?;
    Some(format_item_id(item_id))
}

pub fn build_delete_token(item_id: &str) -> String {
    format!("{DELETE_TOKEN_PREFIX}{item_id}")
}

fn strip_intent<'a>(query: &'a str, intent: &str) -> Option<&'a str> {
    let mut parts = query.splitn(2, char::is_whitespace);
    let first = parts.next()?;
    if !first.eq_ignore_ascii_case(intent) {
        return None;
    }
    Some(parts.next().unwrap_or("").trim())
}

fn item_route_id(item_id: &str) -> String {
    parse_item_id(item_id)
        .map(|id| id.to_string())
        .unwrap_or_else(|| item_id.to_string())
}

fn item_display_id(item_id: &str) -> String {
    parse_item_id(item_id)
        .map(|id| format!("#{id}"))
        .unwrap_or_else(|| item_id.to_string())
}

fn build_item_action_feedback(rest: &str, config: &RuntimeConfig) -> Result<Feedback, AppError> {
    let mut parts = rest.split_whitespace();
    let item_id_raw = parts.next().unwrap_or("").trim();
    if item_id_raw.is_empty() || parts.next().is_some() {
        return Ok(Feedback::new(vec![
            Item::new("Invalid item selection syntax")
                .with_subtitle("Use: item <item_id>")
                .with_valid(false),
        ]));
    }

    let item_id = match parse_item_id(item_id_raw) {
        Some(item_id) => format_item_id(item_id),
        None => {
            return Ok(Feedback::new(vec![
                Item::new("Invalid item_id for selection")
                    .with_subtitle("Expected itm_XXXXXXXX or positive integer item id.")
                    .with_valid(false),
            ]));
        }
    };
    let item_route = item_route_id(&item_id);
    let item_display = item_display_id(&item_id);

    let detail = match execute_fetch_item(&item_id, None, config) {
        Ok(detail) => detail,
        Err(AppError::User(message)) => {
            return Ok(Feedback::new(vec![
                Item::new("Memo item not found")
                    .with_subtitle(format!("{message}: {item_display}"))
                    .with_valid(false),
            ]));
        }
        Err(error) => return Err(error),
    };
    let raw_json_preview = render_item_detail_json(&detail);
    let copy_item = build_copy_item(
        &item_id,
        &item_display,
        Some(&detail.text),
        &raw_json_preview,
    );
    let update_item = build_update_item(
        &item_id,
        &item_display,
        &item_route,
        Some(&detail.text),
        None,
        false,
    );
    let delete_item = build_delete_item(&item_id, &item_display, Some(&detail.text));

    Ok(Feedback::new(vec![copy_item, update_item, delete_item]))
}

fn build_update_feedback(rest: &str, config: &RuntimeConfig) -> Result<Feedback, AppError> {
    let mut parts = rest.splitn(2, char::is_whitespace);
    let item_id_raw = parts.next().unwrap_or("").trim();
    let text = parts.next().unwrap_or("").trim();

    if item_id_raw.is_empty() {
        return Ok(Feedback::new(vec![
            Item::new("Invalid update syntax")
                .with_subtitle("Use: update <item_id> <new text>")
                .with_valid(false),
        ]));
    }

    let item_id = match parse_item_id(item_id_raw) {
        Some(item_id) => format_item_id(item_id),
        None => {
            return Ok(Feedback::new(vec![
                Item::new("Invalid item_id for update")
                    .with_subtitle("Expected itm_XXXXXXXX or positive integer item id.")
                    .with_valid(false),
            ]));
        }
    };
    let item_route = item_route_id(&item_id);
    let item_display = item_display_id(&item_id);
    let detail_text = execute_fetch_item(&item_id, None, config)
        .ok()
        .map(|detail| detail.text);

    if text.is_empty() {
        let update_item = build_update_item(
            &item_id,
            &item_display,
            &item_route,
            detail_text.as_deref(),
            None,
            false,
        );
        return Ok(Feedback::new(vec![update_item]));
    }

    if text.len() > config.max_input_bytes {
        return Ok(Feedback::new(vec![
            Item::new("Input exceeds MEMO_MAX_INPUT_BYTES")
                .with_subtitle(format!(
                    "Current {} bytes, limit {} bytes.",
                    text.len(),
                    config.max_input_bytes
                ))
                .with_valid(false),
        ]));
    }

    let update_item = build_update_item(
        &item_id,
        &item_display,
        &item_route,
        detail_text.as_deref(),
        Some(text),
        true,
    );

    Ok(Feedback::new(vec![update_item]))
}

fn build_delete_feedback(rest: &str, config: &RuntimeConfig) -> Result<Feedback, AppError> {
    let mut parts = rest.split_whitespace();
    let item_id_raw = parts.next().unwrap_or("").trim();
    if item_id_raw.is_empty() || parts.next().is_some() {
        return Ok(Feedback::new(vec![
            Item::new("Invalid delete syntax")
                .with_subtitle("Use: delete <item_id>")
                .with_valid(false),
        ]));
    }

    let item_id = match parse_item_id(item_id_raw) {
        Some(item_id) => format_item_id(item_id),
        None => {
            return Ok(Feedback::new(vec![
                Item::new("Invalid item_id for delete")
                    .with_subtitle("Expected itm_XXXXXXXX or positive integer item id.")
                    .with_valid(false),
            ]));
        }
    };
    let item_display = item_display_id(&item_id);
    let detail_text = execute_fetch_item(&item_id, None, config)
        .ok()
        .map(|detail| detail.text);
    let delete_item = build_delete_item(&item_id, &item_display, detail_text.as_deref());

    Ok(Feedback::new(vec![delete_item]))
}

fn build_copy_feedback(rest: &str, config: &RuntimeConfig) -> Result<Feedback, AppError> {
    let mut parts = rest.split_whitespace();
    let item_id_raw = parts.next().unwrap_or("").trim();
    if item_id_raw.is_empty() || parts.next().is_some() {
        return Ok(Feedback::new(vec![
            Item::new("Invalid copy syntax")
                .with_subtitle("Use: copy <item_id>")
                .with_valid(false),
        ]));
    }

    let item_id = match parse_item_id(item_id_raw) {
        Some(item_id) => format_item_id(item_id),
        None => {
            return Ok(Feedback::new(vec![
                Item::new("Invalid item_id for copy")
                    .with_subtitle("Expected itm_XXXXXXXX or positive integer item id.")
                    .with_valid(false),
            ]));
        }
    };
    let item_display = item_display_id(&item_id);

    let detail = match execute_fetch_item(&item_id, None, config) {
        Ok(detail) => detail,
        Err(AppError::User(message)) => {
            return Ok(Feedback::new(vec![
                Item::new("Memo item not found")
                    .with_subtitle(format!("{message}: {item_display}"))
                    .with_valid(false),
            ]));
        }
        Err(error) => return Err(error),
    };
    let raw_json_preview = render_item_detail_json(&detail);
    let copy_item = build_copy_item(
        &item_id,
        &item_display,
        Some(&detail.text),
        &raw_json_preview,
    );

    Ok(Feedback::new(vec![copy_item]))
}

fn build_search_feedback(rest: &str, config: &RuntimeConfig) -> Result<Feedback, AppError> {
    let (match_mode, query) = match parse_search_intent(rest, config.search_match_mode) {
        Ok(parsed) => parsed,
        Err(feedback) => return Ok(feedback),
    };

    if query.is_empty() {
        return Ok(Feedback::new(vec![
            Item::new("Type search text after keyword")
                .with_subtitle(SEARCH_INTENT_USAGE)
                .with_valid(false),
        ]));
    }

    if query.len() > config.max_input_bytes {
        return Ok(Feedback::new(vec![
            Item::new("Input exceeds MEMO_MAX_INPUT_BYTES")
                .with_subtitle(format!(
                    "Current {} bytes, limit {} bytes.",
                    query.len(),
                    config.max_input_bytes
                ))
                .with_valid(false),
        ]));
    }

    let rows = execute_search(None, query, match_mode, config.recent_limit, 0, config)?;
    if rows.is_empty() {
        return Ok(Feedback::new(vec![
            Item::new("No matching memo records")
                .with_subtitle(format!("No results for: {}", truncate_title(query, 64)))
                .with_valid(false),
        ]));
    }

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let item_display = item_display_id(&row.item_id);
        let item_route = item_route_id(&row.item_id);
        let preview = row.text_preview.trim();
        let title = if preview.is_empty() {
            format!("Search {}: (empty memo)", item_display)
        } else {
            format!("Search {}: {}", item_display, truncate_title(preview, 56))
        };

        let matched_fields = if row.matched_fields.is_empty() {
            "n/a".to_string()
        } else {
            row.matched_fields.join(",")
        };

        items.push(
            Item::new(title)
                .with_uid(format!("search-{}", row.item_id))
                .with_subtitle(format!(
                    "{} | fields {} | score {:.3} | Press Enter to manage",
                    row.created_at, matched_fields, row.score
                ))
                .with_autocomplete(format!("item {}", item_route))
                .with_valid(false),
        );
    }

    Ok(Feedback::new(items).with_skip_knowledge(true))
}

fn parse_search_intent(
    rest: &str,
    default_match_mode: SearchMatchMode,
) -> Result<(SearchMatchMode, &str), Feedback> {
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return Err(Feedback::new(vec![
            Item::new("Type search text after keyword")
                .with_subtitle("Use: search <query>")
                .with_valid(false),
        ]));
    }

    if let Some(after_flag) = trimmed.strip_prefix("--match") {
        let is_match_flag = after_flag
            .chars()
            .next()
            .map(|ch| ch.is_whitespace() || ch == '=')
            .unwrap_or(true);
        if is_match_flag {
            let mut rest = after_flag.trim_start();
            if let Some(remaining) = rest.strip_prefix('=') {
                rest = remaining.trim_start();
            }
            if rest.is_empty() {
                return Err(Feedback::new(vec![
                    Item::new("Missing search match mode")
                        .with_subtitle(SEARCH_MATCH_USAGE)
                        .with_valid(false),
                ]));
            }

            let mut parts = rest.splitn(2, char::is_whitespace);
            let mode_raw = parts.next().unwrap_or("").trim();
            let Some(match_mode) = SearchMatchMode::parse_token(mode_raw) else {
                return Err(Feedback::new(vec![
                    Item::new(format!(
                        "Invalid search match mode: {}",
                        truncate_title(mode_raw, 32)
                    ))
                    .with_subtitle(SEARCH_MATCH_USAGE)
                    .with_valid(false),
                ]));
            };

            let query = parts.next().unwrap_or("").trim();
            if query.is_empty() {
                return Err(Feedback::new(vec![
                    Item::new("Type search text after keyword")
                        .with_subtitle(SEARCH_MATCH_USAGE)
                        .with_valid(false),
                ]));
            }
            return Ok((match_mode, query));
        }
    }

    Ok((default_match_mode, trimmed))
}

fn build_empty_query_feedback(config: &RuntimeConfig) -> Result<Feedback, AppError> {
    let db_exists = config.db_path.exists();

    let mut items = vec![
        Item::new("Type memo text after keyword")
            .with_subtitle(format!(
                "Max {} bytes. Current source: {}.",
                config.max_input_bytes, config.source
            ))
            .with_valid(false),
    ];

    if !db_exists {
        items.push(
            Item::new("Initialize memo database")
                .with_subtitle(format!(
                    "Create/open SQLite at {}",
                    config.db_path.display()
                ))
                .with_arg(DB_INIT_TOKEN)
                .with_valid(true),
        );
        items.push(
            Item::new("No memo records yet")
                .with_subtitle("Run `db-init`, then use `mm <text>` to add your first memo.")
                .with_valid(false),
        );
        return Ok(Feedback::new(items));
    }

    items.push(
        Item::new("Memo database path")
            .with_subtitle(format!("Using SQLite at {}", config.db_path.display()))
            .with_valid(false),
    );

    let recent = execute_list(None, config.recent_limit, 0, config)?;
    if recent.is_empty() {
        items.push(
            Item::new("No memo records yet")
                .with_subtitle("Use `mm <text>` then press Enter to add your first memo.")
                .with_valid(false),
        );
        return Ok(Feedback::new(items));
    }

    for row in recent {
        let item_display = item_display_id(&row.item_id);
        let item_route = item_route_id(&row.item_id);
        let preview = row.text_preview.trim();
        let title = if preview.is_empty() {
            format!("Recent {}: (empty memo)", item_display)
        } else {
            format!("Recent {}: {}", item_display, truncate_title(preview, 56))
        };

        items.push(
            Item::new(title)
                .with_uid(format!("recent-{}", row.item_id))
                .with_subtitle(format!(
                    "{} | {} | Press Enter to manage",
                    row.created_at, row.state
                ))
                .with_autocomplete(format!("item {}", item_route))
                .with_valid(false),
        );
    }

    Ok(Feedback::new(items).with_skip_knowledge(true))
}

fn render_item_detail_json(detail: &ItemDetailResult) -> String {
    serde_json::to_string(detail).unwrap_or_else(|_| "{}".to_string())
}

fn normalize_text_preview(text: &str) -> String {
    let normalized = text
        .chars()
        .map(|ch| {
            if matches!(ch, '\n' | '\r' | '\t') {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>();
    let collapsed = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "(empty memo)".to_string()
    } else {
        collapsed
    }
}

fn split_action_title(prefix: &str, text: &str) -> (String, Option<String>) {
    let preview = normalize_text_preview(text);
    let prefix_len = prefix.chars().count();
    if prefix_len >= ACTION_TITLE_MAX_CHARS {
        return (
            truncate_title(&format!("{prefix}{preview}"), ACTION_TITLE_MAX_CHARS),
            None,
        );
    }

    let available = ACTION_TITLE_MAX_CHARS - prefix_len;
    let preview_len = preview.chars().count();
    if preview_len <= available {
        return (format!("{prefix}{preview}"), None);
    }

    let title_preview = preview.chars().take(available).collect::<String>();
    let overflow = preview.chars().skip(available).collect::<String>();
    (format!("{prefix}{title_preview}…"), Some(overflow))
}

fn build_delete_item(item_id: &str, item_display: &str, text: Option<&str>) -> Item {
    let prefix = format!("Delete memo: {item_display} | ");
    let (title, subtitle) = match text {
        Some(value) => split_action_title(&prefix, value),
        None => (format!("Delete memo: {item_display}"), None),
    };

    let mut item = Item::new(title)
        .with_arg(build_delete_token(item_id))
        .with_valid(true);
    if let Some(value) = subtitle {
        item = item.with_subtitle(value);
    }

    item
}

fn build_copy_item(item_id: &str, item_display: &str, text: Option<&str>, raw_json: &str) -> Item {
    let prefix = format!("Copy memo: {item_display} | ");
    let (title, subtitle) = match text {
        Some(value) => split_action_title(&prefix, value),
        None => (format!("Copy memo: {item_display}"), None),
    };

    let mut item = Item::new(title)
        .with_arg(build_copy_token(item_id))
        .with_mod(
            "cmd",
            ItemModifier::new()
                .with_subtitle(format!("Preview JSON: {}", truncate_title(raw_json, 72)))
                .with_arg(build_copy_json_token(item_id))
                .with_valid(true),
        )
        .with_valid(true);
    if let Some(value) = subtitle {
        item = item.with_subtitle(value);
    }

    item
}

fn build_update_item(
    item_id: &str,
    item_display: &str,
    item_route: &str,
    existing_text: Option<&str>,
    update_text: Option<&str>,
    valid: bool,
) -> Item {
    let preview_text = existing_text.or(update_text);
    let prefix = format!("Update memo: {item_display} | ");
    let (title, subtitle) = match preview_text {
        Some(value) => split_action_title(&prefix, value),
        None => (format!("Update memo: {item_display}"), None),
    };

    let mut item = Item::new(title);
    if valid {
        if let Some(text) = update_text {
            item = item.with_arg(build_update_token(item_id, text));
        }
    } else {
        item = item.with_autocomplete(format!("update {item_route} "));
    }
    if let Some(value) = subtitle {
        item = item.with_subtitle(value);
    }

    item.with_valid(valid)
}

fn truncate_title(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }

    let mut value = input.chars().take(max_chars).collect::<String>();
    value.push('…');
    value
}

fn resolve_db_path() -> PathBuf {
    let home = env::var("HOME").ok();

    if let Some(path) = non_empty_env("MEMO_DB_PATH") {
        return PathBuf::from(expand_home_path(&path, home.as_deref()));
    }

    for env_key in ["ALFRED_WORKFLOW_DATA", "ALFRED_WORKFLOW_CACHE"] {
        if let Some(path) = non_empty_env(env_key) {
            let expanded = expand_home_path(&path, home.as_deref());
            return PathBuf::from(expanded).join("memo.db");
        }
    }

    default_memo_db_path()
}

fn default_memo_db_path() -> PathBuf {
    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(data_home).join("nils-cli").join("memo.db");
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("nils-cli")
            .join("memo.db");
    }

    PathBuf::from("memo.db")
}

fn resolve_source() -> Result<String, AppError> {
    let source = non_empty_env("MEMO_SOURCE").unwrap_or_else(|| DEFAULT_SOURCE.to_string());
    let source = source.trim();

    if source.is_empty() {
        return Err(AppError::User("MEMO_SOURCE must be non-empty".to_string()));
    }

    Ok(source.to_string())
}

fn resolve_require_confirm() -> Result<bool, AppError> {
    let raw = non_empty_env("MEMO_REQUIRE_CONFIRM").unwrap_or_else(|| "0".to_string());
    parse_bool(&raw).ok_or_else(|| {
        AppError::User(
            "invalid MEMO_REQUIRE_CONFIRM: expected one of 1/0/true/false/yes/no/on/off"
                .to_string(),
        )
    })
}

fn resolve_max_input_bytes() -> Result<usize, AppError> {
    let raw = non_empty_env("MEMO_MAX_INPUT_BYTES")
        .unwrap_or_else(|| DEFAULT_MAX_INPUT_BYTES.to_string());

    let parsed = raw.parse::<usize>().map_err(|_| {
        AppError::User(format!(
            "invalid MEMO_MAX_INPUT_BYTES: {raw} (must be integer in range 1..={MAX_INPUT_BYTES_LIMIT})"
        ))
    })?;

    if !(1..=MAX_INPUT_BYTES_LIMIT).contains(&parsed) {
        return Err(AppError::User(format!(
            "invalid MEMO_MAX_INPUT_BYTES: {parsed} (must be integer in range 1..={MAX_INPUT_BYTES_LIMIT})"
        )));
    }

    Ok(parsed)
}

fn resolve_recent_limit() -> Result<usize, AppError> {
    let raw =
        non_empty_env("MEMO_RECENT_LIMIT").unwrap_or_else(|| DEFAULT_RECENT_LIMIT.to_string());

    let parsed = raw.parse::<usize>().map_err(|_| {
        AppError::User(format!(
            "invalid MEMO_RECENT_LIMIT: {raw} (must be integer in range 1..={MAX_RECENT_LIMIT})"
        ))
    })?;

    if !(1..=MAX_RECENT_LIMIT).contains(&parsed) {
        return Err(AppError::User(format!(
            "invalid MEMO_RECENT_LIMIT: {parsed} (must be integer in range 1..={MAX_RECENT_LIMIT})"
        )));
    }

    Ok(parsed)
}

fn resolve_search_match_mode() -> Result<SearchMatchMode, AppError> {
    let raw = non_empty_env("MEMO_SEARCH_MATCH").unwrap_or_else(|| "fts".to_string());
    SearchMatchMode::parse_token(&raw).ok_or_else(|| {
        AppError::User(format!(
            "invalid MEMO_SEARCH_MATCH: {raw} (must be one of fts/prefix/contains)"
        ))
    })
}

fn non_empty_env(key: &str) -> Option<String> {
    let value = env::var(key).ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

fn expand_home_path(raw: &str, home: Option<&str>) -> String {
    let trimmed = raw.trim();
    let Some(home) = home.map(str::trim).filter(|value| !value.is_empty()) else {
        return trimmed.to_string();
    };

    let home = home.trim_end_matches('/');
    let mut expanded = trimmed.replace("$HOME", home);

    if expanded == "~" {
        expanded = home.to_string();
    } else if let Some(rest) = expanded.strip_prefix("~/") {
        expanded = format!("{home}/{rest}");
    }

    expanded
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_config() -> RuntimeConfig {
        RuntimeConfig {
            db_path: PathBuf::from("/tmp/memo-test.db"),
            source: "alfred".to_string(),
            require_confirm: false,
            max_input_bytes: 4096,
            recent_limit: DEFAULT_RECENT_LIMIT,
            search_match_mode: DEFAULT_SEARCH_MATCH_MODE,
        }
    }

    #[test]
    fn add_token_roundtrip() {
        let token = build_add_token("buy milk");
        assert_eq!(parse_add_token(&token).as_deref(), Some("buy milk"));
    }

    #[test]
    fn copy_token_roundtrip() {
        let token = build_copy_token("itm_00000042");
        let parsed = parse_copy_token(&token).expect("copy token should parse");
        assert_eq!(parsed, "itm_00000042");
    }

    #[test]
    fn copy_json_token_roundtrip() {
        let token = build_copy_json_token("itm_00000042");
        let parsed = parse_copy_json_token(&token).expect("copy json token should parse");
        assert_eq!(parsed, "itm_00000042");
    }

    #[test]
    fn update_token_roundtrip() {
        let token = build_update_token("itm_00000042", "buy milk::after work");
        let parsed = parse_update_token(&token).expect("update token should parse");
        assert_eq!(parsed.0, "itm_00000042");
        assert_eq!(parsed.1, "buy milk::after work");
    }

    #[test]
    fn delete_token_roundtrip() {
        let token = build_delete_token("itm_00000042");
        let parsed = parse_delete_token(&token).expect("delete token should parse");
        assert_eq!(parsed, "itm_00000042");
    }

    #[test]
    fn update_token_rejects_missing_text() {
        let token = format!("{UPDATE_TOKEN_PREFIX}itm_00000042{UPDATE_TOKEN_DELIMITER}");
        assert!(parse_update_token(&token).is_none());
    }

    #[test]
    fn script_filter_returns_db_init_on_empty_query() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("missing.db");

        let feedback = build_script_filter("", &config).expect("script filter should build");

        assert!(feedback.items.len() >= 2);
        let db_init_item = feedback
            .items
            .iter()
            .find(|item| item.arg.as_deref() == Some(DB_INIT_TOKEN))
            .expect("db init row should exist");
        assert_eq!(db_init_item.valid, Some(true));
    }

    #[test]
    fn script_filter_existing_db_shows_db_path_info_without_db_init() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        std::fs::File::create(&config.db_path).expect("create db file");

        let feedback = build_script_filter("", &config).expect("script filter should build");

        let has_db_init = feedback
            .items
            .iter()
            .any(|item| item.arg.as_deref() == Some(DB_INIT_TOKEN));
        assert!(
            !has_db_init,
            "existing db should not show db-init action row"
        );

        let expected_subtitle = format!("Using SQLite at {}", config.db_path.display());
        let has_db_path_info = feedback.items.iter().any(|item| {
            item.title == "Memo database path"
                && item.subtitle.as_deref() == Some(expected_subtitle.as_str())
        });
        assert!(has_db_path_info, "existing db should show db path info row");
    }

    #[test]
    fn script_filter_returns_add_action_for_non_empty_query() {
        let feedback =
            build_script_filter("buy milk", &test_config()).expect("script filter should build");

        assert_eq!(feedback.items.len(), 1);
        assert!(
            feedback.items[0]
                .arg
                .as_deref()
                .expect("arg")
                .starts_with(ADD_TOKEN_PREFIX)
        );
        assert_eq!(feedback.items[0].valid, Some(true));
    }

    #[test]
    fn script_filter_returns_update_action_for_update_intent() {
        let feedback = build_script_filter("update itm_00000002 buy almond milk", &test_config())
            .expect("script filter should build");

        assert_eq!(feedback.items.len(), 1);
        let arg = feedback.items[0].arg.as_deref().expect("arg should exist");
        assert!(arg.starts_with(UPDATE_TOKEN_PREFIX));
        assert_eq!(feedback.items[0].valid, Some(true));
    }

    #[test]
    fn script_filter_guides_update_without_text() {
        let feedback =
            build_script_filter("update itm_00000002", &test_config()).expect("script filter");

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].valid, Some(false));
        assert_eq!(feedback.items[0].title, "Update memo: #2");
        assert_eq!(feedback.items[0].autocomplete.as_deref(), Some("update 2 "));
    }

    #[test]
    fn update_row_overflow_moves_remaining_text_to_subtitle() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        let long_text =
            "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor";
        let add = execute_add(long_text, None, None, &config).expect("seed add");
        let query = format!("update {}", add.item_id);

        let feedback = build_script_filter(&query, &config).expect("script filter");

        assert_eq!(feedback.items.len(), 1);
        assert!(
            feedback.items[0].title.ends_with('…'),
            "overflow title should end with ellipsis"
        );
        assert!(
            feedback.items[0]
                .subtitle
                .as_deref()
                .map(|value| !value.is_empty())
                .unwrap_or(false),
            "overflow title should move remaining preview into subtitle"
        );
    }

    #[test]
    fn script_filter_item_intent_returns_copy_update_delete_choices() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        let add = execute_add("seed memo", None, None, &config).expect("seed add");
        let query = format!("item {}", add.item_id);

        let feedback = build_script_filter(&query, &config).expect("script filter");

        assert_eq!(feedback.items.len(), 3);
        let expected_item_display = item_display_id(&add.item_id);
        let expected_item_route = item_route_id(&add.item_id);
        let expected_copy_title_prefix =
            format!("Copy memo: {} | seed memo", expected_item_display);
        assert!(
            feedback.items[0]
                .title
                .starts_with(expected_copy_title_prefix.as_str()),
            "copy row title should include memo text preview"
        );
        let expected_copy_arg = format!("copy::{}", add.item_id);
        assert_eq!(
            feedback.items[0].arg.as_deref(),
            Some(expected_copy_arg.as_str())
        );
        assert!(
            feedback.items[0].subtitle.is_none(),
            "copy row should not use subtitle for short preview"
        );
        let cmd_mod = feedback.items[0]
            .mods
            .as_ref()
            .and_then(|mods| mods.get("cmd"))
            .expect("copy row should include cmd modifier");
        let expected_copy_json_arg = format!("copy-json::{}", add.item_id);
        assert_eq!(
            cmd_mod.arg.as_deref(),
            Some(expected_copy_json_arg.as_str())
        );
        let cmd_subtitle = cmd_mod
            .subtitle
            .as_deref()
            .expect("cmd modifier subtitle should exist");
        assert!(
            cmd_subtitle.contains("Preview JSON:"),
            "cmd modifier subtitle should keep JSON preview"
        );

        let expected_update_title_prefix =
            format!("Update memo: {} | seed memo", expected_item_display);
        assert!(
            feedback.items[1]
                .title
                .starts_with(expected_update_title_prefix.as_str()),
            "update row title should include memo text preview"
        );
        let expected_update_autocomplete = format!("update {} ", expected_item_route);
        assert_eq!(
            feedback.items[1].autocomplete.as_deref(),
            Some(expected_update_autocomplete.as_str())
        );
        assert_eq!(feedback.items[1].valid, Some(false));
        assert!(
            feedback.items[1].subtitle.is_none(),
            "update row should not use subtitle for short preview"
        );

        let expected_delete_title_prefix =
            format!("Delete memo: {} | seed memo", expected_item_display);
        assert!(
            feedback.items[2]
                .title
                .starts_with(expected_delete_title_prefix.as_str()),
            "delete row title should include memo text preview"
        );
        let expected_delete_arg = format!("delete::{}", add.item_id);
        assert_eq!(
            feedback.items[2].arg.as_deref(),
            Some(expected_delete_arg.as_str())
        );
        assert_eq!(feedback.items[2].valid, Some(true));
        assert!(
            feedback.items[2].subtitle.is_none(),
            "delete row should not use subtitle for short preview"
        );
    }

    #[test]
    fn script_filter_returns_delete_action_for_delete_intent() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        let add = execute_add("delete me", None, None, &config).expect("seed add");
        let query = format!("delete {}", add.item_id);

        let feedback = build_script_filter(&query, &config).expect("script filter");

        assert_eq!(feedback.items.len(), 1);
        let arg = feedback.items[0].arg.as_deref().expect("arg should exist");
        assert!(arg.starts_with(DELETE_TOKEN_PREFIX));
        assert!(
            feedback.items[0].title.starts_with(
                format!("Delete memo: {} | delete me", item_display_id(&add.item_id)).as_str()
            ),
            "delete intent title should include memo text preview"
        );
        assert!(
            feedback.items[0].subtitle.is_none(),
            "delete intent should not use subtitle for short preview"
        );
        assert_eq!(feedback.items[0].valid, Some(true));
    }

    #[test]
    fn delete_row_overflow_moves_remaining_text_to_subtitle() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        let long_text = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod";
        let add = execute_add(long_text, None, None, &config).expect("seed add");
        let query = format!("delete {}", add.item_id);

        let feedback = build_script_filter(&query, &config).expect("script filter");

        assert_eq!(feedback.items.len(), 1);
        assert!(
            feedback.items[0].title.ends_with('…'),
            "overflow title should end with ellipsis"
        );
        assert!(
            feedback.items[0]
                .subtitle
                .as_deref()
                .map(|value| !value.is_empty())
                .unwrap_or(false),
            "overflow title should move remaining preview into subtitle"
        );
    }

    #[test]
    fn script_filter_returns_copy_action_for_copy_intent() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        let add = execute_add("seed memo", None, None, &config).expect("seed add");
        let query = format!("copy {}", add.item_id);

        let feedback = build_script_filter(&query, &config).expect("script filter");

        assert_eq!(feedback.items.len(), 1);
        let expected_item_display = item_display_id(&add.item_id);
        let expected_copy_title_prefix =
            format!("Copy memo: {} | seed memo", expected_item_display);
        assert!(
            feedback.items[0]
                .title
                .starts_with(expected_copy_title_prefix.as_str()),
            "copy intent title should include memo text preview"
        );
        let expected_copy_arg = format!("copy::{}", add.item_id);
        assert_eq!(
            feedback.items[0].arg.as_deref(),
            Some(expected_copy_arg.as_str())
        );
        assert!(
            feedback.items[0].subtitle.is_none(),
            "copy intent should not use subtitle for short preview"
        );
        let cmd_mod = feedback.items[0]
            .mods
            .as_ref()
            .and_then(|mods| mods.get("cmd"))
            .expect("copy row should include cmd modifier");
        let expected_copy_json_arg = format!("copy-json::{}", add.item_id);
        assert_eq!(
            cmd_mod.arg.as_deref(),
            Some(expected_copy_json_arg.as_str())
        );
    }

    #[test]
    fn script_filter_search_intent_returns_manage_rows() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        let add = execute_add("buy milk", None, None, &config).expect("seed add");

        let feedback = build_script_filter("search milk", &config).expect("script filter");

        assert_eq!(feedback.skip_knowledge, Some(true));
        assert_eq!(
            feedback.items.len(),
            1,
            "single search hit should keep selectable search row"
        );
        assert_eq!(feedback.items[0].valid, Some(false));
        let expected_autocomplete = format!("item {}", item_route_id(&add.item_id));
        assert_eq!(
            feedback.items[0].autocomplete.as_deref(),
            Some(expected_autocomplete.as_str())
        );
    }

    #[test]
    fn script_filter_search_intent_multiple_hits_keep_search_rows() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        execute_add("milk tea", None, None, &config).expect("seed add one");
        execute_add("milk coffee", None, None, &config).expect("seed add two");

        let feedback = build_script_filter("search milk", &config).expect("script filter");

        assert!(
            feedback.items.len() >= 2,
            "multiple search hits should keep search result rows"
        );
        assert_eq!(feedback.items[0].valid, Some(false));
        let autocomplete = feedback.items[0]
            .autocomplete
            .as_deref()
            .expect("search row should include autocomplete");
        assert!(
            autocomplete.starts_with("item "),
            "multi-hit search rows should route to item intent"
        );
    }

    #[test]
    fn script_filter_search_intent_requires_query_text() {
        let feedback = build_script_filter("search", &test_config()).expect("script filter");

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].valid, Some(false));
        assert_eq!(feedback.items[0].title, "Type search text after keyword");
    }

    #[test]
    fn script_filter_search_intent_supports_explicit_match_modes() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        let add = execute_add("123", None, None, &config).expect("seed add");

        let prefix_feedback =
            build_script_filter("search --match prefix 12", &config).expect("prefix search");
        assert_eq!(prefix_feedback.items.len(), 1);
        let expected_autocomplete = format!("item {}", item_route_id(&add.item_id));
        assert_eq!(
            prefix_feedback.items[0].autocomplete.as_deref(),
            Some(expected_autocomplete.as_str())
        );

        let contains_feedback =
            build_script_filter("search --match contains 23", &config).expect("contains search");
        assert_eq!(contains_feedback.items.len(), 1);
        assert_eq!(
            contains_feedback.items[0].autocomplete.as_deref(),
            Some(expected_autocomplete.as_str())
        );
    }

    #[test]
    fn script_filter_search_intent_uses_configured_default_match_mode() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        config.search_match_mode = SearchMatchMode::Prefix;
        let add = execute_add("123", None, None, &config).expect("seed add");

        let feedback = build_script_filter("search 12", &config).expect("prefix-by-default search");
        assert_eq!(feedback.items.len(), 1);
        let expected_autocomplete = format!("item {}", item_route_id(&add.item_id));
        assert_eq!(
            feedback.items[0].autocomplete.as_deref(),
            Some(expected_autocomplete.as_str())
        );
    }

    #[test]
    fn script_filter_search_intent_rejects_invalid_match_mode() {
        let feedback = build_script_filter("search --match fuzzy milk", &test_config())
            .expect("script filter");

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].valid, Some(false));
        assert_eq!(feedback.items[0].title, "Invalid search match mode: fuzzy");
    }

    #[test]
    fn execute_search_rejects_invalid_limit_or_window() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        execute_add("seed memo", None, None, &config).expect("seed add");

        let invalid_limit = execute_search(None, "seed", SearchMatchMode::Fts, 0, 0, &config);
        assert!(invalid_limit.is_err(), "search should reject limit=0");

        let invalid_window = execute_search(None, "seed", SearchMatchMode::Fts, 200, 400, &config);
        assert!(
            invalid_window.is_err(),
            "search should reject oversized window"
        );
    }

    #[test]
    fn script_filter_rejects_delete_with_invalid_item_id() {
        let feedback =
            build_script_filter("delete abc", &test_config()).expect("script filter should build");

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].valid, Some(false));
        assert_eq!(feedback.items[0].title, "Invalid item_id for delete");
    }

    #[test]
    fn script_filter_enforces_max_input_bytes() {
        let mut config = test_config();
        config.max_input_bytes = 4;

        let feedback = build_script_filter("12345", &config).expect("script filter should build");
        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].valid, Some(false));
    }

    #[test]
    fn parse_bool_supports_expected_values() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("false"), Some(false));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("unknown"), None);
    }

    #[test]
    fn expand_home_path_supports_tilde_prefix() {
        assert_eq!(
            expand_home_path("~/memo.db", Some("/tmp/home")),
            "/tmp/home/memo.db"
        );
        assert_eq!(
            expand_home_path("$HOME/memo.db", Some("/tmp/home")),
            "/tmp/home/memo.db"
        );
    }

    #[test]
    fn copy_row_overflow_moves_remaining_text_to_subtitle() {
        let dir = tempdir().expect("temp dir");
        let mut config = test_config();
        config.db_path = dir.path().join("memo.db");
        let long_text =
            "copy overflow lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod";
        let add = execute_add(long_text, None, None, &config).expect("seed add");
        let query = format!("copy {}", add.item_id);

        let feedback = build_script_filter(&query, &config).expect("script filter");

        assert_eq!(feedback.items.len(), 1);
        assert!(
            feedback.items[0].title.ends_with('…'),
            "overflow title should end with ellipsis"
        );
        assert!(
            feedback.items[0]
                .subtitle
                .as_deref()
                .map(|value| !value.is_empty())
                .unwrap_or(false),
            "overflow title should move remaining preview into subtitle"
        );
    }
}
