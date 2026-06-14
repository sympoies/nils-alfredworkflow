use std::time::Duration;

use alfred_core::Feedback;
use clap::{Parser, Subcommand, ValueEnum};

use bangumi_cli::{
    bangumi_api::{self, BangumiApiError, BangumiSubject},
    config::{ConfigError, RuntimeConfig},
    feedback,
    image_cache::{self, ImageCacheManager},
    input::{self, ParsedInput, SubjectType},
};
use workflow_common::ScriptFilterOutputModeArg as OutputModeArg;
use workflow_common::http::build_blocking_client;
use workflow_common::{
    AppError, EnvelopePayloadKind, OutputMode, build_error_envelope, build_success_envelope,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Bangumi workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Parse workflow query grammar and print Alfred feedback JSON.
    Query {
        /// Raw workflow input string, supports [type] query and type query.
        #[arg(long)]
        input: String,
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
    /// Explicit typed search entrypoint for non-Alfred callers.
    Search {
        /// Search keyword text.
        #[arg(long)]
        query: String,
        /// Explicit Bangumi subject type.
        #[arg(long = "type", value_enum, default_value_t = SubjectTypeArg::All)]
        subject_type: SubjectTypeArg,
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
enum SubjectTypeArg {
    All,
    Book,
    Anime,
    Music,
    Game,
    Real,
}

impl SubjectTypeArg {
    const fn into_subject_type(self) -> SubjectType {
        match self {
            Self::All => SubjectType::All,
            Self::Book => SubjectType::Book,
            Self::Anime => SubjectType::Anime,
            Self::Music => SubjectType::Music,
            Self::Game => SubjectType::Game,
            Self::Real => SubjectType::Real,
        }
    }
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

    fn parsed_input(&self) -> Result<ParsedInput, AppError> {
        match &self.command {
            Commands::Query { input, .. } => input::parse_query_input(input).map_err(from_input),
            Commands::Search {
                query,
                subject_type,
                ..
            } => {
                let keyword = query.trim();
                if keyword.is_empty() {
                    return Err(AppError::user(ERROR_CODE_USER, "query must not be empty"));
                }

                Ok(ParsedInput {
                    subject_type: subject_type.into_subject_type(),
                    keyword: keyword.to_string(),
                })
            }
        }
    }
}

const ERROR_CODE_USER: &str = "NILS_BANGUMI_001";
const ERROR_CODE_RUNTIME: &str = "NILS_BANGUMI_002";

fn from_config(error: ConfigError) -> AppError {
    AppError::user(ERROR_CODE_USER, error.to_string())
}

fn from_input(error: input::InputError) -> AppError {
    AppError::user(ERROR_CODE_USER, error.to_string())
}

fn from_bangumi_api(error: BangumiApiError) -> AppError {
    match error {
        BangumiApiError::Http { status, message } => AppError::runtime(
            ERROR_CODE_RUNTIME,
            format!("bangumi api error ({status}): {message}"),
        ),
        BangumiApiError::Transport { .. } => {
            AppError::runtime(ERROR_CODE_RUNTIME, "bangumi api request failed")
        }
        BangumiApiError::InvalidResponse { .. } => {
            AppError::runtime(ERROR_CODE_RUNTIME, "invalid bangumi api response")
        }
        BangumiApiError::InvalidLegacyUrl { .. } => AppError::runtime(
            ERROR_CODE_RUNTIME,
            "legacy bangumi endpoint url build failed",
        ),
        BangumiApiError::Fallback { .. } => {
            AppError::runtime(ERROR_CODE_RUNTIME, error.to_string())
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
    run_with(cli, RuntimeConfig::from_env, execute_search)
}

fn run_with<LoadConfig, Execute>(
    cli: Cli,
    load_config: LoadConfig,
    execute: Execute,
) -> Result<String, AppError>
where
    LoadConfig: Fn() -> Result<RuntimeConfig, ConfigError>,
    Execute: Fn(&RuntimeConfig, &ParsedInput) -> Result<Feedback, BangumiApiError>,
{
    let command = cli.command_name();
    let mode = cli.output_mode();
    let parsed_input = cli.parsed_input()?;
    let config = load_config().map_err(from_config)?;

    let payload = execute(&config, &parsed_input).map_err(from_bangumi_api)?;
    render_feedback(mode, command, payload)
}

fn execute_search(
    config: &RuntimeConfig,
    parsed_input: &ParsedInput,
) -> Result<Feedback, BangumiApiError> {
    let subjects = bangumi_api::search_subjects(config, parsed_input)?;
    Ok(render_feedback_with_cache(
        config,
        &subjects,
        parsed_input.subject_type,
    ))
}

fn render_feedback_with_cache(
    config: &RuntimeConfig,
    subjects: &[BangumiSubject],
    requested_type: SubjectType,
) -> Feedback {
    let cache = ImageCacheManager::new(config);

    let client = build_blocking_client(None, Some(Duration::from_millis(config.timeout_ms)));

    match client {
        Ok(client) => feedback::subjects_to_feedback_with_icons(
            subjects,
            requested_type,
            Some(&cache),
            |url| image_cache::download_image_bytes(&client, config, url),
        ),
        Err(_) => feedback::subjects_to_feedback(subjects, requested_type),
    }
}

fn render_feedback(
    mode: OutputMode,
    command: &'static str,
    payload: Feedback,
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
    use bangumi_cli::{bangumi_api::SubjectImages, config::ApiFallbackPolicy};

    fn fixture_config() -> RuntimeConfig {
        RuntimeConfig {
            api_key: None,
            max_results: 10,
            timeout_ms: 8_000,
            user_agent: "nils-bangumi-cli/1.1 (+https://github.com/sympoies/nils-alfredworkflow)"
                .to_string(),
            cache_dir: std::path::PathBuf::from("/tmp/bangumi-cli-cache"),
            image_cache_ttl_seconds: 86_400,
            image_cache_max_bytes: 128 * 1024 * 1024,
            api_fallback: ApiFallbackPolicy::Auto,
        }
    }

    fn fixture_subject() -> BangumiSubject {
        BangumiSubject {
            id: 2782,
            subject_type: Some(SubjectType::Anime),
            name: "Cowboy Bebop".to_string(),
            name_cn: Some("星際牛仔".to_string()),
            summary: Some("Space western classic".to_string()),
            url: "https://bgm.tv/subject/2782".to_string(),
            rank: Some(1),
            score: Some(9.0),
            images: SubjectImages::default(),
        }
    }

    #[test]
    fn main_query_command_outputs_feedback_json_contract() {
        let cli = Cli::parse_from(["bangumi-cli", "query", "--input", "anime naruto"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Ok(feedback::subjects_to_feedback(
                    &[fixture_subject()],
                    SubjectType::Anime,
                ))
            },
        )
        .expect("query should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        let first_item = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .expect("first item should exist");

        assert_eq!(
            first_item.get("title").and_then(Value::as_str),
            Some("Cowboy Bebop")
        );
        assert_eq!(
            first_item.get("arg").and_then(Value::as_str),
            Some("https://bgm.tv/subject/2782")
        );
    }

    #[test]
    fn main_search_service_json_mode_wraps_result_in_v1_envelope() {
        let cli = Cli::parse_from([
            "bangumi-cli",
            "search",
            "--query",
            "naruto",
            "--type",
            "anime",
            "--output",
            "json",
        ]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Ok(feedback::subjects_to_feedback(
                    &[fixture_subject()],
                    SubjectType::Anime,
                ))
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
        let cli = Cli::parse_from(["bangumi-cli", "query", "--input", "   "]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| Ok(Feedback::new(Vec::new())),
        )
        .expect_err("empty query should fail");

        assert_eq!(err.kind(), CliErrorKind::User);
        assert_eq!(err.message(), "query must not be empty");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_surfaces_config_errors_with_user_exit_kind() {
        let cli = Cli::parse_from(["bangumi-cli", "search", "--query", "naruto"]);

        let err = run_with(
            cli,
            || Err(ConfigError::InvalidMaxResults("abc".to_string())),
            |_, _| Ok(Feedback::new(Vec::new())),
        )
        .expect_err("config errors should fail");

        assert_eq!(err.kind(), CliErrorKind::User);
        assert_eq!(err.message(), "invalid BANGUMI_MAX_RESULTS: abc");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_maps_http_api_failures_to_runtime_error_kind() {
        let cli = Cli::parse_from(["bangumi-cli", "search", "--query", "naruto"]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Err(BangumiApiError::Http {
                    status: 503,
                    message: "service unavailable".to_string(),
                })
            },
        )
        .expect_err("api errors should fail");

        assert_eq!(err.kind(), CliErrorKind::Runtime);
        assert_eq!(
            err.message(),
            "bangumi api error (503): service unavailable"
        );
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn main_help_flag_is_supported() {
        let help = Cli::try_parse_from(["bangumi-cli", "--help"])
            .expect_err("help should exit through clap error");

        assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn main_service_error_envelope_has_required_error_fields() {
        let payload = serialize_service_error(
            "query",
            &AppError::user(ERROR_CODE_USER, "query must not be empty"),
        );
        let json: Value = serde_json::from_str(&payload).expect("service error should be json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(json.get("command").and_then(Value::as_str), Some("query"));
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
        assert!(json.get("result").is_none());
        assert_eq!(
            json.get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("NILS_BANGUMI_001")
        );
        assert!(
            json.get("error")
                .and_then(|error| error.get("details"))
                .is_none()
        );
    }
}
