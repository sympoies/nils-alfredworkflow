use clap::{Parser, Subcommand};

use spotify_cli::{
    config::{ConfigError, RuntimeConfig},
    feedback,
    spotify_api::{self, SpotifyApiError, TrackSearchResult},
    spotify_auth::{self, SpotifyAccessToken, SpotifyAuthError},
};

use workflow_common::ScriptFilterOutputModeArg as OutputModeArg;
use workflow_common::{
    EnvelopePayloadKind, OutputMode, build_error_envelope, build_success_envelope,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Spotify workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Search Spotify tracks and print Alfred feedback JSON.
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

    fn from_spotify_auth(error: SpotifyAuthError) -> Self {
        match error {
            SpotifyAuthError::Http { status, message } => match status {
                400 | 401 | 403 => {
                    AppError::user(format!("spotify auth error ({status}): {message}"))
                }
                429 => AppError::runtime(format!("spotify auth rate limit (429): {message}")),
                500..=599 => {
                    AppError::runtime(format!("spotify auth unavailable ({status}): {message}"))
                }
                _ => AppError::runtime(format!("spotify auth error ({status}): {message}")),
            },
            SpotifyAuthError::Transport { .. } => {
                AppError::runtime("spotify auth request failed".to_string())
            }
            SpotifyAuthError::InvalidResponse(_) => {
                AppError::runtime("invalid spotify auth response".to_string())
            }
        }
    }

    fn from_spotify_api(error: SpotifyApiError) -> Self {
        match error {
            SpotifyApiError::Http { status, message } => match status {
                429 => AppError::runtime(format!("spotify api rate limit (429): {message}")),
                500..=599 => {
                    AppError::runtime(format!("spotify api unavailable ({status}): {message}"))
                }
                _ => AppError::runtime(format!("spotify api error ({status}): {message}")),
            },
            SpotifyApiError::Transport { .. } => {
                AppError::runtime("spotify api request failed".to_string())
            }
            SpotifyApiError::InvalidResponse(_) => {
                AppError::runtime("invalid spotify api response".to_string())
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
            ErrorKind::User => "NILS_SPOTIFY_001",
            ErrorKind::Runtime => "NILS_SPOTIFY_002",
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
        spotify_auth::request_access_token,
        spotify_api::search_tracks,
    )
}

fn run_with<LoadConfig, RequestToken, SearchTracks>(
    cli: Cli,
    load_config: LoadConfig,
    request_token: RequestToken,
    search_tracks: SearchTracks,
) -> Result<String, AppError>
where
    LoadConfig: Fn() -> Result<RuntimeConfig, ConfigError>,
    RequestToken: Fn(&RuntimeConfig) -> Result<SpotifyAccessToken, SpotifyAuthError>,
    SearchTracks: Fn(&RuntimeConfig, &str, &str) -> Result<Vec<TrackSearchResult>, SpotifyApiError>,
{
    match cli.command {
        Commands::Search { query, output } => {
            let query = query.trim();
            if query.is_empty() {
                return Err(AppError::user("query must not be empty"));
            }

            let config = load_config().map_err(AppError::from_config)?;
            let token = request_token(&config).map_err(AppError::from_spotify_auth)?;
            let tracks = search_tracks(&config, token.access_token.as_str(), query)
                .map_err(AppError::from_spotify_api)?;

            let payload = feedback::tracks_to_feedback(&tracks);
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
            client_id: "demo-client".to_string(),
            client_secret: "demo-secret".to_string(),
            max_results: 5,
            market: None,
        }
    }

    fn fixture_token() -> SpotifyAccessToken {
        SpotifyAccessToken {
            access_token: "demo-token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
        }
    }

    #[test]
    fn main_search_command_outputs_feedback_json_contract() {
        let cli = Cli::parse_from(["spotify-cli", "search", "--query", "daft punk"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_| Ok(fixture_token()),
            |_, _, _| {
                Ok(vec![TrackSearchResult {
                    name: "Harder, Better, Faster, Stronger".to_string(),
                    artists: vec!["Daft Punk".to_string()],
                    album_name: "Discovery".to_string(),
                    external_url: "https://open.spotify.com/track/abc123".to_string(),
                }])
            },
        )
        .expect("search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        let first_item = json
            .get("items")
            .and_then(|items| items.get(0))
            .expect("first item should exist");

        assert_eq!(
            first_item.get("title").and_then(Value::as_str),
            Some("Harder, Better, Faster, Stronger")
        );
        assert_eq!(
            first_item.get("subtitle").and_then(Value::as_str),
            Some("Daft Punk | Discovery")
        );
        assert_eq!(
            first_item.get("arg").and_then(Value::as_str),
            Some("https://open.spotify.com/track/abc123")
        );
    }

    #[test]
    fn main_search_service_json_mode_wraps_result_in_v1_envelope() {
        let cli = Cli::parse_from([
            "spotify-cli",
            "search",
            "--query",
            "daft punk",
            "--output",
            "json",
        ]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_| Ok(fixture_token()),
            |_, _, _| {
                Ok(vec![TrackSearchResult {
                    name: "Harder, Better, Faster, Stronger".to_string(),
                    artists: vec!["Daft Punk".to_string()],
                    album_name: "Discovery".to_string(),
                    external_url: "https://open.spotify.com/track/abc123".to_string(),
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
        let cli = Cli::parse_from(["spotify-cli", "search", "--query", "   "]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_| Ok(fixture_token()),
            |_, _, _| Ok(Vec::new()),
        )
        .expect_err("empty query should fail");

        assert_eq!(err.kind, ErrorKind::User);
        assert_eq!(err.message, "query must not be empty");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_surfaces_config_errors_with_user_exit_kind() {
        let cli = Cli::parse_from(["spotify-cli", "search", "--query", "daft punk"]);

        let err = run_with(
            cli,
            || Err(ConfigError::MissingClientId),
            |_| Ok(fixture_token()),
            |_, _, _| Ok(Vec::new()),
        )
        .expect_err("missing config should fail");

        assert_eq!(err.kind, ErrorKind::User);
        assert_eq!(err.message, "missing SPOTIFY_CLIENT_ID");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_maps_auth_invalid_client_to_user_error_kind() {
        let cli = Cli::parse_from(["spotify-cli", "search", "--query", "daft punk"]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_| {
                Err(SpotifyAuthError::Http {
                    status: 401,
                    message: "invalid_client".to_string(),
                })
            },
            |_, _, _| Ok(Vec::new()),
        )
        .expect_err("auth failures should fail");

        assert_eq!(err.kind, ErrorKind::User);
        assert_eq!(err.message, "spotify auth error (401): invalid_client");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_maps_auth_rate_limit_to_runtime_error_kind() {
        let cli = Cli::parse_from(["spotify-cli", "search", "--query", "daft punk"]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_| {
                Err(SpotifyAuthError::Http {
                    status: 429,
                    message: "retry later".to_string(),
                })
            },
            |_, _, _| Ok(Vec::new()),
        )
        .expect_err("auth failures should fail");

        assert_eq!(err.kind, ErrorKind::Runtime);
        assert_eq!(err.message, "spotify auth rate limit (429): retry later");
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn main_maps_api_unavailable_to_runtime_error_kind() {
        let cli = Cli::parse_from(["spotify-cli", "search", "--query", "daft punk"]);

        let err = run_with(
            cli,
            || Ok(fixture_config()),
            |_| Ok(fixture_token()),
            |_, _, _| {
                Err(SpotifyApiError::Http {
                    status: 503,
                    message: "service unavailable".to_string(),
                })
            },
        )
        .expect_err("api failures should fail");

        assert_eq!(err.kind, ErrorKind::Runtime);
        assert_eq!(
            err.message,
            "spotify api unavailable (503): service unavailable"
        );
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn main_help_flag_is_supported() {
        let help = Cli::try_parse_from(["spotify-cli", "--help"])
            .expect_err("help should exit through clap error");

        assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn main_service_error_envelope_has_required_error_fields() {
        let payload = serialize_service_error("search", &AppError::user("query must not be empty"));
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
            Some("NILS_SPOTIFY_001")
        );
        assert!(
            json.get("error")
                .and_then(|error| error.get("details"))
                .is_none()
        );
    }
}
