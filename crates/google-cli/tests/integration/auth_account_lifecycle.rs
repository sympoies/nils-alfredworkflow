use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::Value;
use tempfile::tempdir;

fn command(config_dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(resolve_cli_path());
    command.args(args);
    command.env("GOOGLE_CLI_CONFIG_DIR", config_dir);
    command.env("GOOGLE_CLI_KEYRING_MODE", "file");
    command.env("GOOGLE_CLI_AUTH_DISABLE_BROWSER", "1");
    command.env("GOOGLE_CLI_AUTH_ALLOW_FAKE_EXCHANGE", "1");
    command.env("PATH", config_dir);
    command
}

fn run(config_dir: &Path, args: &[&str]) -> Output {
    command(config_dir, args).output().expect("run google-cli")
}

fn run_with_stdin(config_dir: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = command(config_dir, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn google-cli");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait google-cli")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be json")
}

fn result<'a>(payload: &'a Value, key: &str) -> &'a Value {
    payload
        .get("result")
        .and_then(|value| value.get(key))
        .unwrap_or(&Value::Null)
}

fn error_code(output: &Output) -> Option<String> {
    json(output)
        .get("error")
        .and_then(|value| value.get("code"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn seed_credentials(config_dir: &Path, extra: &[&str]) {
    let mut args = vec![
        "--output",
        "json",
        "auth",
        "credentials",
        "set",
        "--client-id",
        "client-id",
        "--client-secret",
        "client-secret",
    ];
    args.extend_from_slice(extra);
    assert_eq!(run(config_dir, &args).status.code(), Some(0));
}

fn seed_account(config_dir: &Path, account: &str) {
    let output = run(
        config_dir,
        &[
            "--output", "json", "auth", "add", account, "--manual", "--code", "abc",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
}

fn list(config_dir: &Path) -> Value {
    json(&run(config_dir, &["--output", "json", "auth", "list"]))
}

fn remote_step_one(config_dir: &Path, account: &str) -> String {
    let output = run(
        config_dir,
        &[
            "--output", "json", "auth", "add", account, "--remote", "--step", "1",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    result(&json(&output), "state")
        .as_str()
        .expect("state")
        .to_string()
}

#[test]
fn default_sets_the_default_account_and_refuses_an_unknown_one() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path(), &[]);
    seed_account(temp.path(), "first@example.com");
    seed_account(temp.path(), "second@example.com");
    assert_eq!(
        result(&list(temp.path()), "default_account").as_str(),
        Some("first@example.com")
    );

    let output = run(
        temp.path(),
        &["--output", "json", "auth", "default", "second@example.com"],
    );
    assert_eq!(output.status.code(), Some(0));
    let payload = json(&output);
    assert_eq!(
        result(&payload, "default_account").as_str(),
        Some("second@example.com")
    );
    assert_eq!(
        result(&payload, "previous_default").as_str(),
        Some("first@example.com")
    );
    assert_eq!(
        result(&list(temp.path()), "default_account").as_str(),
        Some("second@example.com")
    );

    let unknown = run(
        temp.path(),
        &["--output", "json", "auth", "default", "nobody@example.com"],
    );
    assert_eq!(error_code(&unknown).as_deref(), Some("NILS_GOOGLE_005"));
    assert_eq!(
        result(&list(temp.path()), "default_account").as_str(),
        Some("second@example.com")
    );
}

#[test]
fn remote_step_two_reads_the_callback_url_from_stdin() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path(), &[]);
    let state = remote_step_one(temp.path(), "me@example.com");
    let step_two = [
        "--output",
        "json",
        "auth",
        "add",
        "me@example.com",
        "--remote",
        "--step",
        "2",
        "--callback-url-stdin",
    ];

    let mismatch = run_with_stdin(
        temp.path(),
        &step_two,
        "http://localhost/?state=wrong-state&code=4%2F0Aabc\n",
    );
    assert_eq!(error_code(&mismatch).as_deref(), Some("NILS_GOOGLE_008"));
    // The code is credential material and must never be echoed back.
    let echoed = String::from_utf8_lossy(&mismatch.stdout).to_string()
        + &String::from_utf8_lossy(&mismatch.stderr);
    assert!(!echoed.contains("0Aabc"));

    let mixed = run_with_stdin(
        temp.path(),
        &[&step_two[..], &["--code", "abc"]].concat(),
        "http://localhost/?state=x&code=y\n",
    );
    assert_eq!(error_code(&mixed).as_deref(), Some("NILS_GOOGLE_005"));

    let accepted = run_with_stdin(
        temp.path(),
        &step_two,
        &format!("  http://localhost/?state={state}&code=4%2F0Aabc&scope=email  \n"),
    );
    assert_eq!(accepted.status.code(), Some(0));
    let payload = json(&accepted);
    assert_eq!(result(&payload, "stored").as_bool(), Some(true));
    assert_eq!(
        result(&list(temp.path()), "accounts"),
        &serde_json::json!(["me@example.com"])
    );
}

#[test]
fn remote_state_is_unpredictable_across_runs() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path(), &[]);
    let first = remote_step_one(temp.path(), "me@example.com");
    let second = remote_step_one(temp.path(), "me@example.com");
    assert_ne!(first, second);
    assert!(first.len() >= 32);
}

/// Serve one HTTP request, reply with `status` and `body`, and report the
/// request line and body it received.
fn one_shot_server(status: u16, body: &'static str) -> (String, mpsc::Receiver<(String, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut request_line = String::new();
        reader.read_line(&mut request_line).expect("request line");
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("header");
            if line == "\r\n" || line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().expect("length");
            }
        }
        let mut request_body = vec![0u8; length];
        reader.read_exact(&mut request_body).expect("body");
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("reply");
        sender
            .send((
                request_line.trim().to_string(),
                String::from_utf8_lossy(&request_body).to_string(),
            ))
            .expect("report");
    });
    (format!("http://{address}/revoke"), receiver)
}

fn seeded_with_revoke_endpoint(revoke_uri: &str) -> tempfile::TempDir {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path(), &["--revoke-uri", revoke_uri]);
    seed_account(temp.path(), "keep@example.com");
    seed_account(temp.path(), "gone@example.com");
    temp
}

#[test]
fn remove_with_revoke_revokes_the_refresh_token_before_forgetting_it() {
    let (uri, requests) = one_shot_server(200, "{}");
    let temp = seeded_with_revoke_endpoint(&uri);

    let output = run(
        temp.path(),
        &[
            "--output",
            "json",
            "auth",
            "remove",
            "gone@example.com",
            "--revoke",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let payload = json(&output);
    assert_eq!(result(&payload, "revoked").as_str(), Some("revoked"));
    assert_eq!(result(&payload, "removed_token").as_bool(), Some(true));

    let (request_line, body) = requests
        .recv_timeout(Duration::from_secs(10))
        .expect("revoke request");
    assert!(request_line.starts_with("POST /revoke"));
    assert!(body.starts_with("token=refresh-"));
    assert_eq!(
        result(&list(temp.path()), "accounts"),
        &serde_json::json!(["keep@example.com"])
    );
}

#[test]
fn remove_with_revoke_treats_an_already_invalid_token_as_revoked() {
    let (uri, _requests) = one_shot_server(400, r#"{"error":"invalid_token"}"#);
    let temp = seeded_with_revoke_endpoint(&uri);

    let output = run(
        temp.path(),
        &[
            "--output",
            "json",
            "auth",
            "remove",
            "gone@example.com",
            "--revoke",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        result(&json(&output), "revoked").as_str(),
        Some("already-invalid")
    );
    assert_eq!(
        result(&list(temp.path()), "accounts"),
        &serde_json::json!(["keep@example.com"])
    );
}

#[test]
fn remove_with_revoke_keeps_the_account_when_revocation_fails() {
    let (uri, _requests) = one_shot_server(503, r#"{"error":"backend_error"}"#);
    let temp = seeded_with_revoke_endpoint(&uri);

    let output = run(
        temp.path(),
        &[
            "--output",
            "json",
            "auth",
            "remove",
            "gone@example.com",
            "--revoke",
        ],
    );
    assert_eq!(error_code(&output).as_deref(), Some("NILS_GOOGLE_007"));
    // Forgetting a token Google still honours would leave a live grant nothing
    // tracks, so the local record stays until revocation succeeds.
    assert_eq!(
        result(&list(temp.path()), "accounts"),
        &serde_json::json!(["gone@example.com", "keep@example.com"])
    );
}

fn resolve_cli_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_google-cli") {
        return PathBuf::from(path);
    }

    if let Ok(current_exe) = std::env::current_exe()
        && let Some(debug_dir) = current_exe.parent().and_then(|deps| deps.parent())
    {
        let candidate = debug_dir.join(format!("google-cli{}", std::env::consts::EXE_SUFFIX));
        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from(env!("CARGO_BIN_EXE_google-cli"))
}

/// A revoke endpoint that must never be contacted: the returned check fails if
/// anything connected to it.
fn untouchable_endpoint() -> (String, impl FnOnce()) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let uri = format!("http://{}/revoke", listener.local_addr().expect("address"));
    let check = move || {
        listener.set_nonblocking(true).expect("nonblocking");
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
            "the revoke endpoint was contacted"
        );
    };
    (uri, check)
}

#[test]
fn plain_remove_stays_local_and_never_contacts_the_revoke_endpoint() {
    let (uri, never_contacted) = untouchable_endpoint();
    let temp = seeded_with_revoke_endpoint(&uri);

    let output = run(
        temp.path(),
        &["--output", "json", "auth", "remove", "gone@example.com"],
    );
    assert_eq!(output.status.code(), Some(0));
    let payload = json(&output);
    assert_eq!(result(&payload, "revoked"), &Value::Null);
    assert_eq!(result(&payload, "removed_token").as_bool(), Some(true));
    assert_eq!(
        result(&list(temp.path()), "accounts"),
        &serde_json::json!(["keep@example.com"])
    );
    never_contacted();
}

#[test]
fn revoke_of_an_account_without_a_token_reports_no_token_without_a_request() {
    let (uri, never_contacted) = untouchable_endpoint();
    let temp = seeded_with_revoke_endpoint(&uri);
    let metadata = temp.path().join("accounts.v1.json");
    let mut accounts: Value =
        serde_json::from_str(&std::fs::read_to_string(&metadata).expect("metadata")).expect("json");
    accounts["accounts"]
        .as_array_mut()
        .expect("accounts")
        .push(Value::from("ghost@example.com"));
    std::fs::write(&metadata, accounts.to_string()).expect("write metadata");

    let output = run(
        temp.path(),
        &[
            "--output",
            "json",
            "auth",
            "remove",
            "ghost@example.com",
            "--revoke",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(result(&json(&output), "revoked").as_str(), Some("no-token"));
    never_contacted();
}

#[test]
fn an_invalid_token_answer_outside_http_400_is_a_failure() {
    let (uri, _requests) = one_shot_server(401, r#"{"error":"invalid_token"}"#);
    let temp = seeded_with_revoke_endpoint(&uri);

    let output = run(
        temp.path(),
        &[
            "--output",
            "json",
            "auth",
            "remove",
            "gone@example.com",
            "--revoke",
        ],
    );
    assert_eq!(error_code(&output).as_deref(), Some("NILS_GOOGLE_007"));
    assert_eq!(
        result(&list(temp.path()), "accounts"),
        &serde_json::json!(["gone@example.com", "keep@example.com"])
    );
}

#[test]
fn lifecycle_arguments_fail_closed() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path(), &[]);
    seed_account(temp.path(), "me@example.com");

    let step_one_stdin = run_with_stdin(
        temp.path(),
        &[
            "--output",
            "json",
            "auth",
            "add",
            "other@example.com",
            "--remote",
            "--step",
            "1",
            "--callback-url-stdin",
        ],
        "http://localhost/?state=x&code=y\n",
    );
    assert_eq!(
        error_code(&step_one_stdin).as_deref(),
        Some("NILS_GOOGLE_005")
    );

    remote_step_one(temp.path(), "other@example.com");
    let empty = run_with_stdin(
        temp.path(),
        &[
            "--output",
            "json",
            "auth",
            "add",
            "other@example.com",
            "--remote",
            "--step",
            "2",
            "--callback-url-stdin",
        ],
        "\n  \n",
    );
    assert_eq!(error_code(&empty).as_deref(), Some("NILS_GOOGLE_005"));

    let bogus = run(
        temp.path(),
        &[
            "--output",
            "json",
            "auth",
            "remove",
            "me@example.com",
            "--bogus",
        ],
    );
    assert_eq!(error_code(&bogus).as_deref(), Some("NILS_GOOGLE_005"));
    assert_eq!(
        result(&list(temp.path()), "accounts"),
        &serde_json::json!(["me@example.com"])
    );
}

#[test]
fn default_accepts_an_alias() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path(), &[]);
    seed_account(temp.path(), "first@example.com");
    seed_account(temp.path(), "second@example.com");
    let alias = run(
        temp.path(),
        &[
            "--output",
            "json",
            "auth",
            "alias",
            "set",
            "work",
            "second@example.com",
        ],
    );
    assert_eq!(alias.status.code(), Some(0));

    let output = run(
        temp.path(),
        &["--output", "json", "auth", "default", "work"],
    );
    assert_eq!(
        result(&json(&output), "default_account").as_str(),
        Some("second@example.com")
    );
}

#[test]
fn credentials_written_before_revocation_existed_use_the_google_endpoint() {
    let temp = tempdir().expect("tempdir");
    let legacy = serde_json::json!({
        "version": 1,
        "credentials": {
            "client_id": "client-id",
            "client_secret": "client-secret",
            "auth_uri": "https://accounts.google.com/o/oauth2/v2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "redirect_uri": "http://localhost"
        }
    });
    std::fs::write(temp.path().join("credentials.v1.json"), legacy.to_string())
        .expect("write legacy credentials");

    let output = run(
        temp.path(),
        &["--output", "json", "auth", "credentials", "list"],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        result(&json(&output), "revoke_uri").as_str(),
        Some("https://oauth2.googleapis.com/revoke")
    );
}

#[test]
fn credentials_report_the_configured_revoke_endpoint() {
    let temp = tempdir().expect("tempdir");
    seed_credentials(temp.path(), &["--revoke-uri", "http://127.0.0.1:9/revoke"]);
    let output = run(
        temp.path(),
        &["--output", "json", "auth", "credentials", "list"],
    );
    assert_eq!(
        result(&json(&output), "revoke_uri").as_str(),
        Some("http://127.0.0.1:9/revoke")
    );
}
