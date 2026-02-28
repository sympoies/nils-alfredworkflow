use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::tempdir;

fn run(config_dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_google-cli"));
    command.args(args);
    command.env("GOOGLE_CLI_CONFIG_DIR", config_dir);
    command.env("GOOGLE_CLI_KEYRING_MODE", "file");
    command.env("GOOGLE_CLI_AUTH_DISABLE_BROWSER", "1");
    command.env("GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE", "1");
    command.env("PATH", config_dir);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("run google-cli")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be json")
}

#[test]
fn credentials_set_and_list_roundtrip() {
    let temp = tempdir().expect("tempdir");

    let set_output = run(
        temp.path(),
        &[
            "--json",
            "auth",
            "credentials",
            "set",
            "--client-id",
            "client-id",
            "--client-secret",
            "client-secret",
        ],
        &[],
    );
    assert_eq!(set_output.status.code(), Some(0));

    let list_output = run(temp.path(), &["--json", "auth", "credentials", "list"], &[]);
    assert_eq!(list_output.status.code(), Some(0));
    let payload = json(&list_output);

    assert_eq!(
        payload
            .get("result")
            .and_then(|value| value.get("configured"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        payload
            .get("result")
            .and_then(|value| value.get("client_id"))
            .and_then(Value::as_str),
        Some("client-id")
    );

    assert!(temp.path().join("credentials.v1.json").exists());
}

#[test]
fn manual_add_persists_token_metadata_and_status() {
    let temp = tempdir().expect("tempdir");

    let _ = run(
        temp.path(),
        &[
            "--json",
            "auth",
            "credentials",
            "set",
            "--client-id",
            "client-id",
            "--client-secret",
            "client-secret",
        ],
        &[],
    );

    let add_output = run(
        temp.path(),
        &[
            "--json",
            "auth",
            "add",
            "me@example.com",
            "--manual",
            "--code",
            "manual-code",
        ],
        &[],
    );
    assert_eq!(add_output.status.code(), Some(0));

    let list_output = run(temp.path(), &["--json", "auth", "list"], &[]);
    assert_eq!(list_output.status.code(), Some(0));
    let list_payload = json(&list_output);
    let accounts = list_payload
        .get("result")
        .and_then(|value| value.get("accounts"))
        .and_then(Value::as_array)
        .expect("accounts array");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].as_str(), Some("me@example.com"));

    let status_output = run(temp.path(), &["--json", "auth", "status"], &[]);
    assert_eq!(status_output.status.code(), Some(0));
    let status_payload = json(&status_output);
    assert_eq!(
        status_payload
            .get("result")
            .and_then(|value| value.get("account"))
            .and_then(Value::as_str),
        Some("me@example.com")
    );
    assert_eq!(
        status_payload
            .get("result")
            .and_then(|value| value.get("has_token"))
            .and_then(Value::as_bool),
        Some(true)
    );

    assert!(temp.path().join("accounts.v1.json").exists());
    assert!(temp.path().join("tokens.v1.json").exists());
}
