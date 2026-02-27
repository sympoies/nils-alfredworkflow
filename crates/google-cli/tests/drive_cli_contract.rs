mod common;

use serde_json::Value;

use common::TestHarness;

#[test]
fn drive_subcommands_forward_expected_paths_and_args() {
    let cases: &[(&[&str], &[&str])] = &[
        (
            &[
                "drive",
                "ls",
                "--parent",
                "root",
                "--query",
                "mimeType='application/pdf'",
            ],
            &[
                "drive",
                "ls",
                "--parent",
                "root",
                "--query",
                "mimeType='application/pdf'",
            ],
        ),
        (
            &["drive", "search", "report", "--raw-query", "--max", "10"],
            &["drive", "search", "report", "--raw-query", "--max", "10"],
        ),
        (&["drive", "get", "file-123"], &["drive", "get", "file-123"]),
        (
            &[
                "drive",
                "download",
                "file-123",
                "--out",
                "/tmp/file.pdf",
                "--format",
                "pdf",
            ],
            &[
                "drive",
                "download",
                "file-123",
                "--out",
                "/tmp/file.pdf",
                "--format",
                "pdf",
            ],
        ),
        (
            &[
                "drive",
                "upload",
                "./report.pdf",
                "--parent",
                "folder-1",
                "--name",
                "report.pdf",
            ],
            &[
                "drive",
                "upload",
                "./report.pdf",
                "--parent",
                "folder-1",
                "--name",
                "report.pdf",
            ],
        ),
    ];

    for (args, expected) in cases {
        let harness = TestHarness::new();
        let output = harness.run(args, &[("FAKE_GOG_STDOUT", "{}")]);
        assert_eq!(output.status.code(), Some(0), "args: {args:?}");
        assert_eq!(
            harness.logged_args(),
            expected
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>(),
            "args: {args:?}"
        );
    }
}

#[test]
fn drive_supports_plain_mode_passthrough() {
    let harness = TestHarness::new();
    let output = harness.run(
        &["--plain", "drive", "ls", "--parent", "root"],
        &[("FAKE_GOG_STDOUT", "id\tname\n123\treport.pdf\n")],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "id\tname\n123\treport.pdf\n"
    );
    assert_eq!(
        harness.logged_args(),
        vec!["--plain", "drive", "ls", "--parent", "root"]
    );
}

#[test]
fn drive_invalid_json_output_maps_to_runtime_error() {
    let harness = TestHarness::new();
    let output = harness.run(
        &[
            "--json",
            "drive",
            "download",
            "file-123",
            "--out",
            "/tmp/file.pdf",
        ],
        &[("FAKE_GOG_STDOUT", "{broken-json")],
    );
    assert_eq!(output.status.code(), Some(1));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_004")
    );
}
