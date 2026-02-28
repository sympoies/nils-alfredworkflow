use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use tempfile::tempdir;

fn run_with_env(config_dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_google-cli"));
    command.args(args);
    command.env("GOOGLE_CLI_CONFIG_DIR", config_dir);
    command.env("GOOGLE_CLI_KEYRING_MODE", "file");
    command.env("GOOGLE_CLI_AUTH_DISABLE_BROWSER", "1");
    command.env("PATH", config_dir);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("run google-cli")
}

fn output_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be json")
}

fn seed_credentials(config_dir: &Path) {
    let output = run_with_env(
        config_dir,
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
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn auth_manage_returns_summary_only_contract() {
    let temp = tempdir().expect("tempdir");

    let output = run_with_env(temp.path(), &["--json", "auth", "manage"], &[]);
    assert_eq!(output.status.code(), Some(0));

    let payload = output_json(&output);
    assert_eq!(payload.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        payload
            .get("result")
            .and_then(|value| value.get("behavior"))
            .and_then(Value::as_str),
        Some("summary-only")
    );
}

#[test]
fn auth_add_requires_credentials_first() {
    let temp = tempdir().expect("tempdir");

    let output = run_with_env(
        temp.path(),
        &[
            "--json",
            "auth",
            "add",
            "me@example.com",
            "--manual",
            "--code",
            "abc",
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(2));

    let payload = output_json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_005")
    );
}

#[test]
fn auth_add_reports_runtime_store_failure_when_keyring_mode_is_fail() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path());

    let output = run_with_env(
        temp.path(),
        &[
            "--json",
            "auth",
            "add",
            "me@example.com",
            "--manual",
            "--code",
            "abc",
        ],
        &[("GOOGLE_CLI_KEYRING_MODE", "fail")],
    );
    assert_eq!(output.status.code(), Some(1));

    let payload = output_json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_007")
    );
}

#[test]
fn auth_status_without_default_returns_ambiguous_error() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path());

    let metadata = json!({
        "version": 1,
        "default_account": null,
        "aliases": {},
        "accounts": ["a@example.com", "b@example.com"]
    });
    let metadata_path = temp.path().join("accounts.v1.json");
    std::fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&metadata).expect("serialize metadata"),
    )
    .expect("write metadata");

    let output = run_with_env(temp.path(), &["--json", "auth", "status"], &[]);
    assert_eq!(output.status.code(), Some(2));

    let payload = output_json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_006")
    );
}
