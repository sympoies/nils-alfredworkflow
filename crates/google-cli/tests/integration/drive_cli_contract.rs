use crate::native_drive;

use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn drive_json_contract_covers_ls_search_get_and_upload() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");

    let fixture_path = native_drive::write_fixture(
        temp.path(),
        &json!({
            "files": [
                {
                    "id": "file-1",
                    "name": "report.pdf",
                    "mime_type": "application/pdf",
                    "size_bytes": 2048,
                    "parents": ["folder-1"]
                }
            ]
        }),
    );
    let missing_gog = temp.path().join("missing-gog");

    let ls = native_drive::run(
        temp.path(),
        &["--output", "json", "drive", "ls", "--parent", "folder-1"],
        &[
            (
                "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(ls.status.code(), Some(0));
    let ls_payload = native_drive::json(&ls);
    assert_eq!(
        ls_payload.get("command").and_then(Value::as_str),
        Some("google.drive.ls")
    );

    let search = native_drive::run(
        temp.path(),
        &["--output", "json", "drive", "search", "report"],
        &[
            (
                "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(search.status.code(), Some(0));

    let get = native_drive::run(
        temp.path(),
        &["--output", "json", "drive", "get", "file-1"],
        &[
            (
                "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(get.status.code(), Some(0));

    let upload_source = temp.path().join("upload.txt");
    std::fs::write(&upload_source, b"hello").expect("write upload source");

    let upload = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "drive",
            "upload",
            upload_source.to_string_lossy().as_ref(),
            "--name",
            "upload.txt",
        ],
        &[
            (
                "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(upload.status.code(), Some(0));
    let upload_payload = native_drive::json(&upload);
    assert_eq!(
        upload_payload.get("command").and_then(Value::as_str),
        Some("google.drive.upload")
    );
}

#[test]
fn drive_plain_contract_emits_human_text() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");

    let fixture_path = native_drive::write_fixture(
        temp.path(),
        &json!({
            "files": [
                {
                    "id": "file-1",
                    "name": "report.pdf",
                    "mime_type": "application/pdf",
                    "size_bytes": 2048,
                    "parents": ["folder-1"]
                }
            ]
        }),
    );

    let output = native_drive::run(
        temp.path(),
        &["--output", "plain", "drive", "search", "report"],
        &[(
            "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Found"));
}

#[test]
fn drive_mkdir_and_trash_are_native_write_commands() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");
    let fixture_path = native_drive::write_fixture(
        temp.path(),
        &json!({"files": [
            {"id": "file-1", "name": "old.txt", "mime_type": "text/plain", "parents": ["folder-1"]}
        ]}),
    );
    let fixture = fixture_path.to_string_lossy();
    let mkdir = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "-a",
            "me@example.com",
            "drive",
            "mkdir",
            "new",
            "--parent",
            "folder-1",
        ],
        &[("GOOGLE_CLI_DRIVE_FIXTURE_PATH", fixture.as_ref())],
    );
    assert_eq!(mkdir.status.code(), Some(0));
    let created = native_drive::json(&mkdir);
    assert_eq!(created["command"], "google.drive.mkdir");
    assert_eq!(created["result"]["file"]["parents"][0], "folder-1");

    let trash = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "-a",
            "me@example.com",
            "drive",
            "trash",
            "file-1",
        ],
        &[("GOOGLE_CLI_DRIVE_FIXTURE_PATH", fixture.as_ref())],
    );
    assert_eq!(trash.status.code(), Some(0));
    let deleted = native_drive::json(&trash);
    assert_eq!(deleted["command"], "google.drive.trash");
    assert_eq!(deleted["result"]["file"]["trashed"], true);
}

#[test]
fn drive_write_commands_return_reconcilable_file_metadata() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");
    let fixture_path = native_drive::write_fixture(
        temp.path(),
        &json!({"files": [
            {"id": "file-1", "name": "old.txt", "mime_type": "text/plain", "size_bytes": 3,
             "parents": ["folder-1"], "content": "old", "version": "8", "trashed": false}
        ]}),
    );
    let fixture = fixture_path.to_string_lossy();
    let source = temp.path().join("new.txt");
    std::fs::write(&source, b"new bytes").expect("update source");
    let cases = [
        (
            vec!["rename", "file-1", "--name", "new.txt"],
            "rename",
            "name",
            "new.txt",
        ),
        (
            vec![
                "move", "file-1", "--parent", "folder-2", "--from", "folder-1",
            ],
            "move",
            "parents",
            "folder-2",
        ),
        (
            vec![
                "copy", "file-1", "--parent", "folder-2", "--name", "copy.txt",
            ],
            "copy",
            "name",
            "copy.txt",
        ),
        (vec!["untrash", "file-1"], "untrash", "trashed", "false"),
    ];
    for (args, command, field, expected) in cases {
        let mut command_args = vec!["--output", "json", "-a", "me@example.com", "drive"];
        command_args.extend(args);
        let output = native_drive::run(
            temp.path(),
            &command_args,
            &[("GOOGLE_CLI_DRIVE_FIXTURE_PATH", fixture.as_ref())],
        );
        assert_eq!(output.status.code(), Some(0), "{command}");
        let payload = native_drive::json(&output);
        assert_eq!(payload["command"], format!("google.drive.{command}"));
        let value = if field == "parents" {
            &payload["result"]["file"][field][0]
        } else {
            &payload["result"]["file"][field]
        };
        assert_eq!(value.to_string().trim_matches('"'), expected);
        assert!(payload["result"]["file"]["version"].is_string());
    }

    let update = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "-a",
            "me@example.com",
            "drive",
            "update",
            "file-1",
            source.to_string_lossy().as_ref(),
            "--mime",
            "text/plain",
        ],
        &[("GOOGLE_CLI_DRIVE_FIXTURE_PATH", fixture.as_ref())],
    );
    assert_eq!(update.status.code(), Some(0));
    let payload = native_drive::json(&update);
    assert_eq!(payload["command"], "google.drive.update");
    assert_eq!(payload["result"]["file"]["size_bytes"], 9);
    assert_eq!(
        payload["result"]["file"]["sha256_checksum"]
            .as_str()
            .map(str::len),
        Some(64)
    );
}

#[test]
fn drive_write_rejects_implicit_account_and_invalid_move_parent() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");
    let fixture_path = native_drive::write_fixture(
        temp.path(),
        &json!({"files": [
            {"id": "file-1", "name": "old.txt", "mime_type": "text/plain", "parents": ["folder-1"]}
        ]}),
    );
    let fixture = fixture_path.to_string_lossy();
    let no_account = native_drive::run(
        temp.path(),
        &["--output", "json", "drive", "trash", "file-1"],
        &[("GOOGLE_CLI_DRIVE_FIXTURE_PATH", fixture.as_ref())],
    );
    assert_eq!(no_account.status.code(), Some(2));
    assert_eq!(
        native_drive::json(&no_account)["error"]["code"],
        "NILS_GOOGLE_012"
    );

    let wrong_from = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "-a",
            "me@example.com",
            "drive",
            "move",
            "file-1",
            "--parent",
            "folder-2",
            "--from",
            "unrelated",
        ],
        &[("GOOGLE_CLI_DRIVE_FIXTURE_PATH", fixture.as_ref())],
    );
    assert_eq!(wrong_from.status.code(), Some(2));
    assert_eq!(
        native_drive::json(&wrong_from)["error"]["code"],
        "NILS_GOOGLE_012"
    );
}

#[test]
fn drive_write_accepts_hyphen_prefixed_names() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");
    let fixture_path = native_drive::write_fixture(
        temp.path(),
        &json!({"files": [
            {"id": "file-1", "name": "old.txt", "mime_type": "text/plain", "parents": ["folder-1"]}
        ]}),
    );
    let fixture = fixture_path.to_string_lossy();
    for args in [
        vec!["mkdir", "-folder", "--parent", "folder-1"],
        vec!["rename", "file-1", "--name", "-report"],
        vec!["copy", "file-1", "--parent", "folder-1", "--name", "-copy"],
    ] {
        let mut command = vec!["--output", "json", "-a", "me@example.com", "drive"];
        command.extend(args);
        let output = native_drive::run(
            temp.path(),
            &command,
            &[("GOOGLE_CLI_DRIVE_FIXTURE_PATH", fixture.as_ref())],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            native_drive::json(&output)["result"]["file"]["name"]
                .as_str()
                .is_some_and(|name| name.starts_with('-'))
        );
    }
}

#[test]
fn drive_download_executes_natively_and_writes_output() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "drive@example.com");

    let fixture_path = native_drive::write_fixture(
        temp.path(),
        &json!({
            "files": [
                {
                    "id": "file-123",
                    "name": "fixture.txt",
                    "mime_type": "text/plain",
                    "size_bytes": 14,
                    "parents": ["root"],
                    "content": "fixture-content",
                    "export_formats": {
                        "pdf": "%PDF fixture"
                    }
                }
            ]
        }),
    );

    let output_path = temp.path().join("out.pdf");
    let missing_gog = temp.path().join("missing-gog");

    let output = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "drive",
            "download",
            "file-123",
            "--out",
            output_path.to_string_lossy().as_ref(),
            "--format",
            "pdf",
        ],
        &[
            (
                "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );

    assert_eq!(output.status.code(), Some(0));
    let payload = native_drive::json(&output);
    assert_eq!(
        payload.get("command").and_then(Value::as_str),
        Some("google.drive.download")
    );
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("source"))
            .and_then(Value::as_str),
        Some("export")
    );
    assert_eq!(
        std::fs::read_to_string(&output_path).expect("downloaded file"),
        "%PDF fixture"
    );
}
