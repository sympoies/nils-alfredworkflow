#[path = "common/native_gmail.rs"]
mod native_gmail;

use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn gmail_json_contract_covers_search_get_thread_and_send() {
    let temp = tempdir().expect("tempdir");
    native_gmail::seed_account(temp.path(), "me@example.com");

    let fixture_path = native_gmail::write_fixture(
        temp.path(),
        &json!({
            "messages": [
                {
                    "id": "msg-1",
                    "thread_id": "thread-1",
                    "snippet": "status update",
                    "label_ids": ["INBOX"],
                    "headers": {
                        "From": "team@example.com",
                        "Subject": "Daily Status"
                    },
                    "body": "Daily status body"
                }
            ]
        }),
    );
    let missing_gog = temp.path().join("missing-gog");

    let search = native_gmail::run(
        temp.path(),
        &[
            "--json",
            "gmail",
            "search",
            "from:team@example.com",
            "--max",
            "10",
            "--format",
            "metadata",
            "--headers",
            "Subject,From",
        ],
        &[
            (
                "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(search.status.code(), Some(0));
    let search_payload = native_gmail::json(&search);
    assert_eq!(
        search_payload.get("command").and_then(Value::as_str),
        Some("google.gmail.search")
    );
    assert_eq!(
        search_payload
            .get("result")
            .and_then(|result| result.get("count"))
            .and_then(Value::as_u64),
        Some(1)
    );

    let get = native_gmail::run(
        temp.path(),
        &["--json", "gmail", "get", "msg-1", "--format", "full"],
        &[
            (
                "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(get.status.code(), Some(0));
    let get_payload = native_gmail::json(&get);
    assert_eq!(
        get_payload
            .get("result")
            .and_then(|result| result.get("message"))
            .and_then(|message| message.get("id"))
            .and_then(Value::as_str),
        Some("msg-1")
    );

    let thread_get = native_gmail::run(
        temp.path(),
        &[
            "--json",
            "gmail",
            "thread",
            "get",
            "thread-1",
            "--format",
            "metadata",
            "--headers",
            "Subject",
        ],
        &[
            (
                "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(thread_get.status.code(), Some(0));

    let send = native_gmail::run(
        temp.path(),
        &[
            "--json",
            "gmail",
            "send",
            "--to",
            "team@example.com",
            "--subject",
            "Status",
            "--body",
            "Native body",
            "--thread-id",
            "thread-1",
        ],
        &[
            (
                "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(send.status.code(), Some(0));
    let send_payload = native_gmail::json(&send);
    assert_eq!(
        send_payload.get("command").and_then(Value::as_str),
        Some("google.gmail.send")
    );
    assert!(
        send_payload
            .get("result")
            .and_then(|result| result.get("message"))
            .and_then(|message| message.get("mime_bytes"))
            .and_then(Value::as_u64)
            .unwrap_or_default()
            > 0
    );
}

#[test]
fn gmail_plain_contract_emits_human_text() {
    let temp = tempdir().expect("tempdir");
    native_gmail::seed_account(temp.path(), "me@example.com");

    let fixture_path = native_gmail::write_fixture(
        temp.path(),
        &json!({
            "messages": [
                {
                    "id": "msg-1",
                    "thread_id": "thread-1",
                    "snippet": "status update",
                    "label_ids": ["INBOX"],
                    "headers": {
                        "From": "team@example.com",
                        "Subject": "Daily Status"
                    },
                    "body": "Daily status body"
                }
            ]
        }),
    );

    let output = native_gmail::run(
        temp.path(),
        &[
            "--plain",
            "gmail",
            "search",
            "from:team@example.com",
            "--max",
            "5",
        ],
        &[(
            "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Found"));
}

#[test]
fn gmail_get_missing_message_maps_to_not_found_error() {
    let temp = tempdir().expect("tempdir");
    native_gmail::seed_account(temp.path(), "me@example.com");

    let fixture_path = native_gmail::write_fixture(temp.path(), &json!({ "messages": [] }));

    let output = native_gmail::run(
        temp.path(),
        &["--json", "gmail", "get", "missing-message"],
        &[(
            "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(1));

    let payload = native_gmail::json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_010")
    );
}
