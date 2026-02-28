use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
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

fn json_output(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be json")
}

fn seed_auth(config_dir: &Path) {
    let set = run(
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
    assert_eq!(set.status.code(), Some(0));

    let add = run(
        config_dir,
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
    assert_eq!(add.status.code(), Some(0));
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

    let payload = json_output(&output);
    assert_eq!(payload.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.auth.list")
    );
}

#[test]
fn gmail_commands_run_without_gog_binary() {
    let temp = tempdir().expect("tempdir");
    seed_auth(temp.path());

    let fixture_path = temp.path().join("gmail-fixture.json");
    std::fs::write(
        &fixture_path,
        serde_json::to_vec_pretty(&json!({
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
                    "body": "body"
                }
            ]
        }))
        .expect("serialize fixture"),
    )
    .expect("write fixture");

    let missing = temp.path().join("missing-gog");
    let output = run(
        temp.path(),
        &["--json", "gmail", "search", "hello"],
        &[
            ("GOOGLE_CLI_GOG_BIN", missing.to_string_lossy().as_ref()),
            (
                "GOOGLE_CLI_GMAIL_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
        ],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = json_output(&output);
    assert_eq!(payload.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.gmail.search")
    );
}

#[test]
fn drive_ls_runs_without_gog_binary() {
    let temp = tempdir().expect("tempdir");
    seed_auth(temp.path());

    let fixture_path = temp.path().join("drive-fixture.json");
    std::fs::write(
        &fixture_path,
        serde_json::to_vec_pretty(&json!({
            "files": [
                {
                    "id": "file-1",
                    "name": "report.pdf",
                    "mime_type": "application/pdf",
                    "size_bytes": 2048,
                    "parents": ["folder-1"]
                }
            ]
        }))
        .expect("serialize fixture"),
    )
    .expect("write fixture");

    let missing = temp.path().join("missing-gog");

    let output = run(
        temp.path(),
        &["--json", "drive", "ls", "--parent", "folder-1"],
        &[
            ("GOOGLE_CLI_GOG_BIN", missing.to_string_lossy().as_ref()),
            (
                "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
        ],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = json_output(&output);
    assert_eq!(payload.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.drive.ls")
    );
}

#[test]
fn drive_download_still_requires_gog_binary() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("missing-gog");

    let output = run(
        temp.path(),
        &[
            "--json",
            "drive",
            "download",
            "file-123",
            "--out",
            "/tmp/out.bin",
        ],
        &[("GOOGLE_CLI_GOG_BIN", missing.to_string_lossy().as_ref())],
    );
    assert_eq!(output.status.code(), Some(1));

    let payload = json_output(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_002")
    );
}
