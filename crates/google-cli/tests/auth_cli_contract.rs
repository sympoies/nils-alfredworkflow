mod common;

use serde_json::Value;

use common::TestHarness;

#[test]
fn auth_subcommands_forward_expected_paths_and_args() {
    let cases: &[(&[&str], &[&str])] = &[
        (
            &["auth", "credentials", "list"],
            &["auth", "credentials", "list"],
        ),
        (
            &["auth", "add", "me@example.com", "--manual", "--remote"],
            &["auth", "add", "me@example.com", "--manual", "--remote"],
        ),
        (&["auth", "list"], &["auth", "list"]),
        (&["auth", "status"], &["auth", "status"]),
        (
            &["auth", "remove", "me@example.com"],
            &["auth", "remove", "me@example.com"],
        ),
        (
            &["auth", "alias", "set", "work", "me@example.com"],
            &["auth", "alias", "set", "work", "me@example.com"],
        ),
        (
            &["auth", "manage", "--force-consent"],
            &["auth", "manage", "--force-consent"],
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
fn auth_forwards_global_flags_without_mutation() {
    let harness = TestHarness::new();
    let output = harness.run(
        &[
            "--account",
            "me@example.com",
            "--client",
            "default",
            "--json",
            "--select",
            "accounts",
            "auth",
            "add",
            "me@example.com",
            "--manual",
        ],
        &[("FAKE_GOG_STDOUT", "{}")],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        harness.logged_args(),
        vec![
            "--account",
            "me@example.com",
            "--client",
            "default",
            "--json",
            "--select",
            "accounts",
            "auth",
            "add",
            "me@example.com",
            "--manual",
        ]
    );
}

#[test]
fn auth_process_failure_maps_to_runtime_error_without_secret_leak() {
    let harness = TestHarness::new();
    let secret = "auth-secret-token";
    let output = harness.run(
        &["--json", "auth", "list"],
        &[
            ("FAKE_GOG_EXIT_CODE", "7"),
            ("FAKE_GOG_STDERR", "token=auth-secret-token\nboom"),
        ],
    );
    assert_eq!(output.status.code(), Some(1));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_003")
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains(secret));
    assert!(!stderr.contains(secret));
}

#[test]
fn auth_invalid_json_output_maps_to_runtime_error() {
    let harness = TestHarness::new();
    let output = harness.run(
        &["--json", "auth", "list"],
        &[("FAKE_GOG_STDOUT", "not-json")],
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
