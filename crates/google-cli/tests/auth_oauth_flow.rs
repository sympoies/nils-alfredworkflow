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

fn seed_credentials(config_dir: &Path) {
    let output = run(
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
fn remote_step_two_rejects_state_mismatch_and_accepts_matching_state() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path());

    let step_one = run(
        temp.path(),
        &[
            "--json",
            "auth",
            "add",
            "me@example.com",
            "--remote",
            "--step",
            "1",
        ],
        &[],
    );
    assert_eq!(step_one.status.code(), Some(0));
    let step_one_payload = json(&step_one);
    let state = step_one_payload
        .get("result")
        .and_then(|value| value.get("state"))
        .and_then(Value::as_str)
        .expect("state")
        .to_string();

    let mismatch = run(
        temp.path(),
        &[
            "--json",
            "auth",
            "add",
            "me@example.com",
            "--remote",
            "--step",
            "2",
            "--state",
            "wrong-state",
            "--code",
            "abc",
        ],
        &[],
    );
    assert_eq!(mismatch.status.code(), Some(1));
    let mismatch_payload = json(&mismatch);
    assert_eq!(
        mismatch_payload
            .get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_008")
    );

    let step_two = run(
        temp.path(),
        &[
            "--json",
            "auth",
            "add",
            "me@example.com",
            "--remote",
            "--step",
            "2",
            "--state",
            &state,
            "--code",
            "abc",
        ],
        &[],
    );
    assert_eq!(step_two.status.code(), Some(0));
    let step_two_payload = json(&step_two);
    assert_eq!(
        step_two_payload
            .get("result")
            .and_then(|value| value.get("mode"))
            .and_then(Value::as_str),
        Some("remote")
    );
    assert_eq!(
        step_two_payload
            .get("result")
            .and_then(|value| value.get("step"))
            .and_then(Value::as_u64),
        Some(2)
    );
}

#[test]
fn loopback_mode_requires_test_callback_or_test_code() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path());

    let output = run(
        temp.path(),
        &["--json", "auth", "add", "me@example.com"],
        &[],
    );
    assert_eq!(output.status.code(), Some(2));
    let payload = json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_005")
    );
}
