mod common;

use serde_json::Value;

use common::TestHarness;

#[test]
fn gmail_subcommands_forward_expected_paths_and_args() {
    let cases: &[(&[&str], &[&str])] = &[
        (
            &["gmail", "search", "from:me", "--max", "10", "--page", "p1"],
            &["gmail", "search", "from:me", "--max", "10", "--page", "p1"],
        ),
        (
            &[
                "gmail",
                "get",
                "msg-123",
                "--format",
                "metadata",
                "--headers",
                "Subject,From",
            ],
            &[
                "gmail",
                "get",
                "msg-123",
                "--format",
                "metadata",
                "--headers",
                "Subject,From",
            ],
        ),
        (
            &[
                "gmail",
                "send",
                "--to",
                "team@example.com",
                "--subject",
                "Status",
                "--body",
                "Wrapped",
            ],
            &[
                "gmail",
                "send",
                "--to",
                "team@example.com",
                "--subject",
                "Status",
                "--body",
                "Wrapped",
            ],
        ),
        (
            &[
                "gmail",
                "thread",
                "get",
                "thread-123",
                "--format",
                "metadata",
            ],
            &[
                "gmail",
                "thread",
                "get",
                "thread-123",
                "--format",
                "metadata",
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
fn gmail_search_supports_json_mode_and_global_flags() {
    let harness = TestHarness::new();
    let output = harness.run(
        &[
            "--account",
            "me@example.com",
            "--json",
            "--results-only",
            "gmail",
            "search",
            "label:inbox",
            "--max",
            "5",
        ],
        &[("FAKE_GOG_STDOUT", r#"{"threads":[]}"#)],
    );
    assert_eq!(output.status.code(), Some(0));

    assert_eq!(
        harness.logged_args(),
        vec![
            "--account",
            "me@example.com",
            "--json",
            "--results-only",
            "gmail",
            "search",
            "label:inbox",
            "--max",
            "5",
        ]
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(
        json.get("command").and_then(Value::as_str),
        Some("google.gmail.search")
    );
}

#[test]
fn gmail_process_failure_maps_to_runtime_error() {
    let harness = TestHarness::new();
    let output = harness.run(
        &[
            "--json",
            "gmail",
            "send",
            "--to",
            "me@example.com",
            "--subject",
            "S",
            "--body",
            "B",
        ],
        &[("FAKE_GOG_EXIT_CODE", "9"), ("FAKE_GOG_STDERR", "boom")],
    );
    assert_eq!(output.status.code(), Some(1));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_003")
    );
}
