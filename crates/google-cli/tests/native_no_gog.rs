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
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("run google-cli")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be json")
}

#[test]
fn auth_commands_run_without_gog_binary() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("missing-gog");

    let output = run(
        temp.path(),
        &["--json", "auth", "list"],
        &[("GOOGLE_CLI_GOG_BIN", missing.to_string_lossy().as_ref())],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = json(&output);
    assert_eq!(payload.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.auth.list")
    );
}

#[test]
fn non_auth_commands_still_require_gog_binary() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("missing-gog");

    let output = run(
        temp.path(),
        &["--json", "drive", "ls"],
        &[("GOOGLE_CLI_GOG_BIN", missing.to_string_lossy().as_ref())],
    );
    assert_eq!(output.status.code(), Some(1));

    let payload = json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_002")
    );
}
