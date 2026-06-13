use clap::{Parser, Subcommand};

use bilibili_cli::{
    bilibili_api::{self, BilibiliApiError, SuggestionTerm},
    config::{ConfigError, RuntimeConfig},
    feedback,
};

use workflow_common::ScriptFilterOutputModeArg as OutputModeArg;
use workflow_common::{
    EnvelopePayloadKind, OutputMode, build_error_envelope, build_success_envelope,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Bilibili workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Query bilibili suggestions from raw workflow input.
    Query {
        /// Query input text.
        #[arg(long)]
        input: String,
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
    /// Search bilibili suggestions from explicit query text.
    Search {
        /// Query text.
        #[arg(long)]
        query: String,
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
}

impl Cli {
    fn command_name(&self) -> &'static str {
        match &self.command {
            Commands::Query { .. } => "query",
            Commands::Search { .. } => "search",
        }
    }

    fn output_mode(&self) -> OutputMode {
        match &self.command {
            Commands::Query { output, .. } => (*output).into(),
            Commands::Search { output, .. } => (*output).into(),
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

    fn from_config(error: ConfigError) -> Self {
        AppError::user(error.to_string())
    }

    fn from_bilibili_api(error: BilibiliApiError) -> Self {
        match error {
            BilibiliApiError::Http { status, message } => {
                AppError::runtime(format!("bilibili api error ({status}): {message}"))
            }
            BilibiliApiError::Transport { .. } => AppError::runtime("bilibili api request failed"),
            BilibiliApiError::InvalidResponse(_) => {
                AppError::runtime("invalid bilibili api response")
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
            ErrorKind::User => "NILS_BILIBILI_001",
            ErrorKind::Runtime => "NILS_BILIBILI_002",
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
    run_with(
        cli,
        RuntimeConfig::from_env,
        bilibili_api::search_suggestions,
    )
}

fn run_with<LoadConfig, SearchSuggestions>(
    cli: Cli,
    load_config: LoadConfig,
    search_suggestions: SearchSuggestions,
) -> Result<String, AppError>
where
    LoadConfig: Fn() -> Result<RuntimeConfig, ConfigError>,
    SearchSuggestions: Fn(&RuntimeConfig, &str) -> Result<Vec<SuggestionTerm>, BilibiliApiError>,
{
    let command = cli.command_name();
    let (raw_query, output) = match cli.command {
        Commands::Query { input, output } => (input, output),
        Commands::Search { query, output } => (query, output),
    };

    let query = raw_query.trim();
    if query.is_empty() {
        return Err(AppError::user("query must not be empty"));
    }

    let config = load_config().map_err(AppError::from_config)?;
    let suggestions = search_suggestions(&config, query).map_err(AppError::from_bilibili_api)?;
    let payload = feedback::suggestions_to_feedback(query, &suggestions);

    render_feedback(output.into(), command, payload)
}

fn render_feedback(
    mode: OutputMode,
    command: &'static str,
    payload: alfred_core::Feedback,
) -> Result<String, AppError> {
    match mode {
        OutputMode::AlfredJson => payload
            .to_json()
            .map_err(|error| AppError::runtime(format!("failed to serialize feedback: {error}"))),
        OutputMode::Json => {
            let payload_json = payload.to_json().map_err(|error| {
                AppError::runtime(format!("failed to serialize feedback: {error}"))
            })?;
            Ok(build_success_envelope(
                command,
                EnvelopePayloadKind::Result,
                &payload_json,
            ))
        }
        OutputMode::Human => unreachable!("only json and alfred-json output modes are supported"),
    }
}

fn serialize_service_error(command: &'static str, error: &AppError) -> String {
    build_error_envelope(command, error.code(), &error.message, None)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn fixture_config() -> RuntimeConfig {
        RuntimeConfig {
            uid: Some("12345".to_string()),
            max_results: 5,
            timeout_ms: 8000,
            user_agent: "nils-bilibili-cli/test".to_string(),
        }
    }

    #[test]
    fn main_query_command_outputs_feedback_json_contract() {
        let cli = Cli::parse_from(["bilibili-cli", "query", "--input", "naruto"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Ok(vec![SuggestionTerm {
                    value: "naruto".to_string(),
                }])
            },
        )
        .expect("query should succeed");

        let payload: Value = serde_json::from_str(&output).expect("output should be json");
        assert_eq!(payload["items"][0]["title"], "naruto");
        assert_eq!(
            payload["items"][0]["arg"],
            "https://search.bilibili.com/all?keyword=naruto"
        );
    }

    #[test]
    fn main_search_command_reuses_same_pipeline() {
        let cli = Cli::parse_from(["bilibili-cli", "search", "--query", "naruto"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Ok(vec![SuggestionTerm {
                    value: "naruto mobile".to_string(),
                }])
            },
        )
        .expect("search should succeed");

        let payload: Value = serde_json::from_str(&output).expect("output should be json");
        assert_eq!(payload["items"][0]["title"], "naruto mobile");
    }

    #[test]
    fn main_query_command_rejects_empty_query() {
        let cli = Cli::parse_from(["bilibili-cli", "query", "--input", "   "]);
        let err = run_with(cli, || Ok(fixture_config()), |_, _| Ok(Vec::new()))
            .expect_err("empty query should fail");

        assert_eq!(err.kind, ErrorKind::User);
        assert_eq!(err.message, "query must not be empty");
    }

    #[test]
    fn main_query_command_surfaces_config_error_as_user_error() {
        let cli = Cli::parse_from(["bilibili-cli", "query", "--input", "naruto"]);

        let err = run_with(
            cli,
            || Err(ConfigError::InvalidMaxResults("abc".to_string())),
            |_, _| Ok(Vec::new()),
        )
        .expect_err("config error should fail");

        assert_eq!(err.kind, ErrorKind::User);
        assert_eq!(err.message, "invalid BILIBILI_MAX_RESULTS: abc");
    }

    #[test]
    fn main_query_command_surfaces_runtime_error() {
        let cli = Cli::parse_from(["bilibili-cli", "query", "--input", "naruto"]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Err(BilibiliApiError::Http {
                    status: 503,
                    message: "service unavailable".to_string(),
                })
            },
        )
        .expect_err("api error should fail");

        assert_eq!(err.kind, ErrorKind::Runtime);
        assert_eq!(err.message, "bilibili api error (503): service unavailable");
    }

    #[test]
    fn main_query_command_supports_service_json_error_mode() {
        let cli = Cli::parse_from(["bilibili-cli", "query", "--input", " ", "--output", "json"]);

        let err = run_with(cli, || Ok(fixture_config()), |_, _| Ok(Vec::new()))
            .expect_err("empty query should fail");

        let payload: Value =
            serde_json::from_str(&serialize_service_error("query", &err)).expect("valid json");
        assert_eq!(payload["ok"], false);
        assert_eq!(payload["error"]["code"], "NILS_BILIBILI_001");
    }
}
