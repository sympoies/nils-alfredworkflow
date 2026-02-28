mod common;

#[path = "common/native_drive.rs"]
mod native_drive;

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
        &["--json", "drive", "ls", "--parent", "folder-1"],
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
        &["--json", "drive", "search", "report"],
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
        &["--json", "drive", "get", "file-1"],
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
            "--json",
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
        &["--plain", "drive", "search", "report"],
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
            "--json",
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
