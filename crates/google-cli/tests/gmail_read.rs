#[path = "common/native_gmail.rs"]
mod native_gmail;

use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn gmail_search_and_get_use_native_account_and_header_selection() {
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
                        "Subject": "Daily Status",
                        "X-Trace": "1"
                    },
                    "body": "Body 1"
                },
                {
                    "id": "msg-2",
                    "thread_id": "thread-2",
                    "snippet": "build failed",
                    "label_ids": ["INBOX", "IMPORTANT"],
                    "headers": {
                        "From": "ci@example.com",
                        "Subject": "Build Failed"
                    },
                    "body": "Body 2"
                }
            ]
        }),
    );

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
        &[(
            "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(search.status.code(), Some(0));

    let search_payload = native_gmail::json(&search);
    assert_eq!(
        search_payload
            .get("result")
            .and_then(|result| result.get("query"))
            .and_then(Value::as_str),
        Some("from:team@example.com")
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
        &[
            "--json",
            "gmail",
            "get",
            "msg-1",
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
    let headers = get_payload
        .get("result")
        .and_then(|result| result.get("message"))
        .and_then(|message| message.get("headers"))
        .and_then(Value::as_object)
        .expect("headers object");

    assert_eq!(headers.len(), 1);
    assert_eq!(
        headers.get("Subject").and_then(Value::as_str),
        Some("Daily Status")
    );
}
