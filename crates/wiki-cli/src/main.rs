use clap::{Parser, Subcommand};

use wiki_cli::{
    config::{ConfigError, RuntimeConfig},
    feedback,
    wiki_api::{self, WikiApiError, WikiSearchResult},
};

use workflow_common::ScriptFilterOutputModeArg as OutputModeArg;
use workflow_common::{
    AppError, EnvelopePayloadKind, OutputMode, build_error_envelope, build_success_envelope,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Wiki workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Search Wikipedia articles and print Alfred feedback JSON.
    Search {
        /// Search query text.
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
            Commands::Search { .. } => "search",
        }
    }

    fn output_mode(&self) -> OutputMode {
        match &self.command {
            Commands::Search { output, .. } => (*output).into(),
        }
    }
}

const ERROR_CODE_USER: &str = "NILS_WIKI_001";
const ERROR_CODE_RUNTIME: &str = "NILS_WIKI_002";

fn from_config(error: ConfigError) -> AppError {
    AppError::user(ERROR_CODE_USER, error.to_string())
}

fn from_wiki_api(error: WikiApiError) -> AppError {
    match error {
        WikiApiError::Http { status, message } => AppError::runtime(
            ERROR_CODE_RUNTIME,
            format!("wikipedia api error ({status}): {message}"),
        ),
        WikiApiError::Transport { .. } => {
            AppError::runtime(ERROR_CODE_RUNTIME, "wikipedia api request failed")
        }
        WikiApiError::InvalidResponse(_) => {
            AppError::runtime(ERROR_CODE_RUNTIME, "invalid wikipedia api response")
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
    run_with(cli, RuntimeConfig::from_env, wiki_api::search_articles)
}

fn run_with<LoadConfig, SearchArticles>(
    cli: Cli,
    load_config: LoadConfig,
    search_articles: SearchArticles,
) -> Result<String, AppError>
where
    LoadConfig: Fn() -> Result<RuntimeConfig, ConfigError>,
    SearchArticles: Fn(&RuntimeConfig, &str) -> Result<Vec<WikiSearchResult>, WikiApiError>,
{
    match cli.command {
        Commands::Search { query, output } => {
            let query = query.trim();
            if query.is_empty() {
                return Err(AppError::user(ERROR_CODE_USER, "query must not be empty"));
            }

            let config = load_config().map_err(from_config)?;
            let results = search_articles(&config, query).map_err(from_wiki_api)?;

            let payload = feedback::search_results_to_feedback(
                &config.language,
                query,
                &config.language_options,
                &results,
            );
            render_feedback(output.into(), "search", payload)
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
                format!("failed to serialize feedback: {error}"),
            )
        }),
        OutputMode::Json => {
            let payload_json = payload.to_json().map_err(|error| {
                AppError::runtime(
                    ERROR_CODE_RUNTIME,
                    format!("failed to serialize feedback: {error}"),
                )
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
    build_error_envelope(command, error.code(), error.message(), None)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use workflow_common::CliErrorKind;

    use super::*;

    fn fixture_config() -> RuntimeConfig {
        RuntimeConfig {
            language: "en".to_string(),
            language_options: Vec::new(),
            max_results: 5,
        }
    }

    #[test]
    fn main_search_command_outputs_feedback_json_contract() {
        let cli = Cli::parse_from(["wiki-cli", "search", "--query", "rust"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Ok(vec![WikiSearchResult {
                    title: "Rust (programming language)".to_string(),
                    snippet: "A language empowering everyone".to_string(),
                    pageid: 36192,
                }])
            },
        )
        .expect("search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        let result_item = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.iter().find(|item| item.get("arg").is_some()))
            .expect("result item should exist");

        assert_eq!(
            result_item.get("title").and_then(Value::as_str),
            Some("Rust (programming language)")
        );
        assert_eq!(
            result_item.get("subtitle").and_then(Value::as_str),
            Some("A language empowering everyone")
        );
        assert_eq!(
            result_item.get("arg").and_then(Value::as_str),
            Some("https://en.wikipedia.org/?curid=36192")
        );
    }

    #[test]
    fn main_search_rows_include_configured_language_switch_items_in_order() {
        let cli = Cli::parse_from(["wiki-cli", "search", "--query", "rust"]);

        let output = run_with(
            cli,
            || {
                Ok(RuntimeConfig {
                    language: "en".to_string(),
                    language_options: vec!["zh".to_string(), "en".to_string(), "ja".to_string()],
                    max_results: 5,
                })
            },
            |_, _| {
                Ok(vec![WikiSearchResult {
                    title: "Rust".to_string(),
                    snippet: "Systems language".to_string(),
                    pageid: 123,
                }])
            },
        )
        .expect("search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be array");

        assert_eq!(
            items[0].get("title").and_then(Value::as_str),
            Some("Current language: en")
        );
        assert_eq!(
            items[1].get("title").and_then(Value::as_str),
            Some("Search in zh Wikipedia")
        );
        assert_eq!(
            items[2].get("title").and_then(Value::as_str),
            Some("Search in en Wikipedia")
        );
        assert_eq!(
            items[3].get("title").and_then(Value::as_str),
            Some("Search in ja Wikipedia")
        );
        assert_eq!(items[4].get("title").and_then(Value::as_str), Some("Rust"));
    }

    #[test]
    fn main_search_service_json_mode_wraps_result_in_v1_envelope() {
        let cli = Cli::parse_from(["wiki-cli", "search", "--query", "rust", "--output", "json"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Ok(vec![WikiSearchResult {
                    title: "Rust (programming language)".to_string(),
                    snippet: "A language empowering everyone".to_string(),
                    pageid: 36192,
                }])
            },
        )
        .expect("search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(json.get("command").and_then(Value::as_str), Some("search"));
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
        assert!(
            json.get("result")
                .and_then(|result| result.get("items"))
                .and_then(Value::as_array)
                .is_some()
        );
    }

    #[test]
    fn main_rejects_empty_query_as_user_error() {
        let cli = Cli::parse_from(["wiki-cli", "search", "--query", "   "]);

        let err = run_with(cli, || Ok(fixture_config()), |_, _| Ok(Vec::new()))
            .expect_err("empty query should fail");

        assert_eq!(err.kind(), CliErrorKind::User);
        assert_eq!(err.message(), "query must not be empty");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_surfaces_config_errors_with_user_exit_kind() {
        let cli = Cli::parse_from(["wiki-cli", "search", "--query", "rust"]);

        let err = run_with(
            cli,
            || Err(ConfigError::InvalidMaxResults("abc".to_string())),
            |_, _| Ok(Vec::new()),
        )
        .expect_err("config errors should fail");

        assert_eq!(err.kind(), CliErrorKind::User);
        assert_eq!(err.message(), "invalid WIKI_MAX_RESULTS: abc");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_maps_http_api_failures_to_runtime_error_kind() {
        let cli = Cli::parse_from(["wiki-cli", "search", "--query", "rust"]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Err(WikiApiError::Http {
                    status: 503,
                    message: "service unavailable".to_string(),
                })
            },
        )
        .expect_err("api errors should fail");

        assert_eq!(err.kind(), CliErrorKind::Runtime);
        assert_eq!(
            err.message(),
            "wikipedia api error (503): service unavailable"
        );
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn main_maps_invalid_response_failures_to_runtime_error_kind() {
        let cli = Cli::parse_from(["wiki-cli", "search", "--query", "rust"]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Err(WikiApiError::InvalidResponse(
                    serde_json::from_str::<serde_json::Value>("not-json")
                        .expect_err("fixture must produce parse error"),
                ))
            },
        )
        .expect_err("invalid response should fail");

        assert_eq!(err.kind(), CliErrorKind::Runtime);
        assert_eq!(err.message(), "invalid wikipedia api response");
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn main_help_flag_is_supported() {
        let help = Cli::try_parse_from(["wiki-cli", "--help"])
            .expect_err("help should exit through clap error");

        assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn main_service_error_envelope_has_required_error_fields() {
        let payload = serialize_service_error(
            "search",
            &AppError::user(ERROR_CODE_USER, "query must not be empty"),
        );
        let json: Value = serde_json::from_str(&payload).expect("service error should be json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(json.get("command").and_then(Value::as_str), Some("search"));
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
        assert!(json.get("result").is_none());
        assert_eq!(
            json.get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("NILS_WIKI_001")
        );
        assert!(
            json.get("error")
                .and_then(|error| error.get("details"))
                .is_none()
        );
    }
}
