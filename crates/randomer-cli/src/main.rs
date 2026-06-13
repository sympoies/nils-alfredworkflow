// Production paths in this binary route every fallible call through `?`
// plus a `NILS_RANDOMER_<NNN>` code; lock the gate so future regressions
// surface as build errors. Tests keep `unwrap()` / `expect()` for
// compactness — see the `#[allow]` on `mod tests` below.
#![deny(clippy::unwrap_used, clippy::expect_used)]

use clap::{Parser, Subcommand};
use randomer_cli::{RandomerError, generate_feedback, list_formats_feedback, list_types_feedback};
use workflow_common::ScriptFilterOutputModeArg as OutputModeArg;
use workflow_common::{
    EnvelopePayloadKind, OutputMode, build_error_envelope, build_success_envelope,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Randomer workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List supported formats as Alfred menu items.
    ListFormats {
        /// Optional case-insensitive filter against format keys.
        #[arg(long)]
        query: Option<String>,
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
    /// List type keys for selector flow in rrv mode.
    ListTypes {
        /// Optional case-insensitive filter against format keys.
        #[arg(long)]
        query: Option<String>,
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
    /// Generate values for a specific format.
    Generate {
        /// Target format key (case-insensitive).
        #[arg(long)]
        format: String,
        /// Number of values to generate.
        #[arg(long, default_value_t = 1)]
        count: usize,
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
}

impl Cli {
    fn command_name(&self) -> &'static str {
        match &self.command {
            Commands::ListFormats { .. } => "list-formats",
            Commands::ListTypes { .. } => "list-types",
            Commands::Generate { .. } => "generate",
        }
    }

    fn output_mode(&self) -> OutputMode {
        match &self.command {
            Commands::ListFormats { output, .. } => (*output).into(),
            Commands::ListTypes { output, .. } => (*output).into(),
            Commands::Generate { output, .. } => (*output).into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorKind {
    User,
    Runtime,
}

#[derive(Debug, PartialEq, Eq)]
struct AppError {
    kind: ErrorKind,
    message: String,
}

impl AppError {
    fn user(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::User,
            message: message.into(),
        }
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: message.into(),
        }
    }

    fn from_randomer(error: RandomerError) -> Self {
        match error {
            RandomerError::UnknownFormat(_) | RandomerError::InvalidCount(_) => {
                Self::user(error.to_string())
            }
        }
    }

    fn exit_code(&self) -> i32 {
        match self.kind {
            ErrorKind::User => 2,
            ErrorKind::Runtime => 1,
        }
    }

    fn code(&self) -> &'static str {
        match self.kind {
            ErrorKind::User => "NILS_RANDOMER_001",
            ErrorKind::Runtime => "NILS_RANDOMER_002",
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let command = cli.command_name();
    let mode = cli.output_mode();

    match run(cli) {
        Ok(output) => {
            println!("{output}");
        }
        Err(error) => {
            match mode {
                OutputMode::Json => {
                    println!("{}", serialize_service_error(command, &error));
                }
                OutputMode::AlfredJson => {
                    eprintln!("error: {}", error.message);
                }
                OutputMode::Human => {
                    unreachable!("only json and alfred-json output modes are supported")
                }
            }
            std::process::exit(error.exit_code());
        }
    }
}

fn run(cli: Cli) -> Result<String, AppError> {
    match cli.command {
        Commands::ListFormats { query, output } => {
            let payload = list_formats_feedback(query.as_deref());
            render_feedback(output.into(), "list-formats", payload)
        }
        Commands::ListTypes { query, output } => {
            let payload = list_types_feedback(query.as_deref());
            render_feedback(output.into(), "list-types", payload)
        }
        Commands::Generate {
            format,
            count,
            output,
        } => {
            let payload =
                generate_feedback(format.as_str(), count).map_err(AppError::from_randomer)?;
            render_feedback(output.into(), "generate", payload)
        }
    }
}

fn render_feedback(
    mode: OutputMode,
    command: &'static str,
    payload: alfred_core::Feedback,
) -> Result<String, AppError> {
    match mode {
        OutputMode::AlfredJson => payload.to_json().map_err(|error| {
            AppError::runtime(format!("failed to serialize Alfred feedback: {error}"))
        }),
        OutputMode::Json => {
            let result = payload.to_json().map_err(|error| {
                AppError::runtime(format!("failed to serialize Alfred feedback: {error}"))
            })?;
            Ok(build_success_envelope(
                command,
                EnvelopePayloadKind::Result,
                &result,
            ))
        }
        OutputMode::Human => unreachable!("only json and alfred-json output modes are supported"),
    }
}

fn serialize_service_error(command: &'static str, error: &AppError) -> String {
    build_error_envelope(command, error.code(), &error.message, None)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn list_formats_outputs_alfred_json_items() {
        let cli = Cli::parse_from(["randomer-cli", "list-formats"]);
        let output = run(cli).expect("list-formats should succeed");
        let json: Value = serde_json::from_str(&output).expect("output should be JSON");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be present");

        assert_eq!(items.len(), 11);
        assert!(
            items
                .first()
                .and_then(|item| item.get("mods"))
                .and_then(|mods| mods.get("cmd"))
                .is_some()
        );
    }

    #[test]
    fn list_formats_service_json_mode_wraps_result_in_v1_envelope() {
        let cli = Cli::parse_from(["randomer-cli", "list-formats", "--output", "json"]);
        let output = run(cli).expect("list-formats should succeed");
        let json: Value = serde_json::from_str(&output).expect("output should be JSON");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("list-formats")
        );
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
        assert!(
            json.get("result")
                .and_then(|result| result.get("items"))
                .and_then(Value::as_array)
                .is_some()
        );
    }

    #[test]
    fn list_formats_applies_case_insensitive_contains_filter() {
        let cli = Cli::parse_from(["randomer-cli", "list-formats", "--query", "HEX"]);
        let output = run(cli).expect("list-formats should succeed");
        let json: Value = serde_json::from_str(&output).expect("output should be JSON");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be present");

        assert_eq!(items.len(), 1);
        let subtitle = items[0]
            .get("subtitle")
            .and_then(Value::as_str)
            .expect("subtitle should exist");
        assert!(subtitle.contains("hex"));
    }

    #[test]
    fn generate_outputs_requested_count_for_known_format() {
        let cli = Cli::parse_from([
            "randomer-cli",
            "generate",
            "--format",
            "OtP",
            "--count",
            "4",
        ]);

        let output = run(cli).expect("generate should succeed");
        let json: Value = serde_json::from_str(&output).expect("output should be JSON");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be present");

        assert_eq!(items.len(), 4);
        assert!(items.iter().all(|item| {
            item.get("subtitle").and_then(Value::as_str) == Some("otp")
                && item.get("arg").and_then(Value::as_str)
                    == item.get("title").and_then(Value::as_str)
        }));
    }

    #[test]
    fn list_types_outputs_selector_items_with_format_args() {
        let cli = Cli::parse_from(["randomer-cli", "list-types", "--query", "in"]);
        let output = run(cli).expect("list-types should succeed");
        let json: Value = serde_json::from_str(&output).expect("output should be JSON");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be present");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].get("title").and_then(Value::as_str), Some("int"));
        assert_eq!(items[0].get("arg").and_then(Value::as_str), Some("int"));
        assert_eq!(
            items[0]
                .get("variables")
                .and_then(|vars| vars.get("RANDOMER_FORMAT"))
                .and_then(Value::as_str),
            Some("int")
        );
        assert!(
            items[0]
                .get("subtitle")
                .and_then(Value::as_str)
                .is_some_and(|subtitle| subtitle.contains("show 10 values"))
        );
    }

    #[test]
    fn invalid_format_returns_user_error_kind_and_exit_code_2() {
        let cli = Cli::parse_from(["randomer-cli", "generate", "--format", "not-a-format"]);
        let error = run(cli).expect_err("unknown format should fail");

        assert_eq!(error.kind, ErrorKind::User);
        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.message, "unknown format: not-a-format");
    }

    #[test]
    fn help_flag_is_supported() {
        let help = Cli::try_parse_from(["randomer-cli", "--help"])
            .expect_err("help should exit through clap");
        assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn service_error_envelope_has_required_error_fields() {
        let payload = serialize_service_error("generate", &AppError::user("unknown format: bad"));
        let json: Value = serde_json::from_str(&payload).expect("service error should be json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("generate")
        );
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
        assert!(json.get("result").is_none());
        assert_eq!(
            json.get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("NILS_RANDOMER_001")
        );
        assert!(
            json.get("error")
                .and_then(|error| error.get("details"))
                .is_none()
        );
    }
}
