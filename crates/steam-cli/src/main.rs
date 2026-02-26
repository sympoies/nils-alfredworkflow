use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::Value;

use steam_cli::{
    config::{ConfigError, RuntimeConfig},
    feedback,
    steam_store_api::{self, SteamSearchResult, SteamStoreApiError},
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Steam workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Search Steam apps and print Alfred feedback JSON.
    Search {
        /// Search query text.
        #[arg(long)]
        query: String,
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputMode::Alfred)]
        mode: OutputMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
enum OutputMode {
    ServiceJson,
    Alfred,
}

impl Cli {
    fn command_name(&self) -> &'static str {
        match &self.command {
            Commands::Search { .. } => "search",
        }
    }

    fn output_mode(&self) -> OutputMode {
        match &self.command {
            Commands::Search { mode, .. } => *mode,
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

    fn from_steam_api(error: SteamStoreApiError) -> Self {
        match error {
            SteamStoreApiError::Http { status, message } => {
                AppError::runtime(format!("steam store api error ({status}): {message}"))
            }
            SteamStoreApiError::Transport { .. } => {
                AppError::runtime("steam store request failed".to_string())
            }
            SteamStoreApiError::InvalidResponse(_) => {
                AppError::runtime("invalid steam store response".to_string())
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
            ErrorKind::User => "steam.user",
            ErrorKind::Runtime => "steam.runtime",
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
                OutputMode::ServiceJson => {
                    println!("{}", serialize_service_error(command, &error));
                }
                OutputMode::Alfred => {
                    eprintln!("error: {}", error.message);
                }
            }
            std::process::exit(error.exit_code());
        }
    }
}

fn run(cli: Cli) -> Result<String, AppError> {
    run_with(cli, RuntimeConfig::from_env, steam_store_api::search_apps)
}

fn run_with<LoadConfig, SearchApps>(
    cli: Cli,
    load_config: LoadConfig,
    search_apps: SearchApps,
) -> Result<String, AppError>
where
    LoadConfig: Fn() -> Result<RuntimeConfig, ConfigError>,
    SearchApps: Fn(&RuntimeConfig, &str) -> Result<Vec<SteamSearchResult>, SteamStoreApiError>,
{
    match cli.command {
        Commands::Search { query, mode } => {
            let query = query.trim();
            if query.is_empty() {
                return Err(AppError::user("query must not be empty"));
            }

            let config = load_config().map_err(AppError::from_config)?;
            let results = search_apps(&config, query).map_err(AppError::from_steam_api)?;

            let payload = feedback::search_results_to_feedback(
                &config.region,
                query,
                &config.region_options,
                &config.language,
                &results,
            );
            render_feedback(mode, "search", payload)
        }
    }
}

#[derive(Debug, Serialize)]
struct ServiceErrorEnvelope {
    code: &'static str,
    message: String,
    details: Option<Value>,
}

#[derive(Debug, Serialize)]
struct ServiceEnvelope {
    schema_version: &'static str,
    command: &'static str,
    ok: bool,
    result: Option<Value>,
    error: Option<ServiceErrorEnvelope>,
}

fn render_feedback(
    mode: OutputMode,
    command: &'static str,
    payload: alfred_core::Feedback,
) -> Result<String, AppError> {
    match mode {
        OutputMode::Alfred => payload
            .to_json()
            .map_err(|err| AppError::runtime(format!("failed to serialize feedback: {err}"))),
        OutputMode::ServiceJson => {
            let result = serde_json::to_value(payload)
                .map_err(|err| AppError::runtime(format!("failed to serialize feedback: {err}")))?;
            serde_json::to_string(&ServiceEnvelope {
                schema_version: "v1",
                command,
                ok: true,
                result: Some(result),
                error: None,
            })
            .map_err(|err| {
                AppError::runtime(format!("failed to serialize service envelope: {err}"))
            })
        }
    }
}

fn serialize_service_error(command: &'static str, error: &AppError) -> String {
    let envelope = ServiceEnvelope {
        schema_version: "v1",
        command,
        ok: false,
        result: None,
        error: Some(ServiceErrorEnvelope {
            code: error.code(),
            message: error.message.clone(),
            details: None,
        }),
    };

    serde_json::to_string(&envelope).unwrap_or_else(|serialize_error| {
        serde_json::json!({
            "schema_version": "v1",
            "command": command,
            "ok": false,
            "result": Value::Null,
            "error": {
                "code": "internal.serialize",
                "message": format!("failed to serialize service error envelope: {serialize_error}"),
                "details": Value::Null,
            }
        })
        .to_string()
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use steam_cli::steam_store_api::{SteamPlatforms, SteamPrice};

    fn fixture_config() -> RuntimeConfig {
        RuntimeConfig {
            region: "us".to_string(),
            region_options: vec!["jp".to_string(), "us".to_string()],
            max_results: 5,
            language: "english".to_string(),
        }
    }

    #[test]
    fn main_search_command_outputs_feedback_json_contract() {
        let cli = Cli::parse_from(["steam-cli", "search", "--query", "counter strike"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Ok(vec![SteamSearchResult {
                    app_id: 730,
                    name: "Counter-Strike 2".to_string(),
                    price: Some(SteamPrice {
                        final_price_cents: Some(0),
                        final_formatted: Some("Free".to_string()),
                    }),
                    platforms: SteamPlatforms {
                        windows: true,
                        mac: false,
                        linux: true,
                    },
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
    }

    #[test]
    fn main_search_service_json_mode_wraps_result_in_v1_envelope() {
        let cli = Cli::parse_from([
            "steam-cli",
            "search",
            "--query",
            "counter strike",
            "--mode",
            "service-json",
        ]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| Ok(Vec::<SteamSearchResult>::new()),
        )
        .expect("search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("v1")
        );
        assert_eq!(json.get("command").and_then(Value::as_str), Some("search"));
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
        assert!(json.get("result").is_some());
    }

    #[test]
    fn main_search_rejects_empty_query_before_api_call() {
        let cli = Cli::parse_from(["steam-cli", "search", "--query", "   "]);

        let error = run_with(
            cli,
            || {
                panic!("config should not be loaded when query is empty");
            },
            |_, _| {
                panic!("api should not be called when query is empty");
            },
        )
        .expect_err("empty query must fail");

        assert_eq!(error.kind, ErrorKind::User);
        assert_eq!(error.message, "query must not be empty");
    }

    #[test]
    fn main_search_surfaces_config_errors_as_user_errors() {
        let cli = Cli::parse_from(["steam-cli", "search", "--query", "dota"]);

        let error = run_with(
            cli,
            || Err(ConfigError::InvalidRegion("USA".to_string())),
            |_, _| {
                panic!("api should not be called when config is invalid");
            },
        )
        .expect_err("invalid config should fail");

        assert_eq!(error.kind, ErrorKind::User);
        assert!(error.message.contains("invalid STEAM_REGION"));
    }

    #[test]
    fn main_search_surfaces_api_http_errors_as_runtime_errors() {
        let cli = Cli::parse_from(["steam-cli", "search", "--query", "dota"]);

        let error = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| {
                Err(SteamStoreApiError::Http {
                    status: 503,
                    message: "upstream unavailable".to_string(),
                })
            },
        )
        .expect_err("api failure should fail");

        assert_eq!(error.kind, ErrorKind::Runtime);
        assert_eq!(
            error.message,
            "steam store api error (503): upstream unavailable"
        );
    }

    #[test]
    fn serialize_service_error_emits_required_fields() {
        let payload = serialize_service_error("search", &AppError::user("query must not be empty"));
        let json: Value = serde_json::from_str(&payload).expect("payload must be valid JSON");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("v1")
        );
        assert_eq!(json.get("command").and_then(Value::as_str), Some("search"));
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            json.get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("steam.user")
        );
    }
}
