use crate::native_drive;

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
            "--output", "json", "drive", "ls", "--parent", "folder-1", "--max", "10",
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
        &[
            "--output",
            "json",
            "drive",
            "search",
            "name:report",
            "--max",
            "10",
        ],
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
fn drive_search_exposes_a_continuation_for_account_wide_reads() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");
    let fixture_path = native_drive::write_fixture(
        temp.path(),
        &json!({"files": [
            {"id": "a", "name": "Alpha", "mime_type": "text/plain"},
            {"id": "b", "name": "Beta", "mime_type": "text/plain"},
            {"id": "c", "name": "Charlie", "mime_type": "text/plain"}
        ]}),
    );
    let envs = [(
        "GOOGLE_CLI_DRIVE_FIXTURE_PATH",
        fixture_path.to_string_lossy().to_string(),
    )];
    let borrowed = [(envs[0].0, envs[0].1.as_str())];
    let first = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "drive",
            "search",
            "--query",
            "a",
            "--all-drives",
            "--max",
            "2",
        ],
        &borrowed,
    );
    assert_eq!(first.status.code(), Some(0));
    let first_payload = native_drive::json(&first);
    let first_result = &first_payload["result"];
    assert_eq!(first_result["count"], 2);
    assert_eq!(first_result["next_page_token"], "2");
    assert_eq!(first_result["all_drives"], true);

    let second = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "drive",
            "search",
            "--query",
            "a",
            "--all-drives",
            "--max",
            "2",
            "--page",
            "2",
        ],
        &borrowed,
    );
    assert_eq!(second.status.code(), Some(0));
    let second_result = native_drive::json(&second)["result"].clone();
    assert_eq!(second_result["count"], 1);
    assert!(second_result["next_page_token"].is_null());

    let ls_first = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "drive",
            "ls",
            "--all-drives",
            "--max",
            "2",
        ],
        &borrowed,
    );
    assert_eq!(ls_first.status.code(), Some(0));
    let ls_first_result = native_drive::json(&ls_first)["result"].clone();
    assert_eq!(ls_first_result["all_drives"], true);
    assert_eq!(ls_first_result["next_page_token"], "2");
    let ls_second = native_drive::run(
        temp.path(),
        &[
            "--output",
            "json",
            "drive",
            "ls",
            "--all-drives",
            "--max",
            "2",
            "--page",
            "2",
        ],
        &borrowed,
    );
    assert_eq!(ls_second.status.code(), Some(0));
    let ls_second_result = native_drive::json(&ls_second)["result"].clone();
    assert_eq!(ls_second_result["count"], 1);
    assert!(ls_second_result["next_page_token"].is_null());

    let invalid = native_drive::run(
        temp.path(),
        &["--output", "json", "drive", "search", "a", "--max", "0"],
        &borrowed,
    );
    assert_eq!(invalid.status.code(), Some(2));

    for action in ["ls", "search"] {
        let args = if action == "ls" {
            vec!["--output", "json", "drive", "ls", "--max", "1001"]
        } else {
            vec!["--output", "json", "drive", "search", "a", "--max", "1001"]
        };
        let above_provider_page_size = native_drive::run(temp.path(), &args, &borrowed);
        assert_eq!(above_provider_page_size.status.code(), Some(0));
    }
}

#[test]
fn drive_get_missing_file_maps_to_not_found_error() {
    let temp = tempdir().expect("tempdir");
    native_drive::seed_account(temp.path(), "me@example.com");

    let fixture_path = native_drive::write_fixture(temp.path(), &json!({ "files": [] }));
    let output = native_drive::run(
        temp.path(),
        &["--output", "json", "drive", "get", "missing-file"],
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
            "--output",
            "json",
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
            "--output",
            "json",
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
        &["--output", "json", "drive", "ls"],
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
