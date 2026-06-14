use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};

use timezone_cli::{
    convert,
    error::{AppError, ERROR_CODE_RUNTIME},
    feedback, local_tz,
    parser::{self, TimezoneEntry},
};
use workflow_common::ScriptFilterOutputModeArg as OutputModeArg;
use workflow_common::{
    EnvelopePayloadKind, OutputMode, build_error_envelope, build_success_envelope,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Multi-timezone workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Render current time rows for query/config timezone list.
    Now {
        /// Query timezone list (comma/newline separated IANA IDs).
        #[arg(long, default_value = "")]
        query: String,
        /// Configured fallback timezone list when query is empty.
        #[arg(long = "config-zones", default_value = "")]
        config_zones: String,
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
}

impl Cli {
    fn command_name(&self) -> &'static str {
        match &self.command {
            Commands::Now { .. } => "now",
        }
    }

    fn output_mode(&self) -> OutputMode {
        match &self.command {
            Commands::Now { output, .. } => (*output).into(),
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
                    eprintln!("error: {}", error.message());
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
    run_with(cli, Utc::now, local_tz::detect_local_timezone)
}

fn run_with<Now, DetectLocal>(
    cli: Cli,
    now: Now,
    detect_local: DetectLocal,
) -> Result<String, AppError>
where
    Now: Fn() -> DateTime<Utc>,
    DetectLocal: Fn() -> local_tz::LocalTimezone,
{
    match cli.command {
        Commands::Now {
            query,
            config_zones,
            output,
        } => {
            let zones = resolve_zones(&query, &config_zones, detect_local)?;
            let rows = convert::now_rows(now(), &zones);
            let payload = feedback::rows_to_feedback(&rows);
            render_feedback(output.into(), "now", payload)
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
            AppError::runtime(
                ERROR_CODE_RUNTIME,
                format!("failed to serialize timezone feedback: {error}"),
            )
        }),
        OutputMode::Json => {
            let result = payload.to_json().map_err(|error| {
                AppError::runtime(
                    ERROR_CODE_RUNTIME,
                    format!("failed to serialize timezone feedback: {error}"),
                )
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
    build_error_envelope(command, error.code(), error.message(), None)
}

fn resolve_zones<DetectLocal>(
    query: &str,
    config_zones: &str,
    detect_local: DetectLocal,
) -> Result<Vec<TimezoneEntry>, AppError>
where
    DetectLocal: Fn() -> local_tz::LocalTimezone,
{
    if !query.trim().is_empty() {
        return parser::parse_timezone_list(query).map_err(AppError::from);
    }

    if !config_zones.trim().is_empty() {
        return parser::parse_timezone_list(config_zones).map_err(AppError::from);
    }

    let local = detect_local();
    Ok(vec![TimezoneEntry::new(local.id, local.tz)])
}

#[cfg(test)]
mod tests {
    use chrono_tz::Tz;
    use serde_json::Value;
    use timezone_cli::{
        error::{ERROR_CODE_USER, ErrorKind},
        local_tz::LocalTimezoneSource,
    };

    use super::*;

    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-02-10T12:00:00+00:00")
            .expect("parse fixed now")
            .with_timezone(&Utc)
    }

    fn fixed_local(id: &str) -> local_tz::LocalTimezone {
        let tz = id.parse::<Tz>().expect("valid timezone fixture");
        local_tz::LocalTimezone {
            id: id.to_string(),
            tz,
            source: LocalTimezoneSource::Override,
            trace: vec!["test fixture".to_string()],
        }
    }

    fn item_uids(json: &Value) -> Vec<&str> {
        json.get("items")
            .and_then(Value::as_array)
            .expect("items array")
            .iter()
            .filter_map(|item| item.get("uid").and_then(Value::as_str))
            .collect()
    }

    #[test]
    fn order_preserved_for_config_list() {
        let cli = Cli::parse_from([
            "timezone-cli",
            "now",
            "--query",
            "",
            "--config-zones",
            "Asia/Taipei\nAmerica/New_York,Europe/London",
        ]);

        let output = run_with(cli, fixed_now, || fixed_local("UTC")).expect("run should pass");
        let json: Value = serde_json::from_str(&output).expect("json output");

        assert_eq!(
            item_uids(&json),
            vec!["Asia/Taipei", "America/New_York", "Europe/London"]
        );
    }

    #[test]
    fn service_json_mode_wraps_result_in_v1_envelope() {
        let cli = Cli::parse_from([
            "timezone-cli",
            "now",
            "--query",
            "Asia/Taipei",
            "--config-zones",
            "",
            "--output",
            "json",
        ]);

        let output = run_with(cli, fixed_now, || fixed_local("UTC")).expect("run should pass");
        let json: Value = serde_json::from_str(&output).expect("json output");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(json.get("command").and_then(Value::as_str), Some("now"));
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
        assert!(
            json.get("result")
                .and_then(|result| result.get("items"))
                .and_then(Value::as_array)
                .is_some()
        );
    }

    #[test]
    fn query_overrides_config_list() {
        let cli = Cli::parse_from([
            "timezone-cli",
            "now",
            "--query",
            "Europe/London,Asia/Tokyo",
            "--config-zones",
            "Asia/Taipei,America/New_York",
        ]);

        let output = run_with(cli, fixed_now, || fixed_local("UTC")).expect("run should pass");
        let json: Value = serde_json::from_str(&output).expect("json output");

        assert_eq!(item_uids(&json), vec!["Europe/London", "Asia/Tokyo"]);
    }

    #[test]
    fn empty_query_and_config_uses_detected_local_timezone() {
        let cli = Cli::parse_from(["timezone-cli", "now", "--query", "", "--config-zones", ""]);

        let output = run_with(cli, fixed_now, || fixed_local("Asia/Taipei"))
            .expect("local fallback should pass");
        let json: Value = serde_json::from_str(&output).expect("json output");

        assert_eq!(item_uids(&json), vec!["Asia/Taipei"]);
    }

    #[test]
    fn invalid_timezone_returns_user_error() {
        let cli = Cli::parse_from([
            "timezone-cli",
            "now",
            "--query",
            "Mars/Olympus",
            "--config-zones",
            "",
        ]);

        let error = run_with(cli, fixed_now, || fixed_local("UTC")).expect_err("invalid input");

        assert_eq!(error.kind(), ErrorKind::User);
        assert_eq!(error.exit_code(), 2);
        assert!(error.message().contains("invalid timezone"));
    }

    #[test]
    fn help_flag_is_supported() {
        let help = Cli::try_parse_from(["timezone-cli", "--help"])
            .expect_err("help should be surfaced by clap");

        assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn service_error_envelope_has_required_error_fields() {
        let payload =
            serialize_service_error("now", &AppError::user(ERROR_CODE_USER, "invalid timezone"));
        let json: Value = serde_json::from_str(&payload).expect("service error should be json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(json.get("command").and_then(Value::as_str), Some("now"));
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
        assert!(json.get("result").is_none());
        assert_eq!(
            json.get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("NILS_TIMEZONE_001")
        );
        assert!(
            json.get("error")
                .and_then(|error| error.get("details"))
                .is_none()
        );
    }
}
