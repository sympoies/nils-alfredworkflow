#[path = "common/native_gmail.rs"]
mod native_gmail;

use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn gmail_send_builds_mime_with_attachment_and_thread_metadata() {
    let temp = tempdir().expect("tempdir");
    native_gmail::seed_account(temp.path(), "default@example.com");

    let fixture_path = native_gmail::write_fixture(temp.path(), &json!({ "messages": [] }));
    let attachment_path = temp.path().join("report.txt");
    std::fs::write(&attachment_path, "report-body").expect("write attachment");

    let output = native_gmail::run(
        temp.path(),
        &[
            "--json",
            "gmail",
            "send",
            "--to",
            "team@example.com",
            "--subject",
            "Sprint Update",
            "--body",
            "Native Gmail send body",
            "--thread-id",
            "thread-123",
            "--attachment",
            attachment_path.to_string_lossy().as_ref(),
        ],
        &[(
            "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );

    assert_eq!(output.status.code(), Some(0));
    let payload = native_gmail::json(&output);

    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("message"))
            .and_then(|message| message.get("thread_id"))
            .and_then(Value::as_str),
        Some("thread-123")
    );

    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("message"))
            .and_then(|message| message.get("attachment_count"))
            .and_then(Value::as_u64),
        Some(1)
    );

    let preview = payload
        .get("result")
        .and_then(|result| result.get("message"))
        .and_then(|message| message.get("mime_preview"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    assert!(preview.contains("Subject: Sprint Update"));
}
