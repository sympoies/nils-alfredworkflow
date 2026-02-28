mod common;

use serde_json::Value;

use common::TestHarness;

#[test]
fn service_json_success_envelope_has_required_keys() {
    let harness = TestHarness::new();
    let output = harness.run(
        &["--json", "auth", "list"],
        &[("FAKE_GOG_STDOUT", r#"{"accounts":["me@example.com"]}"#)],
    );
    assert_eq!(output.status.code(), Some(0));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(
        json.get("schema_version").and_then(Value::as_str),
        Some("v1")
    );
    assert_eq!(
        json.get("command").and_then(Value::as_str),
        Some("google.auth.list")
    );
    assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
    assert!(
        json.get("result")
            .and_then(|value| value.get("accounts"))
            .and_then(Value::as_array)
            .is_some()
    );
}

#[test]
fn output_mode_conflict_returns_machine_readable_user_error() {
    let harness = TestHarness::new();
    let output = harness.run(&["--json", "--plain", "auth", "list"], &[]);
    assert_eq!(output.status.code(), Some(2));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(
        json.get("command").and_then(Value::as_str),
        Some("google.auth.list")
    );
    assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_001")
    );
}

#[test]
fn missing_gog_returns_runtime_error_envelope() {
    let harness = TestHarness::new();
    let missing = harness.missing_gog_path();
    let output = harness.run(
        &["--json", "drive", "download", "file-id"],
        &[("GOOGLE_CLI_GOG_BIN", missing.as_str())],
    );
    assert_eq!(output.status.code(), Some(1));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(
        json.get("command").and_then(Value::as_str),
        Some("google.drive.download")
    );
    assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("NILS_GOOGLE_002")
    );
}
