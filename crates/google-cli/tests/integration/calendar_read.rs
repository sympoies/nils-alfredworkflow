use crate::native_calendar;

use serde_json::{Value, json};
use tempfile::tempdir;

const CALENDAR_ID: &str = "cal-home@group.calendar.google.com";

fn fixture_payload() -> Value {
    json!({
        "calendars": [
            {
                "id": CALENDAR_ID,
                "summary": "Hermes · Home",
                "time_zone": "Asia/Taipei",
                "access_role": "owner",
                "primary": false
            },
            {
                "id": "primary@example.com",
                "summary": "Primary",
                "time_zone": "Asia/Taipei",
                "access_role": "owner",
                "primary": true
            }
        ],
        "events": [
            {
                "id": "ev-hotpot",
                "summary": "全家去吃千葉火鍋",
                "status": "confirmed",
                "start": "2026-08-15T11:20:00+08:00",
                "end": "2026-08-15T13:00:00+08:00",
                "all_day": false,
                "location": "千葉火鍋",
                "description": "Original family dinner",
                "html_link": "https://example.invalid/ev-hotpot",
                "private_properties": {
                    "gn_platform": "line",
                    "gn_group_id": "C5be",
                    "gn_note_id": "n_e923e876"
                }
            },
            {
                "id": "ev-other-group",
                "summary": "Unrelated",
                "status": "confirmed",
                "start": "2026-08-16T09:00:00+08:00",
                "end": "2026-08-16T10:00:00+08:00",
                "all_day": false,
                "private_properties": {
                    "gn_group_id": "OTHER"
                }
            }
        ]
    })
}

#[test]
fn calendars_list_returns_native_account_and_items() {
    let temp = tempdir().expect("tempdir");
    native_calendar::seed_account(temp.path(), "default@example.com");
    let fixture = native_calendar::write_fixture(temp.path(), &fixture_payload());
    let (key, value) = native_calendar::fixture_env(&fixture);

    let output = native_calendar::run(
        temp.path(),
        &["--output", "json", "calendar", "calendars", "list"],
        &[(key, value.as_str())],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = native_calendar::json(&output);
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.calendar.calendars.list")
    );
    let result = payload.get("result").expect("result");
    assert_eq!(result.get("count").and_then(Value::as_u64), Some(2));
    assert_eq!(result["calendars"][0]["id"].as_str(), Some(CALENDAR_ID));
    assert_eq!(
        result["calendars"][0]["time_zone"].as_str(),
        Some("Asia/Taipei")
    );
}

#[test]
fn events_list_filters_by_private_extended_property() {
    let temp = tempdir().expect("tempdir");
    native_calendar::seed_account(temp.path(), "default@example.com");
    let fixture = native_calendar::write_fixture(temp.path(), &fixture_payload());
    let (key, value) = native_calendar::fixture_env(&fixture);

    let output = native_calendar::run(
        temp.path(),
        &[
            "--output",
            "json",
            "calendar",
            "events",
            "list",
            "--calendar-id",
            CALENDAR_ID,
            "--private-property",
            "gn_group_id=C5be",
        ],
        &[(key, value.as_str())],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = native_calendar::json(&output);
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.calendar.events.list")
    );
    let result = payload.get("result").expect("result");
    assert_eq!(result.get("count").and_then(Value::as_u64), Some(1));
    assert_eq!(result["events"][0]["id"].as_str(), Some("ev-hotpot"));
    assert_eq!(
        result["events"][0]["private_properties"]["gn_note_id"].as_str(),
        Some("n_e923e876")
    );
}

#[test]
fn events_get_resolves_by_id_and_reports_missing_ids() {
    let temp = tempdir().expect("tempdir");
    native_calendar::seed_account(temp.path(), "default@example.com");
    let fixture = native_calendar::write_fixture(temp.path(), &fixture_payload());
    let (key, value) = native_calendar::fixture_env(&fixture);

    let found = native_calendar::run(
        temp.path(),
        &[
            "--output",
            "json",
            "calendar",
            "events",
            "get",
            "ev-hotpot",
            "--calendar-id",
            CALENDAR_ID,
        ],
        &[(key, value.as_str())],
    );
    assert_eq!(found.status.code(), Some(0));
    let payload = native_calendar::json(&found);
    assert_eq!(
        payload["result"]["event"]["summary"].as_str(),
        Some("全家去吃千葉火鍋")
    );

    let missing = native_calendar::run(
        temp.path(),
        &[
            "--output",
            "json",
            "calendar",
            "events",
            "get",
            "ev-nope",
            "--calendar-id",
            CALENDAR_ID,
        ],
        &[(key, value.as_str())],
    );
    assert_ne!(missing.status.code(), Some(0));
    let error = native_calendar::json(&missing);
    assert_eq!(error.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        error["error"]["code"].as_str(),
        Some("NILS_GOOGLE_016"),
        "missing events should map to the calendar not-found code"
    );
}

#[test]
fn events_create_builds_a_timed_event_with_back_references() {
    let temp = tempdir().expect("tempdir");
    native_calendar::seed_account(temp.path(), "default@example.com");
    let fixture = native_calendar::write_fixture(temp.path(), &fixture_payload());
    let (key, value) = native_calendar::fixture_env(&fixture);

    let output = native_calendar::run(
        temp.path(),
        &[
            "--output",
            "json",
            "calendar",
            "events",
            "create",
            "--calendar-id",
            CALENDAR_ID,
            "--summary",
            "全家去吃千葉火鍋",
            "--start",
            "2026-08-15T11:20",
            "--end",
            "2026-08-15T13:00",
            "--time-zone",
            "Asia/Taipei",
            "--location",
            "千葉火鍋",
            "--private-property",
            "gn_note_id=n_e923e876",
        ],
        &[(key, value.as_str())],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = native_calendar::json(&output);
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.calendar.events.create")
    );
    let result = payload.get("result").expect("result");
    assert_eq!(
        result.get("fixture_mode").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        result["event"]["summary"].as_str(),
        Some("全家去吃千葉火鍋")
    );
    assert_eq!(
        result["event"]["private_properties"]["gn_note_id"].as_str(),
        Some("n_e923e876")
    );
    // `--start 2026-08-15T11:20` must reach the API as a complete RFC3339
    // value; Calendar v3 answers HTTP 400 for an `HH:MM` dateTime.
    assert_eq!(
        result["event"]["start"].as_str(),
        Some("2026-08-15T11:20:00")
    );
    assert_eq!(result["event"]["end"].as_str(), Some("2026-08-15T13:00:00"));
}

#[test]
fn events_create_rejects_a_naive_start_without_a_zone() {
    let temp = tempdir().expect("tempdir");
    native_calendar::seed_account(temp.path(), "default@example.com");
    let fixture = native_calendar::write_fixture(temp.path(), &fixture_payload());
    let (key, value) = native_calendar::fixture_env(&fixture);

    let output = native_calendar::run(
        temp.path(),
        &[
            "--output",
            "json",
            "calendar",
            "events",
            "create",
            "--calendar-id",
            CALENDAR_ID,
            "--summary",
            "Ambiguous",
            "--start",
            "2026-08-15T11:20",
            "--end",
            "2026-08-15T13:00",
        ],
        &[(key, value.as_str())],
    );
    assert_ne!(output.status.code(), Some(0));

    let error = native_calendar::json(&output);
    assert_eq!(
        error["error"]["code"].as_str(),
        Some("NILS_GOOGLE_015"),
        "an ambiguous wall-clock start is a user input error"
    );
}

#[test]
fn events_delete_confirms_the_target_and_reports_missing_ids() {
    let temp = tempdir().expect("tempdir");
    native_calendar::seed_account(temp.path(), "default@example.com");
    let fixture = native_calendar::write_fixture(temp.path(), &fixture_payload());
    let (key, value) = native_calendar::fixture_env(&fixture);

    let deleted = native_calendar::run(
        temp.path(),
        &[
            "--output",
            "json",
            "calendar",
            "events",
            "delete",
            "ev-hotpot",
            "--calendar-id",
            CALENDAR_ID,
        ],
        &[(key, value.as_str())],
    );
    assert_eq!(deleted.status.code(), Some(0));
    let payload = native_calendar::json(&deleted);
    assert_eq!(payload["result"]["deleted"].as_bool(), Some(true));
    assert_eq!(payload["result"]["event_id"].as_str(), Some("ev-hotpot"));
    assert_eq!(payload["result"]["calendar_id"].as_str(), Some(CALENDAR_ID));

    // Deleting an id that is not there must not report success; a caller that
    // trusted `deleted: true` would tell a chat group the event is gone when
    // some other event still stands.
    let missing = native_calendar::run(
        temp.path(),
        &[
            "--output",
            "json",
            "calendar",
            "events",
            "delete",
            "ev-nope",
            "--calendar-id",
            CALENDAR_ID,
        ],
        &[(key, value.as_str())],
    );
    assert_ne!(missing.status.code(), Some(0));
    let error = native_calendar::json(&missing);
    assert_eq!(error.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        error["error"]["code"].as_str(),
        Some("NILS_GOOGLE_016"),
        "a missing event must map to the calendar not-found code"
    );
}

#[test]
fn events_delete_requires_a_calendar_and_an_event_id() {
    let temp = tempdir().expect("tempdir");
    native_calendar::seed_account(temp.path(), "default@example.com");
    let fixture = native_calendar::write_fixture(temp.path(), &fixture_payload());
    let (key, value) = native_calendar::fixture_env(&fixture);

    for arguments in [
        vec![
            "--output",
            "json",
            "calendar",
            "events",
            "delete",
            "ev-hotpot",
        ],
        vec![
            "--output",
            "json",
            "calendar",
            "events",
            "delete",
            "--calendar-id",
            CALENDAR_ID,
        ],
    ] {
        let output = native_calendar::run(temp.path(), &arguments, &[(key, value.as_str())]);
        assert_ne!(output.status.code(), Some(0));
        let error = native_calendar::json(&output);
        assert_eq!(
            error["error"]["code"].as_str(),
            Some("NILS_GOOGLE_015"),
            "an incomplete delete is a user input error"
        );
    }
}

#[test]
fn events_update_changes_only_supplied_fields() {
    let temp = tempdir().expect("tempdir");
    native_calendar::seed_account(temp.path(), "default@example.com");
    let fixture = native_calendar::write_fixture(temp.path(), &fixture_payload());
    let (key, value) = native_calendar::fixture_env(&fixture);

    let output = native_calendar::run(
        temp.path(),
        &[
            "--output",
            "json",
            "calendar",
            "events",
            "update",
            "ev-hotpot",
            "--calendar-id",
            CALENDAR_ID,
            "--summary",
            "Updated dinner",
            "--location",
            "New restaurant",
        ],
        &[(key, value.as_str())],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = native_calendar::json(&output);
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.calendar.events.update")
    );
    let event = &payload["result"]["event"];
    assert_eq!(event["id"].as_str(), Some("ev-hotpot"));
    assert_eq!(event["summary"].as_str(), Some("Updated dinner"));
    assert_eq!(event["location"].as_str(), Some("New restaurant"));
    assert_eq!(
        event["description"].as_str(),
        Some("Original family dinner")
    );
    assert_eq!(event["start"].as_str(), Some("2026-08-15T11:20:00+08:00"));
    assert_eq!(event["end"].as_str(), Some("2026-08-15T13:00:00+08:00"));
}
