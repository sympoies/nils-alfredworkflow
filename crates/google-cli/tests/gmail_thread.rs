#[path = "common/native_gmail.rs"]
mod native_gmail;

use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn gmail_thread_get_and_modify_cover_fetch_and_label_mutation() {
    let temp = tempdir().expect("tempdir");
    native_gmail::seed_account(temp.path(), "default@example.com");

    let fixture_path = native_gmail::write_fixture(
        temp.path(),
        &json!({
            "messages": [
                {
                    "id": "msg-1",
                    "thread_id": "thread-1",
                    "snippet": "daily status update",
                    "label_ids": ["INBOX"],
                    "headers": {
                        "From": "team@example.com",
                        "Subject": "Daily Status"
                    },
                    "body": "Body 1"
                },
                {
                    "id": "msg-2",
                    "thread_id": "thread-1",
                    "snippet": "follow-up",
                    "label_ids": ["INBOX", "UNREAD"],
                    "headers": {
                        "From": "team@example.com",
                        "Subject": "Re: Daily Status"
                    },
                    "body": "Body 2"
                }
            ]
        }),
    );

    let get = native_gmail::run(
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
        &[(
            "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(get.status.code(), Some(0));
    let get_payload = native_gmail::json(&get);
    assert_eq!(
        get_payload
            .get("result")
            .and_then(|result| result.get("thread"))
            .and_then(|thread| thread.get("message_count"))
            .and_then(Value::as_u64),
        Some(2)
    );

    let modify = native_gmail::run(
        temp.path(),
        &[
            "--json",
            "gmail",
            "thread",
            "modify",
            "thread-1",
            "--add-label",
            "STARRED",
            "--remove-label",
            "UNREAD",
        ],
        &[(
            "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(modify.status.code(), Some(0));

    let modify_payload = native_gmail::json(&modify);
    assert_eq!(
        modify_payload
            .get("result")
            .and_then(|result| result.get("thread"))
            .and_then(|thread| thread.get("modified_message_count"))
            .and_then(Value::as_u64),
        Some(2)
    );

    let missing = native_gmail::run(
        temp.path(),
        &["--json", "gmail", "thread", "get", "missing-thread"],
        &[(
            "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(missing.status.code(), Some(1));
    let missing_payload = native_gmail::json(&missing);
    assert_eq!(
        missing_payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_010")
    );
}
