use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

fn run_cli(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_steam-cli"));
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
    let server = MockServer::spawn(MockResponse::json(
        503,
        "Service Unavailable",
        r#"{"message":"upstream unavailable"}"#,
    ));

    let endpoint = format!("{}/api/storesearch", server.base_url());
    let output = run_cli(
        &["search", "--query", "dota"],
        &[("STEAM_STORE_SEARCH_ENDPOINT", &endpoint)],
    );

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("steam store api error (503): upstream unavailable"));

    server.join();
}

#[test]
fn cli_contract_success_returns_alfred_json_items() {
    let server = MockServer::spawn(MockResponse::json(
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
    ));

    let endpoint = format!("{}/api/storesearch", server.base_url());
    let output = run_cli(
        &["search", "--query", "counter strike"],
        &[
            ("STEAM_STORE_SEARCH_ENDPOINT", &endpoint),
            ("STEAM_REGION", "us"),
            ("STEAM_REGION_OPTIONS", "jp,us"),
            ("STEAM_LANGUAGE", "english"),
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

#[derive(Debug)]
struct MockResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: String,
}

impl MockResponse {
    fn json(status: u16, reason: &'static str, body: &str) -> Self {
        Self {
            status,
            reason,
            content_type: "application/json",
            body: body.to_string(),
        }
    }
}

struct MockServer {
    base_url: String,
    handle: Option<thread::JoinHandle<()>>,
    request_path: Arc<Mutex<Option<String>>>,
}

impl MockServer {
    fn spawn(response: MockResponse) -> Self {
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
                        if start.elapsed() > Duration::from_secs(3) {
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
                .and_then(|_| stream.write_all(response.body.as_bytes()))
                .expect("write response");
        });

        Self {
            base_url,
            handle: Some(handle),
            request_path,
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
            path.starts_with("/api/storesearch"),
            "unexpected request path: {path}"
        );
    }
}
