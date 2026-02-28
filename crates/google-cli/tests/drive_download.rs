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
            "drive@example.com",
            "--manual",
            "--code",
            "manual-code",
        ],
        &[],
    );
    assert_eq!(add.status.code(), Some(0));
}

fn write_drive_fixture(config_dir: &Path) -> std::path::PathBuf {
    let fixture_path = config_dir.join("drive-fixture.json");
    std::fs::write(
        &fixture_path,
        serde_json::to_vec_pretty(&json!({
            "files": [
                {
                    "id": "file-123",
                    "name": "report.txt",
                    "mime_type": "text/plain",
                    "content": "download-content",
                    "export_formats": {
                        "pdf": "%PDF-1.7 fixture"
                    }
                }
            ]
        }))
        .expect("serialize fixture"),
    )
    .expect("write fixture");
    fixture_path
}

#[test]
fn drive_download_writes_destination_path() {
    let temp = tempdir().expect("tempdir");
    seed_auth(temp.path());
    let fixture = write_drive_fixture(temp.path());
    let out_path = temp.path().join("downloads").join("report.txt");

    let output = run(
        temp.path(),
        &[
            "--json",
            "drive",
            "download",
            "file-123",
            "--out",
            out_path.to_string_lossy().as_ref(),
        ],
        &[(
            "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
            fixture.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = json_output(&output);
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.drive.download")
    );
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("source"))
            .and_then(Value::as_str),
        Some("download")
    );
    assert_eq!(
        std::fs::read_to_string(&out_path).expect("read output"),
        "download-content"
    );
}

#[test]
fn drive_download_supports_export_format() {
    let temp = tempdir().expect("tempdir");
    seed_auth(temp.path());
    let fixture = write_drive_fixture(temp.path());
    let out_path = temp.path().join("exports").join("report.pdf");

    let output = run(
        temp.path(),
        &[
            "--json",
            "drive",
            "download",
            "file-123",
            "--format",
            "pdf",
            "--out",
            out_path.to_string_lossy().as_ref(),
        ],
        &[(
            "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
            fixture.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        std::fs::read_to_string(&out_path).expect("read output"),
        "%PDF-1.7 fixture"
    );

    let payload = json_output(&output);
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("source"))
            .and_then(Value::as_str),
        Some("export")
    );
}

#[test]
fn drive_download_rejects_existing_path_without_overwrite() {
    let temp = tempdir().expect("tempdir");
    seed_auth(temp.path());
    let fixture = write_drive_fixture(temp.path());
    let out_path = temp.path().join("report.txt");
    std::fs::write(&out_path, "existing").expect("seed existing");

    let output = run(
        temp.path(),
        &[
            "--json",
            "drive",
            "download",
            "file-123",
            "--out",
            out_path.to_string_lossy().as_ref(),
        ],
        &[(
            "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
            fixture.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(2));

    let payload = json_output(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_012")
    );
}

#[test]
fn drive_download_missing_file_maps_not_found() {
    let temp = tempdir().expect("tempdir");
    seed_auth(temp.path());
    let fixture = write_drive_fixture(temp.path());

    let output = run(
        temp.path(),
        &["--json", "drive", "download", "missing-file"],
        &[(
            "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
            fixture.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(1));

    let payload = json_output(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_013")
    );
}
