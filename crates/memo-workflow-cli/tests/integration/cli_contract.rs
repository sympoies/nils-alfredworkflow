use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

fn bin() -> String {
    resolve_cli_path().display().to_string()
}

fn item_route_id(item_id: &str) -> String {
    item_id
        .strip_prefix("itm_")
        .and_then(|raw| raw.parse::<u64>().ok())
        .map(|id| id.to_string())
        .unwrap_or_else(|| item_id.to_string())
}

fn item_display_id(item_id: &str) -> String {
    item_id
        .strip_prefix("itm_")
        .and_then(|raw| raw.parse::<u64>().ok())
        .map(|id| format!("#{id}"))
        .unwrap_or_else(|| item_id.to_string())
}

fn assert_json_success_envelope(payload: &Value, command: &str) {
    assert_eq!(
        payload.get("schema_version").and_then(Value::as_str),
        Some("cli-envelope@v1")
    );
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some(command)
    );
    assert_eq!(payload.get("ok").and_then(Value::as_bool), Some(true));
}

#[test]
fn script_filter_returns_items_array() {
    let output = Command::new(bin())
        .args(["script-filter", "--query", "buy milk"])
        .output()
        .expect("script-filter should run");

    assert!(output.status.success(), "script-filter must exit 0");

    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("script-filter stdout must be JSON");
    assert!(payload.get("items").and_then(Value::as_array).is_some());
}

#[test]
fn script_filter_empty_query_includes_db_init_row() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");

    let output = Command::new(bin())
        .args(["script-filter", "--query", ""])
        .env("MEMO_DB_PATH", db.to_str().expect("db path"))
        .output()
        .expect("script-filter should run");

    assert!(output.status.success(), "script-filter must exit 0");

    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("script-filter stdout must be JSON");
    let has_db_init = payload
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .any(|item| item.get("arg").and_then(Value::as_str) == Some("db-init"))
        })
        .unwrap_or(false);

    assert!(has_db_init, "empty query should include db-init action row");

    let has_db_path_info = payload
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .any(|item| item.get("title").and_then(Value::as_str) == Some("Memo database path"))
        })
        .unwrap_or(false);

    assert!(
        !has_db_path_info,
        "empty query without db should not show db path info row"
    );
}

#[test]
fn script_filter_empty_query_with_existing_db_shows_db_path_row_without_db_init() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let init = Command::new(bin())
        .args(["db-init", "--db", db_path])
        .output()
        .expect("db-init should run");
    assert!(init.status.success(), "db-init must exit 0");

    let output = Command::new(bin())
        .args(["script-filter", "--query", ""])
        .env("MEMO_DB_PATH", db_path)
        .output()
        .expect("script-filter should run");
    assert!(output.status.success(), "script-filter must exit 0");

    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("script-filter stdout must be JSON");
    let items = payload
        .get("items")
        .and_then(Value::as_array)
        .expect("items array");

    let has_db_init = items
        .iter()
        .any(|item| item.get("arg").and_then(Value::as_str) == Some("db-init"));
    assert!(
        !has_db_init,
        "empty query with existing db should not include db-init action row"
    );

    let db_path_subtitle = format!("Using SQLite at {db_path}");
    let has_db_path_info = items.iter().any(|item| {
        item.get("title").and_then(Value::as_str) == Some("Memo database path")
            && item.get("subtitle").and_then(Value::as_str) == Some(db_path_subtitle.as_str())
    });
    assert!(
        has_db_path_info,
        "empty query with existing db should show db path info row"
    );
}

#[test]
fn script_filter_recent_rows_offer_manage_autocomplete() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add = Command::new(bin())
        .args([
            "add",
            "--db",
            db_path,
            "--text",
            "manage me",
            "--mode",
            "json",
        ])
        .output()
        .expect("add should run");
    assert!(add.status.success(), "add should succeed");
    let add_payload: Value = serde_json::from_slice(&add.stdout).expect("add json");
    let item_id = add_payload
        .get("result")
        .and_then(|result| result.get("item_id"))
        .and_then(Value::as_str)
        .expect("item id");

    let output = Command::new(bin())
        .args(["script-filter", "--query", ""])
        .env("MEMO_DB_PATH", db_path)
        .output()
        .expect("script-filter should run");
    assert!(output.status.success(), "script-filter should exit 0");

    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("script-filter stdout must be JSON");
    assert_eq!(
        payload.get("skipknowledge").and_then(Value::as_bool),
        Some(true),
        "recent memo uid rows must stay newest-first"
    );
    let items = payload
        .get("items")
        .and_then(Value::as_array)
        .expect("items array");

    let expected_uid = format!("recent-{item_id}");
    let expected_autocomplete = format!("item {}", item_route_id(item_id));
    let has_manage_row = items.iter().any(|item| {
        item.get("uid").and_then(Value::as_str) == Some(expected_uid.as_str())
            && item.get("autocomplete").and_then(Value::as_str)
                == Some(expected_autocomplete.as_str())
            && item
                .get("subtitle")
                .and_then(Value::as_str)
                .map(|subtitle| subtitle.contains("Press Enter to manage"))
                .unwrap_or(false)
    });
    assert!(
        has_manage_row,
        "recent row should expose item autocomplete for manage flow"
    );
}

#[test]
fn script_filter_item_intent_shows_copy_update_delete_menu_and_update_guidance() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add = Command::new(bin())
        .args([
            "add",
            "--db",
            db_path,
            "--text",
            "menu seed",
            "--mode",
            "json",
        ])
        .output()
        .expect("add should run");
    assert!(add.status.success(), "add should succeed");
    let add_payload: Value = serde_json::from_slice(&add.stdout).expect("add payload json");
    let item_id = add_payload
        .get("result")
        .and_then(|result| result.get("item_id"))
        .and_then(Value::as_str)
        .expect("item id");

    let item_query = format!("item {item_id}");
    let menu = Command::new(bin())
        .args(["script-filter", "--query", &item_query])
        .env("MEMO_DB_PATH", db_path)
        .output()
        .expect("script-filter item intent should run");
    assert!(menu.status.success(), "item intent should succeed");

    let menu_payload: Value =
        serde_json::from_slice(&menu.stdout).expect("item intent payload json");
    let menu_items = menu_payload
        .get("items")
        .and_then(Value::as_array)
        .expect("items array");
    assert_eq!(
        menu_items.len(),
        3,
        "item intent should render three menu rows"
    );
    let expected_copy_title_prefix = format!("Copy memo: {} | menu seed", item_display_id(item_id));
    let expected_copy_arg = format!("copy::{item_id}");
    let expected_copy_json_arg = format!("copy-json::{item_id}");
    let expected_update_title_prefix =
        format!("Update memo: {} | menu seed", item_display_id(item_id));
    let expected_update_autocomplete = format!("update {} ", item_route_id(item_id));
    let expected_delete_arg = format!("delete::{item_id}");
    assert!(
        menu_items[0]
            .get("title")
            .and_then(Value::as_str)
            .map(|value| value.starts_with(expected_copy_title_prefix.as_str()))
            .unwrap_or(false),
        "item menu copy title should include memo preview"
    );
    assert_eq!(
        menu_items[0].get("arg").and_then(Value::as_str),
        Some(expected_copy_arg.as_str())
    );
    assert!(
        menu_items[0].get("subtitle").is_none(),
        "item menu copy subtitle should stay empty for short preview"
    );
    assert_eq!(
        menu_items[0]
            .get("mods")
            .and_then(|mods| mods.get("cmd"))
            .and_then(|cmd| cmd.get("arg"))
            .and_then(Value::as_str),
        Some(expected_copy_json_arg.as_str())
    );
    assert!(
        menu_items[1]
            .get("title")
            .and_then(Value::as_str)
            .map(|value| value.starts_with(expected_update_title_prefix.as_str()))
            .unwrap_or(false),
        "item menu update title should include memo preview"
    );
    assert_eq!(
        menu_items[1].get("autocomplete").and_then(Value::as_str),
        Some(expected_update_autocomplete.as_str())
    );
    assert!(
        menu_items[1].get("subtitle").is_none(),
        "item menu update subtitle should stay empty for short preview"
    );
    assert_eq!(
        menu_items[2].get("arg").and_then(Value::as_str),
        Some(expected_delete_arg.as_str())
    );

    let update_guidance_query = format!("update {}", item_route_id(item_id));
    let update_guidance = Command::new(bin())
        .args(["script-filter", "--query", &update_guidance_query])
        .env("MEMO_DB_PATH", db_path)
        .output()
        .expect("script-filter update guidance should run");
    assert!(
        update_guidance.status.success(),
        "update guidance should succeed"
    );

    let guidance_payload: Value =
        serde_json::from_slice(&update_guidance.stdout).expect("guidance payload json");
    let guidance_items = guidance_payload
        .get("items")
        .and_then(Value::as_array)
        .expect("items array");
    assert_eq!(guidance_items.len(), 1, "update guidance should be one row");
    assert!(
        guidance_items[0]
            .get("title")
            .and_then(Value::as_str)
            .map(|value| value.starts_with(expected_update_title_prefix.as_str()))
            .unwrap_or(false),
        "update guidance title should include memo preview"
    );
    assert_eq!(
        guidance_items[0]
            .get("autocomplete")
            .and_then(Value::as_str),
        Some(expected_update_autocomplete.as_str())
    );
    assert_eq!(
        guidance_items[0].get("valid").and_then(Value::as_bool),
        Some(false)
    );

    let update_execute_query = format!("update {} changed text", item_route_id(item_id));
    let update_execute = Command::new(bin())
        .args(["script-filter", "--query", &update_execute_query])
        .env("MEMO_DB_PATH", db_path)
        .output()
        .expect("script-filter update execute should run");
    assert!(
        update_execute.status.success(),
        "update execute should succeed"
    );
    let execute_payload: Value =
        serde_json::from_slice(&update_execute.stdout).expect("execute payload json");
    let execute_items = execute_payload
        .get("items")
        .and_then(Value::as_array)
        .expect("items array");
    let expected_update_arg = format!("update::{item_id}::changed text");
    assert_eq!(
        execute_items[0].get("arg").and_then(Value::as_str),
        Some(expected_update_arg.as_str())
    );
}

#[test]
fn db_init_creates_database() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");

    let output = Command::new(bin())
        .args(["db-init", "--db", db.to_str().expect("db path")])
        .output()
        .expect("db-init should run");

    assert!(output.status.success(), "db-init must exit 0");
    assert!(db.exists(), "db file should be created");
}

#[test]
fn add_writes_one_row() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");

    let status = Command::new(bin())
        .args(["db-init", "--db", db.to_str().expect("db path")])
        .status()
        .expect("db-init should run");
    assert!(status.success(), "db-init should succeed");

    let output = Command::new(bin())
        .args([
            "add",
            "--db",
            db.to_str().expect("db path"),
            "--text",
            "buy milk",
            "--source",
            "alfred-test",
            "--mode",
            "json",
        ])
        .output()
        .expect("add should run");

    assert!(output.status.success(), "add must exit 0");

    let payload: Value = serde_json::from_slice(&output.stdout).expect("add stdout must be JSON");
    assert_json_success_envelope(&payload, "memo.add");
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("source"))
            .and_then(Value::as_str),
        Some("alfred-test")
    );
}

#[test]
fn add_rejects_empty_text() {
    let output = Command::new(bin())
        .args(["add", "--text", "   "])
        .output()
        .expect("add should run");

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn list_returns_latest_first() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let first = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "first memo"])
        .output()
        .expect("first add should run");
    assert!(first.status.success(), "first add should succeed");

    let second = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "second memo"])
        .output()
        .expect("second add should run");
    assert!(second.status.success(), "second add should succeed");

    let list = Command::new(bin())
        .args(["list", "--db", db_path, "--limit", "2", "--mode", "json"])
        .output()
        .expect("list should run");
    assert!(list.status.success(), "list should exit 0");

    let payload: Value = serde_json::from_slice(&list.stdout).expect("list stdout must be JSON");
    assert_json_success_envelope(&payload, "memo.list");
    let rows = payload
        .get("result")
        .and_then(Value::as_array)
        .expect("result must be an array");
    assert_eq!(rows.len(), 2, "list should return two rows");

    let first_preview = rows[0]
        .get("text_preview")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        first_preview.contains("second memo"),
        "newest row should be listed first"
    );
}

#[test]
fn update_mutates_existing_item() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "before", "--mode", "json"])
        .output()
        .expect("add should run");
    assert!(add.status.success(), "add should succeed");
    let add_payload: Value = serde_json::from_slice(&add.stdout).expect("add json");
    let item_id = add_payload
        .get("result")
        .and_then(|result| result.get("item_id"))
        .and_then(Value::as_str)
        .expect("item id")
        .to_string();

    let update = Command::new(bin())
        .args([
            "update",
            "--db",
            db_path,
            "--item-id",
            &item_id,
            "--text",
            "after",
            "--mode",
            "json",
        ])
        .output()
        .expect("update should run");
    assert!(update.status.success(), "update should succeed");
    let update_payload: Value = serde_json::from_slice(&update.stdout).expect("update json");
    assert_json_success_envelope(&update_payload, "memo.update");
    assert_eq!(
        update_payload
            .get("result")
            .and_then(|result| result.get("text"))
            .and_then(Value::as_str),
        Some("after")
    );
}

#[test]
fn delete_removes_existing_item() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add = Command::new(bin())
        .args([
            "add",
            "--db",
            db_path,
            "--text",
            "to delete",
            "--mode",
            "json",
        ])
        .output()
        .expect("add should run");
    assert!(add.status.success(), "add should succeed");
    let add_payload: Value = serde_json::from_slice(&add.stdout).expect("add json");
    let item_id = add_payload
        .get("result")
        .and_then(|result| result.get("item_id"))
        .and_then(Value::as_str)
        .expect("item id")
        .to_string();

    let delete = Command::new(bin())
        .args([
            "delete",
            "--db",
            db_path,
            "--item-id",
            &item_id,
            "--mode",
            "json",
        ])
        .output()
        .expect("delete should run");
    assert!(delete.status.success(), "delete should succeed");

    let list = Command::new(bin())
        .args(["list", "--db", db_path, "--mode", "json"])
        .output()
        .expect("list should run");
    assert!(list.status.success(), "list should succeed");
    let list_payload: Value = serde_json::from_slice(&list.stdout).expect("list json");
    let rows = list_payload
        .get("result")
        .and_then(Value::as_array)
        .expect("list rows");
    assert!(
        rows.iter().all(|row| {
            row.get("item_id")
                .and_then(Value::as_str)
                .map(|id| id != item_id)
                .unwrap_or(true)
        }),
        "deleted item should not appear in list"
    );
}

#[test]
fn action_token_crud_roundtrip_with_isolated_db() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add = Command::new(bin())
        .args([
            "action",
            "--token",
            "add::first memo",
            "--db",
            db_path,
            "--mode",
            "json",
        ])
        .output()
        .expect("action add should run");
    assert!(add.status.success(), "action add should succeed");
    let add_payload: Value = serde_json::from_slice(&add.stdout).expect("add json");
    let item_id = add_payload
        .get("result")
        .and_then(|result| result.get("item_id"))
        .and_then(Value::as_str)
        .expect("item id")
        .to_string();

    let update_token = format!("update::{item_id}::updated memo");
    let update = Command::new(bin())
        .args([
            "action",
            "--token",
            &update_token,
            "--db",
            db_path,
            "--mode",
            "json",
        ])
        .output()
        .expect("action update should run");
    assert!(update.status.success(), "action update should succeed");

    let copy_token = format!("copy::{item_id}");
    let copy = Command::new(bin())
        .args(["action", "--token", &copy_token, "--db", db_path])
        .output()
        .expect("action copy should run");
    assert!(copy.status.success(), "action copy should succeed");
    assert_eq!(
        String::from_utf8_lossy(&copy.stdout).trim_end(),
        "updated memo",
        "copy token should output raw memo text in text mode"
    );

    let copy_json_token = format!("copy-json::{item_id}");
    let copy_json = Command::new(bin())
        .args(["action", "--token", &copy_json_token, "--db", db_path])
        .output()
        .expect("action copy-json should run");
    assert!(
        copy_json.status.success(),
        "action copy-json should succeed"
    );
    let copied_payload: Value =
        serde_json::from_slice(&copy_json.stdout).expect("copy-json should output item json");
    assert_eq!(
        copied_payload.get("item_id").and_then(Value::as_str),
        Some(item_id.as_str())
    );
    assert_eq!(
        copied_payload.get("text").and_then(Value::as_str),
        Some("updated memo")
    );

    let delete_token = format!("delete::{item_id}");
    let delete = Command::new(bin())
        .args([
            "action",
            "--token",
            &delete_token,
            "--db",
            db_path,
            "--mode",
            "json",
        ])
        .output()
        .expect("action delete should run");
    assert!(delete.status.success(), "action delete should succeed");
}

#[test]
fn script_filter_exposes_update_delete_intents() {
    let output_update = Command::new(bin())
        .args([
            "script-filter",
            "--query",
            "update itm_00000001 revised text",
        ])
        .output()
        .expect("script-filter update should run");
    assert!(
        output_update.status.success(),
        "script-filter update must exit 0"
    );
    let update_payload: Value =
        serde_json::from_slice(&output_update.stdout).expect("update payload json");
    let update_arg = update_payload
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("arg"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        update_arg.starts_with("update::"),
        "update intent should produce update token"
    );

    let output_delete = Command::new(bin())
        .args(["script-filter", "--query", "delete itm_00000001"])
        .output()
        .expect("script-filter delete should run");
    assert!(
        output_delete.status.success(),
        "script-filter delete must exit 0"
    );
    let delete_payload: Value =
        serde_json::from_slice(&output_delete.stdout).expect("delete payload json");
    let delete_arg = delete_payload
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("arg"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        delete_arg.starts_with("delete::"),
        "delete intent should produce delete token"
    );
}

#[test]
fn search_command_returns_matching_rows() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add_one = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "buy milk"])
        .output()
        .expect("first add should run");
    assert!(add_one.status.success(), "first add should succeed");

    let add_two = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "buy oat milk"])
        .output()
        .expect("second add should run");
    assert!(add_two.status.success(), "second add should succeed");

    let search = Command::new(bin())
        .args([
            "search", "--db", db_path, "--query", "oat", "--mode", "json",
        ])
        .output()
        .expect("search should run");
    assert!(search.status.success(), "search should succeed");

    let payload: Value = serde_json::from_slice(&search.stdout).expect("search payload json");
    assert_json_success_envelope(&payload, "memo.search");
    let rows = payload
        .get("result")
        .and_then(Value::as_array)
        .expect("search result rows");
    assert!(!rows.is_empty(), "search should return rows");
    assert!(
        rows[0].get("item_id").and_then(Value::as_str).is_some(),
        "search row should include item_id"
    );
    assert!(
        rows[0]
            .get("matched_fields")
            .and_then(Value::as_array)
            .is_some(),
        "search row should include matched_fields"
    );
}

#[test]
fn search_command_supports_prefix_and_contains_match_modes() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "123"])
        .output()
        .expect("add should run");
    assert!(add.status.success(), "add should succeed");

    let prefix = Command::new(bin())
        .args([
            "search", "--db", db_path, "--query", "12", "--match", "prefix", "--mode", "json",
        ])
        .output()
        .expect("prefix search should run");
    assert!(prefix.status.success(), "prefix search should succeed");
    let prefix_payload: Value =
        serde_json::from_slice(&prefix.stdout).expect("prefix search payload json");
    assert_json_success_envelope(&prefix_payload, "memo.search");
    let prefix_rows = prefix_payload
        .get("result")
        .and_then(Value::as_array)
        .expect("prefix result rows");
    assert!(
        !prefix_rows.is_empty(),
        "prefix mode should match 12 against stored 123"
    );

    let contains = Command::new(bin())
        .args([
            "search", "--db", db_path, "--query", "23", "--match", "contains", "--mode", "json",
        ])
        .output()
        .expect("contains search should run");
    assert!(contains.status.success(), "contains search should succeed");
    let contains_payload: Value =
        serde_json::from_slice(&contains.stdout).expect("contains search payload json");
    assert_json_success_envelope(&contains_payload, "memo.search");
    let contains_rows = contains_payload
        .get("result")
        .and_then(Value::as_array)
        .expect("contains result rows");
    assert!(
        !contains_rows.is_empty(),
        "contains mode should match 23 against stored 123"
    );
}

#[test]
fn script_filter_search_intent_uses_env_default_match_mode() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "123"])
        .output()
        .expect("add should run");
    assert!(add.status.success(), "add should succeed");

    let output = Command::new(bin())
        .args(["script-filter", "--query", "search 12"])
        .env("MEMO_DB_PATH", db_path)
        .env("MEMO_SEARCH_MATCH", "prefix")
        .output()
        .expect("script-filter should run");
    assert!(output.status.success(), "script-filter should succeed");

    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("script-filter payload json");
    let items = payload
        .get("items")
        .and_then(Value::as_array)
        .expect("items array");
    assert_eq!(items.len(), 1, "prefix default should match 12 against 123");
    let autocomplete = items[0]
        .get("autocomplete")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        autocomplete.starts_with("item "),
        "configured search mode should still route to item autocomplete"
    );
}

#[test]
fn script_filter_search_intent_keeps_search_row_for_single_hit() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "search target"])
        .output()
        .expect("add should run");
    assert!(add.status.success(), "add should succeed");

    let output = Command::new(bin())
        .args(["script-filter", "--query", "search target"])
        .env("MEMO_DB_PATH", db_path)
        .output()
        .expect("script-filter should run");
    assert!(output.status.success(), "script-filter should succeed");

    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("script-filter payload json");
    let items = payload
        .get("items")
        .and_then(Value::as_array)
        .expect("items array");
    assert_eq!(
        items.len(),
        1,
        "single search hit should keep one search row"
    );
    assert_eq!(
        items[0].get("valid").and_then(Value::as_bool),
        Some(false),
        "single-hit search row should remain non-actionable"
    );
    let autocomplete = items[0]
        .get("autocomplete")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        autocomplete.starts_with("item "),
        "single-hit search row should keep item autocomplete"
    );
}

#[test]
fn script_filter_search_intent_keeps_search_rows_for_multi_hit() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("memo.db");
    let db_path = db.to_str().expect("db path");

    let add_one = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "milk tea"])
        .output()
        .expect("first add should run");
    assert!(add_one.status.success(), "first add should succeed");

    let add_two = Command::new(bin())
        .args(["add", "--db", db_path, "--text", "milk coffee"])
        .output()
        .expect("second add should run");
    assert!(add_two.status.success(), "second add should succeed");

    let output = Command::new(bin())
        .args(["script-filter", "--query", "search milk"])
        .env("MEMO_DB_PATH", db_path)
        .output()
        .expect("script-filter should run");
    assert!(output.status.success(), "script-filter should succeed");

    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("script-filter payload json");
    let items = payload
        .get("items")
        .and_then(Value::as_array)
        .expect("items array");
    assert!(
        items.len() >= 2,
        "multi-hit search should keep searchable item rows"
    );

    let autocomplete = items[0]
        .get("autocomplete")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        autocomplete.starts_with("item "),
        "multi-hit search row should keep item autocomplete"
    );
}

#[test]
fn update_delete_invalid_item_id_returns_usage_error() {
    let update = Command::new(bin())
        .args([
            "update",
            "--item-id",
            "bad",
            "--text",
            "updated",
            "--mode",
            "json",
        ])
        .output()
        .expect("update should run");
    assert_eq!(update.status.code(), Some(2));
    let update_stderr = String::from_utf8_lossy(&update.stderr);
    assert!(
        update_stderr.contains("error[NILS_MEMO_001]"),
        "update stderr must surface NILS_MEMO_001, got: {update_stderr}"
    );

    let delete = Command::new(bin())
        .args(["delete", "--item-id", "bad", "--mode", "json"])
        .output()
        .expect("delete should run");
    assert_eq!(delete.status.code(), Some(2));
    let delete_stderr = String::from_utf8_lossy(&delete.stderr);
    assert!(
        delete_stderr.contains("error[NILS_MEMO_001]"),
        "delete stderr must surface NILS_MEMO_001, got: {delete_stderr}"
    );
}

fn resolve_cli_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_memo-workflow-cli") {
        return PathBuf::from(path);
    }

    if let Ok(current_exe) = std::env::current_exe()
        && let Some(debug_dir) = current_exe.parent().and_then(|deps| deps.parent())
    {
        let candidate =
            debug_dir.join(format!("memo-workflow-cli{}", std::env::consts::EXE_SUFFIX));
        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from(env!("CARGO_BIN_EXE_memo-workflow-cli"))
}
