use clap::{Parser, Subcommand};

use steam_cli::{
    config::{ConfigError, RuntimeConfig},
    feedback,
    steam_store_api::{self, SteamSearchResult, SteamStoreApiError},
};

use workflow_common::ScriptFilterOutputModeArg as OutputModeArg;
use workflow_common::{
    EnvelopePayloadKind, OutputMode, build_error_envelope, build_success_envelope,
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
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
    /// List current Steam Store specials (discounted titles) as Alfred feedback JSON.
    Specials {
        /// Output mode: workflow-compatible Alfred JSON or service envelope JSON.
        #[arg(long, value_enum, default_value_t = OutputModeArg::AlfredJson)]
        output: OutputModeArg,
    },
}

impl Cli {
    fn command_name(&self) -> &'static str {
        match &self.command {
            Commands::Search { .. } => "search",
            Commands::Specials { .. } => "specials",
        }
    }

    fn output_mode(&self) -> OutputMode {
        match &self.command {
            Commands::Search { output, .. } => (*output).into(),
            Commands::Specials { output } => (*output).into(),
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
            ErrorKind::User => "NILS_STEAM_001",
            ErrorKind::Runtime => "NILS_STEAM_002",
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
        steam_store_api::search_apps,
        steam_store_api::fetch_specials,
    )
}

fn run_with<LoadConfig, SearchApps, FetchSpecials>(
    cli: Cli,
    load_config: LoadConfig,
    search_apps: SearchApps,
    fetch_specials: FetchSpecials,
) -> Result<String, AppError>
where
    LoadConfig: Fn() -> Result<RuntimeConfig, ConfigError>,
    SearchApps: Fn(&RuntimeConfig, &str) -> Result<Vec<SteamSearchResult>, SteamStoreApiError>,
    FetchSpecials: Fn(&RuntimeConfig) -> Result<Vec<SteamSearchResult>, SteamStoreApiError>,
{
    match cli.command {
        Commands::Search { query, output } => {
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
                config.show_region_options,
                &config.language,
                &results,
            );
            render_feedback(output.into(), "search", payload)
        }
        Commands::Specials { output } => {
            let config = load_config().map_err(AppError::from_config)?;
            let results = fetch_specials(&config).map_err(AppError::from_steam_api)?;

            let payload =
                feedback::specials_to_feedback(&config.region, &config.language, &results);
            render_feedback(output.into(), "specials", payload)
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
    use steam_cli::config::SteamSearchApi;
    use steam_cli::steam_store_api::{SteamItemType, SteamPlatforms, SteamPrice};

    fn fixture_config() -> RuntimeConfig {
        RuntimeConfig {
            region: "us".to_string(),
            region_options: vec!["jp".to_string(), "us".to_string()],
            show_region_options: true,
            max_results: 5,
            specials_max_results: 30,
            language: "english".to_string(),
            search_api: SteamSearchApi::SearchSuggestions,
            show_covers: false,
            cover_cache_dir: None,
        }
    }

    fn fixture_config_without_language() -> RuntimeConfig {
        RuntimeConfig {
            region: "us".to_string(),
            region_options: vec!["jp".to_string(), "us".to_string()],
            show_region_options: true,
            max_results: 5,
            specials_max_results: 30,
            language: String::new(),
            search_api: SteamSearchApi::SearchSuggestions,
            show_covers: false,
            cover_cache_dir: None,
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
                        ..SteamPrice::default()
                    }),
                    item_type: SteamItemType::Game,
                    platforms: SteamPlatforms {
                        windows: true,
                        mac: false,
                        linux: true,
                    },
                    image_url: None,
                    cover_path: None,
                }])
            },
            |_| panic!("specials must not be called for the search command"),
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
    fn main_search_command_omits_language_param_when_not_configured() {
        let cli = Cli::parse_from(["steam-cli", "search", "--query", "counter strike"]);

        let output = run_with(
            cli,
            || Ok(fixture_config_without_language()),
            |_, _| {
                Ok(vec![SteamSearchResult {
                    app_id: 730,
                    name: "Counter-Strike 2".to_string(),
                    price: Some(SteamPrice {
                        final_price_cents: Some(0),
                        final_formatted: Some("Free".to_string()),
                        ..SteamPrice::default()
                    }),
                    item_type: SteamItemType::Game,
                    platforms: SteamPlatforms {
                        windows: true,
                        mac: false,
                        linux: true,
                    },
                    image_url: None,
                    cover_path: None,
                }])
            },
            |_| panic!("specials must not be called for the search command"),
        )
        .expect("search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be array");

        assert_eq!(
            items[3].get("arg").and_then(Value::as_str),
            Some("https://store.steampowered.com/app/730/?cc=us")
        );
    }

    #[test]
    fn main_search_service_json_mode_wraps_result_in_v1_envelope() {
        let cli = Cli::parse_from([
            "steam-cli",
            "search",
            "--query",
            "counter strike",
            "--output",
            "json",
        ]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| Ok(Vec::<SteamSearchResult>::new()),
            |_| panic!("specials must not be called for the search command"),
        )
        .expect("search should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
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
            |_| panic!("specials must not be called when query is empty"),
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
            |_| panic!("specials must not be called when config is invalid"),
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
            |_| panic!("specials must not be called for the search command"),
        )
        .expect_err("api failure should fail");

        assert_eq!(error.kind, ErrorKind::Runtime);
        assert_eq!(
            error.message,
            "steam store api error (503): upstream unavailable"
        );
    }

    #[test]
    fn main_specials_command_outputs_sorted_feedback() {
        let cli = Cli::parse_from(["steam-cli", "specials"]);

        let output = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| panic!("search must not be called for the specials command"),
            |_| {
                Ok(vec![
                    SteamSearchResult {
                        app_id: 1,
                        name: "Light".to_string(),
                        price: Some(SteamPrice {
                            final_price_cents: Some(7500),
                            final_formatted: Some("NT$ 75".to_string()),
                            original_price_cents: Some(10000),
                            original_formatted: Some("NT$ 100".to_string()),
                            discount_percent: Some(25),
                        }),
                        item_type: SteamItemType::Game,
                        platforms: SteamPlatforms::default(),
                        image_url: None,
                        cover_path: None,
                    },
                    SteamSearchResult {
                        app_id: 2,
                        name: "Heavy".to_string(),
                        price: Some(SteamPrice {
                            final_price_cents: Some(3400),
                            final_formatted: Some("NT$ 340".to_string()),
                            original_price_cents: Some(8500),
                            original_formatted: Some("NT$ 850".to_string()),
                            discount_percent: Some(60),
                        }),
                        item_type: SteamItemType::Game,
                        platforms: SteamPlatforms::default(),
                        image_url: None,
                        cover_path: None,
                    },
                ])
            },
        )
        .expect("specials should succeed");

        let json: Value = serde_json::from_str(&output).expect("output must be JSON");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be array");

        assert_eq!(items[0].get("title").and_then(Value::as_str), Some("Heavy"));
        assert_eq!(items[1].get("title").and_then(Value::as_str), Some("Light"));
    }

    #[test]
    fn main_specials_surfaces_api_errors_as_runtime_errors() {
        let cli = Cli::parse_from(["steam-cli", "specials"]);

        let error = run_with(
            cli,
            || Ok(fixture_config()),
            |_, _| panic!("search must not be called for the specials command"),
            |_| {
                Err(SteamStoreApiError::Http {
                    status: 503,
                    message: "upstream unavailable".to_string(),
                })
            },
        )
        .expect_err("api failure should fail");

        assert_eq!(error.kind, ErrorKind::Runtime);
    }

    #[test]
    fn serialize_service_error_emits_required_fields() {
        let payload = serialize_service_error("search", &AppError::user("query must not be empty"));
        let json: Value = serde_json::from_str(&payload).expect("payload must be valid JSON");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
        );
        assert_eq!(json.get("command").and_then(Value::as_str), Some("search"));
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            json.get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str),
            Some("NILS_STEAM_001")
        );
    }
}
