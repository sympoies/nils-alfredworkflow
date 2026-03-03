use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use prost::Message;
use serde_json::Value;

fn run_cli(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(resolve_cli_path());
    cmd.args(args);
    for (key, value) in envs {
        cmd.env(key, value);
    }

    cmd.output().expect("run steam-cli")
}

#[test]
fn cli_contract_empty_query_returns_user_error() {
    let output = run_cli(&["search", "--query", "   "], &[]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("query must not be empty"));
}

#[test]
fn cli_contract_invalid_config_returns_user_error() {
    let output = run_cli(&["search", "--query", "dota"], &[("STEAM_REGION", "USA")]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid STEAM_REGION"));
}

#[test]
fn cli_contract_api_failure_returns_runtime_error_message() {
    let server = MockServer::spawn(
        MockResponse::json(
            503,
            "Service Unavailable",
            r#"{"message":"upstream unavailable"}"#,
        ),
        "/IStoreQueryService/SearchSuggestions/v1",
    );

    let endpoint = format!(
        "{}{}",
        server.base_url(),
        "/IStoreQueryService/SearchSuggestions/v1"
    );
    let output = run_cli(
        &["search", "--query", "dota"],
        &[("STEAM_SEARCH_SUGGESTIONS_ENDPOINT", &endpoint)],
    );

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("steam store api error (503): upstream unavailable"));

    server.join();
}

#[test]
fn cli_contract_success_returns_alfred_json_items() {
    let response_body = encode_suggestions_response(vec![SearchSuggestionFixture {
        app_id: Some(730),
        name: "Counter-Strike 2".to_string(),
        prices: vec![SearchSuggestionPriceFixture {
            final_price_cents: Some(0),
            final_formatted: Some("Free".to_string()),
        }],
    }]);

    let server = MockServer::spawn(
        MockResponse::octet_stream(200, "OK", response_body),
        "/IStoreQueryService/SearchSuggestions/v1",
    );

    let endpoint = format!(
        "{}{}",
        server.base_url(),
        "/IStoreQueryService/SearchSuggestions/v1"
    );
    let output = run_cli(
        &["search", "--query", "counter strike"],
        &[
            ("STEAM_SEARCH_SUGGESTIONS_ENDPOINT", &endpoint),
            ("STEAM_REGION", "us"),
            ("STEAM_REGION_OPTIONS", "jp,us"),
            ("STEAM_SHOW_REGION_OPTIONS", "1"),
            ("STEAM_LANGUAGE", "english"),
        ],
    );

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("stdout should be json");
    assert!(
        json.get("schema_version").is_none(),
        "legacy steam contract does not emit schema_version yet"
    );
    assert!(
        json.get("command").is_none(),
        "legacy steam contract does not emit command yet"
    );
    assert!(
        json.get("ok").is_none(),
        "legacy steam contract does not emit ok yet"
    );
    let items = json
        .get("items")
        .and_then(Value::as_array)
        .expect("items should be array");

    assert_eq!(
        items[0].get("title").and_then(Value::as_str),
        Some("Current region: US")
    );
    assert_eq!(
        items[1].get("title").and_then(Value::as_str),
        Some("Search in JP region")
    );
    assert_eq!(
        items[3].get("arg").and_then(Value::as_str),
        Some("https://store.steampowered.com/app/730/?cc=us&l=english")
    );

    server.join();
}

#[test]
fn cli_contract_hides_region_switch_rows_by_default() {
    let response_body = encode_suggestions_response(vec![SearchSuggestionFixture {
        app_id: Some(730),
        name: "Counter-Strike 2".to_string(),
        prices: vec![SearchSuggestionPriceFixture {
            final_price_cents: Some(0),
            final_formatted: Some("Free".to_string()),
        }],
    }]);

    let server = MockServer::spawn(
        MockResponse::octet_stream(200, "OK", response_body),
        "/IStoreQueryService/SearchSuggestions/v1",
    );

    let endpoint = format!(
        "{}{}",
        server.base_url(),
        "/IStoreQueryService/SearchSuggestions/v1"
    );
    let output = run_cli(
        &["search", "--query", "counter strike"],
        &[
            ("STEAM_SEARCH_SUGGESTIONS_ENDPOINT", &endpoint),
            ("STEAM_REGION", "us"),
            ("STEAM_REGION_OPTIONS", "jp,us"),
        ],
    );

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("stdout should be json");
    let items = json
        .get("items")
        .and_then(Value::as_array)
        .expect("items should be array");

    assert_eq!(
        items[0].get("title").and_then(Value::as_str),
        Some("Counter-Strike 2")
    );
    assert_eq!(
        items[0].get("arg").and_then(Value::as_str),
        Some("https://store.steampowered.com/app/730/?cc=us")
    );
    assert_eq!(items.len(), 1);

    server.join();
}

#[test]
fn cli_contract_legacy_mode_uses_store_search_api() {
    let server = MockServer::spawn(
        MockResponse::json(
            200,
            "OK",
            r#"{
                "items": [
                    {
                        "id": 730,
                        "name": "Counter-Strike 2",
                        "price": {"final": 0, "final_formatted": "Free"},
                        "platforms": {"windows": true, "mac": false, "linux": true}
                    }
                ]
            }"#,
        ),
        "/api/storesearch",
    );

    let endpoint = format!("{}{}", server.base_url(), "/api/storesearch");
    let output = run_cli(
        &["search", "--query", "counter strike"],
        &[
            ("STEAM_SEARCH_API", "storesearch"),
            ("STEAM_STORE_SEARCH_ENDPOINT", &endpoint),
            ("STEAM_REGION", "us"),
        ],
    );

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("stdout should be json");
    let items = json
        .get("items")
        .and_then(Value::as_array)
        .expect("items should be array");

    assert_eq!(
        items[0].get("arg").and_then(Value::as_str),
        Some("https://store.steampowered.com/app/730/?cc=us")
    );

    server.join();
}

#[derive(Debug)]
struct MockResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
}

impl MockResponse {
    fn json(status: u16, reason: &'static str, body: &str) -> Self {
        Self {
            status,
            reason,
            content_type: "application/json",
            body: body.as_bytes().to_vec(),
        }
    }

    fn octet_stream(status: u16, reason: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            reason,
            content_type: "application/octet-stream",
            body,
        }
    }
}

struct MockServer {
    base_url: String,
    handle: Option<thread::JoinHandle<()>>,
    request_path: Arc<Mutex<Option<String>>>,
    expected_path_prefix: String,
}

impl MockServer {
    fn spawn(response: MockResponse, expected_path_prefix: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        listener
            .set_nonblocking(true)
            .expect("set nonblocking accept");

        let base_url = format!("http://{}", listener.local_addr().expect("read addr"));
        let request_path = Arc::new(Mutex::new(None));
        let captured_path = Arc::clone(&request_path);

        let handle = thread::spawn(move || {
            let start = Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if start.elapsed() > Duration::from_secs(10) {
                            panic!("mock server timed out waiting for request");
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("mock server accept failed: {error}"),
                }
            };

            let cloned = stream.try_clone().expect("clone stream");
            let mut reader = BufReader::new(cloned);
            let mut first_line = String::new();
            reader
                .read_line(&mut first_line)
                .expect("read request line");
            if let Some(path) = first_line.split_whitespace().nth(1).map(ToOwned::to_owned) {
                *captured_path.lock().expect("path lock") = Some(path);
            }

            loop {
                let mut line = String::new();
                let bytes = reader.read_line(&mut line).expect("read header line");
                if bytes == 0 || line == "\r\n" {
                    break;
                }
            }

            let response_head = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.status,
                response.reason,
                response.content_type,
                response.body.len()
            );

            stream
                .write_all(response_head.as_bytes())
                .and_then(|_| stream.write_all(response.body.as_slice()))
                .expect("write response");
        });

        Self {
            base_url,
            handle: Some(handle),
            request_path,
            expected_path_prefix: expected_path_prefix.to_string(),
        }
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn join(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.join().expect("mock server thread");
        }

        let path = self
            .request_path
            .lock()
            .expect("path lock")
            .clone()
            .unwrap_or_default();
        assert!(
            path.starts_with(&self.expected_path_prefix),
            "unexpected request path: {path}"
        );
    }
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionsResponseFixture {
    #[prost(message, repeated, tag = "3")]
    results: Vec<SearchSuggestionFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionFixture {
    #[prost(optional, uint32, tag = "2")]
    app_id: Option<u32>,
    #[prost(string, tag = "6")]
    name: String,
    #[prost(message, repeated, tag = "40")]
    prices: Vec<SearchSuggestionPriceFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionPriceFixture {
    #[prost(optional, uint32, tag = "5")]
    final_price_cents: Option<u32>,
    #[prost(optional, string, tag = "8")]
    final_formatted: Option<String>,
}

fn encode_suggestions_response(results: Vec<SearchSuggestionFixture>) -> Vec<u8> {
    SearchSuggestionsResponseFixture { results }.encode_to_vec()
}

fn resolve_cli_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_steam-cli") {
        return PathBuf::from(path);
    }

    if let Ok(current_exe) = std::env::current_exe()
        && let Some(debug_dir) = current_exe.parent().and_then(|deps| deps.parent())
    {
        let candidate = debug_dir.join(format!("steam-cli{}", std::env::consts::EXE_SUFFIX));
        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from(env!("CARGO_BIN_EXE_steam-cli"))
}
