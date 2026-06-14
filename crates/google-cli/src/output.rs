use std::io::{self, Write};

use serde_json::{Value, json};
use workflow_common::{EnvelopePayloadKind, build_error_envelope, build_success_envelope};

use crate::error::{AppError, redact_sensitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedOutput {
    pub stdout: String,
    pub stderr: String,
}

impl RenderedOutput {
    pub fn emit(&self) {
        let stdout_handle = io::stdout();
        let stderr_handle = io::stderr();
        let mut stdout = stdout_handle.lock();
        let mut stderr = stderr_handle.lock();
        self.emit_to(&mut stdout, &mut stderr);
    }

    fn emit_to(&self, stdout: &mut impl Write, stderr: &mut impl Write) {
        if !self.stdout.is_empty() {
            let _ = stdout.write_all(self.stdout.as_bytes());
            let _ = stdout.flush();
        }
        if !self.stderr.is_empty() {
            let _ = stderr.write_all(self.stderr.as_bytes());
            let _ = stderr.flush();
        }
    }
}

pub fn render_success(
    command_id: &str,
    mode: OutputMode,
    payload: Value,
    text: &str,
) -> RenderedOutput {
    match mode {
        OutputMode::Json => RenderedOutput {
            stdout: format_success_envelope(command_id, payload),
            stderr: String::new(),
        },
        OutputMode::Human | OutputMode::Plain => RenderedOutput {
            stdout: format!("{text}\n"),
            stderr: String::new(),
        },
    }
}

pub fn render_error(command_id: &str, mode: OutputMode, error: &AppError) -> RenderedOutput {
    match mode {
        OutputMode::Json => RenderedOutput {
            stdout: format_error_envelope(command_id, error),
            stderr: String::new(),
        },
        OutputMode::Human | OutputMode::Plain => RenderedOutput {
            stdout: String::new(),
            stderr: format!(
                "error[{}]: {}\n",
                error.code(),
                redact_sensitive(error.message())
            ),
        },
    }
}

fn format_success_envelope(command_id: &str, payload: Value) -> String {
    let payload_json = payload.to_string();
    let envelope = build_success_envelope(command_id, EnvelopePayloadKind::Result, &payload_json);
    canonicalize_envelope_json(&envelope)
}

fn format_error_envelope(command_id: &str, error: &AppError) -> String {
    let mut details = json!({
        "kind": error.kind().as_str(),
        "exit_code": error.exit_code(),
    });

    if let Some(context) = error.details() {
        details["context"] = context.clone();
    }

    let details_json = details.to_string();
    let envelope = build_error_envelope(
        command_id,
        error.code(),
        error.message(),
        Some(&details_json),
    );
    canonicalize_envelope_json(&envelope)
}

fn canonicalize_envelope_json(envelope: &str) -> String {
    match serde_json::from_str::<Value>(envelope) {
        Ok(value) => value.to_string(),
        Err(_) => envelope.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{OutputMode, render_error, render_success};

    #[test]
    fn json_success_wraps_payload() {
        let output = render_success(
            "google.auth.list",
            OutputMode::Json,
            serde_json::json!({"accounts": ["me@example.com"]}),
            "Listed native auth accounts.",
        );

        let json: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("google.auth.list")
        );
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
        assert!(
            json.get("result")
                .and_then(|result| result.get("accounts"))
                .and_then(Value::as_array)
                .is_some()
        );
        assert_eq!(
            output.stdout,
            "{\"command\":\"google.auth.list\",\"ok\":true,\"result\":{\"accounts\":[\"me@example.com\"]},\"schema_version\":\"cli-envelope@v1\"}"
        );
    }

    #[test]
    fn json_error_wraps_context_details() {
        let error = crate::error::AppError::invalid_auth_input("missing account");
        let output = render_error("google.auth.add", OutputMode::Json, &error);
        let json: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(
            json.get("error")
                .and_then(|value| value.get("code"))
                .and_then(Value::as_str),
            Some("NILS_GOOGLE_005")
        );
        assert_eq!(
            json.get("error")
                .and_then(|value| value.get("details"))
                .and_then(|value| value.get("context"))
                .and_then(|value| value.get("kind"))
                .and_then(Value::as_str),
            Some("auth_invalid_input")
        );
        assert_eq!(
            output.stdout,
            "{\"command\":\"google.auth.add\",\"error\":{\"code\":\"NILS_GOOGLE_005\",\"details\":{\"context\":{\"kind\":\"auth_invalid_input\"},\"exit_code\":2,\"kind\":\"user\"},\"message\":\"missing account\"},\"ok\":false,\"schema_version\":\"cli-envelope@v1\"}"
        );
    }

    #[test]
    fn emit_writes_stdout_and_stderr_to_provided_streams() {
        let output = super::RenderedOutput {
            stdout: "{\"ok\":true}\n".to_string(),
            stderr: "error\n".to_string(),
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        output.emit_to(&mut stdout, &mut stderr);

        assert_eq!(
            String::from_utf8(stdout).expect("stdout utf8"),
            "{\"ok\":true}\n"
        );
        assert_eq!(String::from_utf8(stderr).expect("stderr utf8"), "error\n");
    }
}
