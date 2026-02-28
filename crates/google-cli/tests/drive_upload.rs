#[path = "common/native_drive.rs"]
mod native_drive;

use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn drive_upload_infers_mime_and_supports_replace_behavior() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");

    let fixture_path = native_drive::write_fixture(
        temp.path(),
        &json!({
            "files": [
                {
                    "id": "existing-file-id",
                    "name": "report.pdf",
                    "mime_type": "application/pdf",
                    "size_bytes": 1024,
                    "parents": ["folder-1"]
                }
            ]
        }),
    );

    let upload_source = temp.path().join("report.pdf");
    std::fs::write(&upload_source, b"pdf bytes").expect("write upload source");

    let output = native_drive::run(
        temp.path(),
        &[
            "--json",
            "drive",
            "upload",
            upload_source.to_string_lossy().as_ref(),
            "--parent",
            "folder-1",
            "--replace",
        ],
        &[(
            "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = native_drive::json(&output);
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("upload"))
            .and_then(|upload| upload.get("replaced"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("upload"))
            .and_then(|upload| upload.get("replaced_file_id"))
            .and_then(Value::as_str),
        Some("existing-file-id")
    );
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("upload"))
            .and_then(|upload| upload.get("inferred_mime_type"))
            .and_then(Value::as_str),
        Some("application/pdf")
    );
}

#[test]
fn drive_upload_allows_explicit_mime_override() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");

    let fixture_path = native_drive::write_fixture(temp.path(), &json!({ "files": [] }));

    let upload_source = temp.path().join("payload.bin");
    std::fs::write(&upload_source, b"raw bytes").expect("write upload source");

    let output = native_drive::run(
        temp.path(),
        &[
            "--json",
            "drive",
            "upload",
            upload_source.to_string_lossy().as_ref(),
            "--mime",
            "text/plain",
            "--name",
            "payload.txt",
            "--convert",
        ],
        &[(
            "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(0));

    let payload = native_drive::json(&output);
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("upload"))
            .and_then(|upload| upload.get("file"))
            .and_then(|file| file.get("name"))
            .and_then(Value::as_str),
        Some("payload.txt")
    );
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("upload"))
            .and_then(|upload| upload.get("inferred_mime_type"))
            .and_then(Value::as_str),
        Some("text/plain")
    );
    assert_eq!(
        payload
            .get("result")
            .and_then(|result| result.get("upload"))
            .and_then(|upload| upload.get("convert_requested"))
            .and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn drive_upload_missing_source_maps_to_drive_input_error() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");

    let missing_path = temp.path().join("missing.bin");
    let output = native_drive::run(
        temp.path(),
        &[
            "--json",
            "drive",
            "upload",
            missing_path.to_string_lossy().as_ref(),
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(2));

    let payload = native_drive::json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_012")
    );
}
