#[path = "common/native_drive.rs"]
mod native_drive;

use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn drive_json_contract_covers_ls_search_and_get() {
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
                },
                {
                    "id": "file-2",
                    "name": "notes.txt",
                    "mime_type": "text/plain",
                    "size_bytes": 100,
                    "parents": ["root"]
                }
            ]
        }),
    );
    let missing_gog = temp.path().join("missing-gog");

    let ls = native_drive::run(
        temp.path(),
        &[
            "--json", "drive", "ls", "--parent", "folder-1", "--max", "10",
        ],
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
    assert_eq!(
        ls_payload
            .get("result")
            .and_then(|result| result.get("count"))
            .and_then(Value::as_u64),
        Some(1)
    );

    let search = native_drive::run(
        temp.path(),
        &["--json", "drive", "search", "name:report", "--max", "10"],
        &[
            (
                "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
                fixture_path.to_string_lossy().as_ref(),
            ),
            ("GOOGLE_CLI_GOG_BIN", missing_gog.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(search.status.code(), Some(0));
    let search_payload = native_drive::json(&search);
    assert_eq!(
        search_payload
            .get("result")
            .and_then(|result| result.get("files"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );

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
    let get_payload = native_drive::json(&get);
    assert_eq!(
        get_payload
            .get("result")
            .and_then(|result| result.get("file"))
            .and_then(|file| file.get("id"))
            .and_then(Value::as_str),
        Some("file-1")
    );
}

#[test]
fn drive_get_missing_file_maps_to_not_found_error() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");

    let fixture_path = native_drive::write_fixture(temp.path(), &json!({ "files": [] }));
    let output = native_drive::run(
        temp.path(),
        &["--json", "drive", "get", "missing-file"],
        &[(
            "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(1));

    let payload = native_drive::json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_013")
    );
}

#[test]
fn drive_commands_reuse_shared_account_resolution_and_error_when_ambiguous() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_credentials(temp.path());

    let add_a = native_drive::run(
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

    let add_b = native_drive::run(
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
        &["--json", "drive", "ls"],
        &[(
            "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
            fixture_path.to_string_lossy().as_ref(),
        )],
    );
    assert_eq!(output.status.code(), Some(2));

    let payload = native_drive::json(&output);
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_006")
    );
}
