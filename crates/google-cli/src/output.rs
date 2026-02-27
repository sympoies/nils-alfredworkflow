use serde_json::{Value, json};

use crate::error::{AppError, ErrorKind, redact_sensitive};
use crate::runtime::ProcessOutput;

pub const ENVELOPE_SCHEMA_VERSION: &str = "v1";

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
        if !self.stdout.is_empty() {
            print!("{}", self.stdout);
        }
        if !self.stderr.is_empty() {
            eprint!("{}", self.stderr);
        }
    }
}

pub fn render_success(
    command_id: &str,
    mode: OutputMode,
    process: ProcessOutput,
) -> Result<RenderedOutput, AppError> {
    match mode {
        OutputMode::Json => {
            let stdout = String::from_utf8_lossy(&process.stdout).to_string();
            let payload: Value = serde_json::from_str(&stdout)
                .map_err(|error| AppError::invalid_json(command_id, stdout.as_str(), &error))?;
            Ok(RenderedOutput {
                stdout: format_success_envelope(command_id, payload),
                stderr: String::new(),
            })
        }
        OutputMode::Human | OutputMode::Plain => Ok(RenderedOutput {
            stdout: String::from_utf8_lossy(&process.stdout).to_string(),
            stderr: String::from_utf8_lossy(&process.stderr).to_string(),
        }),
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
    json!({
        "schema_version": ENVELOPE_SCHEMA_VERSION,
        "command": command_id,
        "ok": true,
        "result": payload,
    })
    .to_string()
}

fn format_error_envelope(command_id: &str, error: &AppError) -> String {
    let kind = match error.kind() {
        ErrorKind::User => "user",
        ErrorKind::Runtime => "runtime",
    };
    let mut envelope = json!({
        "schema_version": ENVELOPE_SCHEMA_VERSION,
        "command": command_id,
        "ok": false,
        "error": {
            "code": error.code(),
            "message": redact_sensitive(error.message()),
            "details": {
                "kind": kind,
                "exit_code": error.exit_code(),
            }
        }
    });

    if let Some(details) = error.details() {
        envelope["error"]["details"]["wrapper"] = details.clone();
    }

    envelope.to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{OutputMode, render_success};
    use crate::runtime::ProcessOutput;

    #[test]
    fn json_success_wraps_valid_payload() {
        let output = render_success(
            "google.auth.list",
            OutputMode::Json,
            ProcessOutput {
                stdout: br#"{"accounts":["me@example.com"]}"#.to_vec(),
                stderr: Vec::new(),
            },
        )
        .expect("render");

        let json: Value = serde_json::from_str(&output.stdout).expect("json");
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
                .and_then(|result| result.get("accounts"))
                .and_then(Value::as_array)
                .is_some()
        );
    }

    #[test]
    fn json_success_rejects_invalid_payload() {
        let error = render_success(
            "google.auth.list",
            OutputMode::Json,
            ProcessOutput {
                stdout: b"not-json".to_vec(),
                stderr: Vec::new(),
            },
        )
        .expect_err("invalid payload should fail");

        assert_eq!(error.code(), crate::error::ERROR_CODE_RUNTIME_INVALID_JSON);
    }
}
