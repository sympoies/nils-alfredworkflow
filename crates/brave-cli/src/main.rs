use clap::{Parser, Subcommand};

use brave_cli::{
    brave_api::{self, BraveApiError, WebSearchResult},
    config::{ConfigError, RuntimeConfig},
    feedback,
    google_suggest::{self, DEFAULT_SUGGEST_MAX_RESULTS, GoogleSuggestError},
    token::{self, QueryToken},
};

use workflow_common::ScriptFilterOutputModeArg as OutputModeArg;
use workflow_common::{
    AppError, EnvelopePayloadKind, OutputMode, build_error_envelope, build_success_envelope,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Brave search workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Search Brave web results and print Alfred feedback JSON.
    Search {
        /// Search query text.
        #[arg(long)]
        query: String,
        /// Canonical output mode (`json` or `alfred-json`).
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
    /// Query Google suggestions, then search selected tokenized query.
    Query {
        /// Query text from Alfred script filter.
        #[arg(long)]
        input: String,
        /// Canonical output mode (`json` or `alfred-json`).
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
}

impl Cli {
    fn command_name(&self) -> &'static str {
        match &self.command {
            Commands::Search { .. } => "search",
            Commands::Query { .. } => "query",
        }
    }

    fn output_mode(&self) -> OutputMode {
        match &self.command {
            Commands::Search { output, .. } => (*output).into(),
            Commands::Query { output, .. } => (*output).into(),
        }
    }
}

const ERROR_CODE_USER: &str = "NILS_BRAVE_001";
const ERROR_CODE_RUNTIME: &str = "NILS_BRAVE_002";

fn from_config(error: ConfigError) -> AppError {
    AppError::user(ERROR_CODE_USER, error.to_string())
}

fn from_brave_api(error: BraveApiError) -> AppError {
    match error {
        BraveApiError::Http { status, message } => AppError::runtime(
            ERROR_CODE_RUNTIME,
            format!("brave api error ({status}): {message}"),
        ),
        BraveApiError::Transport { .. } => {
            AppError::runtime(ERROR_CODE_RUNTIME, "brave api request failed")
        }
        BraveApiError::InvalidResponse(_) => {
            AppError::runtime(ERROR_CODE_RUNTIME, "invalid brave api response")
        }
    }
}

fn from_google_suggest(error: GoogleSuggestError) -> AppError {
    match error {
        GoogleSuggestError::Transport { .. } => {
            AppError::runtime(ERROR_CODE_RUNTIME, "google suggest request failed")
        }
        GoogleSuggestError::InvalidResponse(_) => {
            AppError::runtime(ERROR_CODE_RUNTIME, "invalid google suggest response")
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
                    unreachable!("brave-cli only supports json and alfred-json output modes")
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
        brave_api::search_web,
        google_suggest::fetch_suggestions,
    )
}

fn run_with<LoadConfig, SearchWeb, FetchSuggestions>(
    cli: Cli,
    load_config: LoadConfig,
    search_web: SearchWeb,
    fetch_suggestions: FetchSuggestions,
) -> Result<String, AppError>
where
    LoadConfig: Fn() -> Result<RuntimeConfig, ConfigError>,
    SearchWeb: Fn(&RuntimeConfig, &str) -> Result<Vec<WebSearchResult>, BraveApiError>,
    FetchSuggestions: Fn(&str, u8) -> Result<Vec<String>, GoogleSuggestError>,
{
    match cli.command {
        Commands::Search { query, output } => {
            let query = query.trim();
            if query.is_empty() {
                return Err(AppError::user(ERROR_CODE_USER, "query must not be empty"));
            }

            let config = load_config().map_err(from_config)?;
            let results = search_web(&config, query).map_err(from_brave_api)?;

            let payload = feedback::search_results_to_feedback(&results);
            render_feedback(output.into(), "search", payload)
        }
        Commands::Query { input, output } => {
            let payload = match token::parse_query_token(&input) {
                QueryToken::Empty => feedback::empty_input_feedback(),
                QueryToken::SearchMissingQuery => feedback::missing_search_target_feedback(),
                QueryToken::Suggest { query } => {
                    let suggestions = fetch_suggestions(&query, DEFAULT_SUGGEST_MAX_RESULTS)
                        .map_err(from_google_suggest)?;
                    feedback::suggestions_to_feedback(&query, &suggestions)
                }
                QueryToken::Search { query } => {
                    let config = load_config().map_err(from_config)?;
                    let results = search_web(&config, &query).map_err(from_brave_api)?;
                    feedback::search_results_to_feedback(&results)
                }
            };

            render_feedback(output.into(), "query", payload)
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
        OutputMode::Human => {
            unreachable!("brave-cli only supports json and alfred-json output modes")
        }
    }
}

fn serialize_service_error(command: &'static str, error: &AppError) -> String {
    build_error_envelope(command, error.code(), error.message(), None)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use brave_cli::config::SafeSearch;

    use workflow_common::CliErrorKind;

    use super::*;

    fn fixture_config() -> RuntimeConfig {
        RuntimeConfig {
            api_key: "demo-key".to_string(),
            count: 5,
            safesearch: SafeSearch::Moderate,
            country: None,
        }
    }

    fn fixture_suggestions(
        _query: &str,
        _max_results: u8,
    ) -> Result<Vec<String>, GoogleSuggestError> {
        Ok(vec!["rust language".to_string(), "rust book".to_string()])
    }

    #[test]
    fn main_search_command_outputs_feedback_json_contract() {
        let cli = Cli::parse_from(["brave-cli", "search", "--query", "rust"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Ok(vec![WebSearchResult {
                    title: "Rust Language".to_string(),
                    url: "https://www.rust-lang.org/".to_string(),
                    description: "Build reliable software".to_string(),
                }])
            },
            fixture_suggestions,
        )
        .expect("search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        let first_item = json
            .get("items")
            .and_then(|items| items.get(0))
            .expect("first item should exist");

        assert_eq!(
            first_item.get("title").and_then(Value::as_str),
            Some("Rust Language")
        );
        assert_eq!(
            first_item.get("subtitle").and_then(Value::as_str),
            Some("rust-lang.org | Build reliable software")
        );
        assert_eq!(
            first_item.get("arg").and_then(Value::as_str),
            Some("https://www.rust-lang.org/")
        );
    }

    #[test]
    fn main_search_service_json_mode_wraps_result_in_v1_envelope() {
        let cli = Cli::parse_from(["brave-cli", "search", "--query", "rust", "--output", "json"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Ok(vec![WebSearchResult {
                    title: "Rust Language".to_string(),
                    url: "https://www.rust-lang.org/".to_string(),
                    description: "Build reliable software".to_string(),
                }])
            },
            fixture_suggestions,
        )
        .expect("search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(json.get("command").and_then(Value::as_str), Some("search"));
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
        assert!(json.get("error").is_none());
        assert!(
            json.get("result")
                .and_then(|result| result.get("items"))
                .and_then(Value::as_array)
                .is_some()
        );
    }

    #[test]
    fn main_rejects_empty_query_as_user_error() {
        let cli = Cli::parse_from(["brave-cli", "search", "--query", "   "]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| Ok(Vec::new()),
            fixture_suggestions,
        )
        .expect_err("empty query should fail");

        assert_eq!(err.kind(), CliErrorKind::User);
        assert_eq!(err.message(), "query must not be empty");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_surfaces_config_errors_with_user_exit_kind() {
        let cli = Cli::parse_from(["brave-cli", "search", "--query", "rust"]);

        let err = run_with(
            cli,
            || Err(ConfigError::MissingApiKey),
            |_, _| Ok(Vec::new()),
            fixture_suggestions,
        )
        .expect_err("missing config should fail");

        assert_eq!(err.kind(), CliErrorKind::User);
        assert_eq!(err.message(), "missing BRAVE_API_KEY");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_maps_http_api_failures_to_runtime_error_kind() {
        let cli = Cli::parse_from(["brave-cli", "search", "--query", "rust"]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Err(BraveApiError::Http {
                    status: 429,
                    message: "rate limit exceeded".to_string(),
                })
            },
            fixture_suggestions,
        )
        .expect_err("api errors should fail");

        assert_eq!(err.kind(), CliErrorKind::Runtime);
        assert_eq!(err.message(), "brave api error (429): rate limit exceeded");
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn main_maps_invalid_response_failures_to_runtime_error_kind() {
        let cli = Cli::parse_from(["brave-cli", "search", "--query", "rust"]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Err(BraveApiError::InvalidResponse(
                    serde_json::from_str::<serde_json::Value>("not-json")
                        .expect_err("fixture must produce parse error"),
                ))
            },
            fixture_suggestions,
        )
        .expect_err("invalid response should fail");

        assert_eq!(err.kind(), CliErrorKind::Runtime);
        assert_eq!(err.message(), "invalid brave api response");
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn main_help_flag_is_supported() {
        let help = Cli::try_parse_from(["brave-cli", "--help"])
            .expect_err("help should exit through clap error");

        assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn main_query_suggest_mode_maps_to_autocomplete_rows() {
        let cli = Cli::parse_from(["brave-cli", "query", "--input", "rust"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| Ok(Vec::new()),
            fixture_suggestions,
        )
        .expect("query suggest should succeed");

        let json: Value = serde_json::from_str(&output).expect("output should be json");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("suggest items should exist");
        let first_item = items.first().expect("direct-result item should exist");
        let second_item = items.get(1).expect("autocomplete item should exist");

        assert_eq!(
            first_item.get("title").and_then(Value::as_str),
            Some("Show Web Results: rust")
        );
        assert_eq!(
            first_item.get("arg").and_then(Value::as_str),
            Some("google-requery:search:rust")
        );
        assert_eq!(first_item.get("valid").and_then(Value::as_bool), Some(true));
        assert_eq!(
            second_item.get("autocomplete").and_then(Value::as_str),
            Some("res::rust")
        );
        assert_eq!(
            second_item.get("valid").and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn main_query_search_mode_routes_res_token_to_brave_search() {
        let cli = Cli::parse_from(["brave-cli", "query", "--input", "res::rust book"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, query| {
                assert_eq!(query, "rust book");
                Ok(vec![WebSearchResult {
                    title: "Rust Book".to_string(),
                    url: "https://doc.rust-lang.org/book/".to_string(),
                    description: "Official Rust guide".to_string(),
                }])
            },
            fixture_suggestions,
        )
        .expect("query search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output should be json");
        let first_item = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .expect("result item should exist");

        assert_eq!(
            first_item.get("title").and_then(Value::as_str),
            Some("Rust Book")
        );
        assert_eq!(
            first_item.get("arg").and_then(Value::as_str),
            Some("https://doc.rust-lang.org/book/")
        );
    }

    #[test]
    fn main_query_empty_input_returns_guidance_without_external_calls() {
        let cli = Cli::parse_from(["brave-cli", "query", "--input", "   "]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| Ok(Vec::new()),
            |_, _| Ok(Vec::new()),
        )
        .expect("query empty input should succeed");

        let json: Value = serde_json::from_str(&output).expect("output should be json");
        let first_item = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .expect("guidance item should exist");
        assert_eq!(
            first_item.get("title").and_then(Value::as_str),
            Some("Type a query for suggestions")
        );
        assert_eq!(
            first_item.get("valid").and_then(Value::as_bool),
            Some(false)
        );
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
            Some("NILS_BRAVE_001")
        );
        assert_eq!(
            json.get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str),
            Some("query must not be empty")
        );
        assert!(
            json.get("error")
                .and_then(|error| error.get("details"))
                .is_none()
        );
    }
}
