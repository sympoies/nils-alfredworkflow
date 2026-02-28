#[path = "common/native_gmail.rs"]
mod native_gmail;

use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn gmail_commands_reuse_shared_account_resolution_and_error_when_ambiguous() {
    let temp = tempdir().expect("tempdir");
    native_gmail::seed_credentials(temp.path());

    let add_a = native_gmail::run(
        temp.path(),
        &[
            "--json",
            "auth",
            "add",
            "a@example.com",
            "--manual",
            "--code",
            "a-code",
        ],
        &[],
    );
    assert_eq!(add_a.status.code(), Some(0));

    let add_b = native_gmail::run(
        temp.path(),
        &[
            "--json",
            "auth",
            "add",
            "b@example.com",
            "--manual",
            "--code",
            "b-code",
        ],
        &[],
    );
    assert_eq!(add_b.status.code(), Some(0));

    let metadata_path = temp.path().join("accounts.v1.json");
    std::fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&json!({
            "version": 1,
            "default_account": null,
            "aliases": {},
            "accounts": ["a@example.com", "b@example.com"]
        }))
        .expect("serialize metadata"),
    )
    .expect("write metadata");

    let fixture_path = native_gmail::write_fixture(
        temp.path(),
        &json!({
            "messages": [
                {
                    "id": "msg-1",
                    "thread_id": "thread-1",
                    "snippet": "hello",
                    "label_ids": ["INBOX"],
                    "headers": {
                        "From": "team@example.com",
                        "Subject": "Hello"
                    },
                    "body": "Body"
                }
            ]
        }),
    );

    let output = native_gmail::run(
        temp.path(),
        &["--json", "gmail", "search", "hello"],
        &[(
            "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(2));

    let payload = native_gmail::json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_006")
    );
}

#[test]
fn gmail_commands_resolve_default_or_explicit_account() {
    let temp = tempdir().expect("tempdir");
    native_gmail::seed_credentials(temp.path());

    let add_a = native_gmail::run(
        temp.path(),
        &[
            "--json",
            "auth",
            "add",
            "default@example.com",
            "--manual",
            "--code",
            "a-code",
        ],
        &[],
    );
    assert_eq!(add_a.status.code(), Some(0));

    let fixture_path = native_gmail::write_fixture(
        temp.path(),
        &json!({
            "messages": [
                {
                    "id": "msg-1",
                    "thread_id": "thread-1",
                    "snippet": "hello",
                    "label_ids": ["INBOX"],
                    "headers": {
                        "From": "team@example.com",
                        "Subject": "Hello"
                    },
                    "body": "Body"
                }
            ]
        }),
    );

    let output = native_gmail::run(
        temp.path(),
        &["--json", "gmail", "search", "hello"],
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
            .and_then(|result| result.get("account"))
            .and_then(Value::as_str),
        Some("default@example.com")
    );
}
