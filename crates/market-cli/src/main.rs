use alfred_core::{Feedback, Item};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use rust_decimal::Decimal;
use serde_json::json;
use workflow_common::{
    EnvelopePayloadKind, OutputMode, build_alfred_error_feedback, build_error_details_json,
    build_error_envelope, build_success_envelope, redact_sensitive, select_output_mode,
};

use market_cli::{
    config::RuntimeConfig,
    error::AppError,
    expression,
    model::{MarketKind, MarketRequest, normalize_fx_symbol},
    parse_favorites_list,
    providers::{HttpProviders, ProviderApi},
    service,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "FX + crypto market data CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Query fiat exchange rate (Frankfurter).
    Fx {
        #[arg(long)]
        base: String,
        #[arg(long)]
        quote: String,
        #[arg(long)]
        amount: String,
        #[arg(long, value_enum)]
        output: Option<OutputModeArg>,
        #[arg(long)]
        json: bool,
    },
    /// Query crypto spot price (Coinbase with Kraken fallback).
    Crypto {
        #[arg(long)]
        base: String,
        #[arg(long)]
        quote: String,
        #[arg(long)]
        amount: String,
        #[arg(long, value_enum)]
        output: Option<OutputModeArg>,
        #[arg(long)]
        json: bool,
    },
    /// Evaluate market expressions and return Alfred Script Filter JSON.
    Expr {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "USD")]
        default_fiat: String,
        #[arg(long, value_enum)]
        output: Option<OutputModeArg>,
        #[arg(long)]
        json: bool,
    },
    /// Render configured market favorites as non-actionable Alfred rows.
    Favorites {
        #[arg(long)]
        list: Option<String>,
        #[arg(long, default_value = "USD")]
        default_fiat: String,
        #[arg(long, value_enum)]
        output: Option<OutputModeArg>,
        #[arg(long)]
        json: bool,
    },
}

const ERROR_CODE_USER_INVALID_INPUT: &str = "user.invalid_input";
const ERROR_CODE_USER_OUTPUT_MODE_CONFLICT: &str = "user.output_mode_conflict";
const ERROR_CODE_RUNTIME_PROVIDER_INIT: &str = "runtime.provider_init_failed";
const ERROR_CODE_RUNTIME_PROVIDER_FAILED: &str = "runtime.provider_failed";
const ERROR_CODE_RUNTIME_SERIALIZE: &str = "runtime.serialize_failed";
const FAVORITES_PROMPT_TITLE: &str = "Enter a market expression";
const FAVORITES_PROMPT_EXAMPLE: &str = "Example: 1 BTC + 3 ETH to JPY";
const FAVORITES_QUOTE_UNAVAILABLE_SUBTITLE: &str =
    "Favorite symbol. Type an expression to convert. Quote unavailable.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputModeArg {
    Human,
    Json,
    AlfredJson,
}

impl From<OutputModeArg> for OutputMode {
    fn from(value: OutputModeArg) -> Self {
        match value {
            OutputModeArg::Human => OutputMode::Human,
            OutputModeArg::Json => OutputMode::Json,
            OutputModeArg::AlfredJson => OutputMode::AlfredJson,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliError {
    kind: market_cli::error::ErrorKind,
    code: &'static str,
    message: String,
}

impl CliError {
    fn user(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: market_cli::error::ErrorKind::User,
            code,
            message: message.into(),
        }
    }

    fn runtime(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: market_cli::error::ErrorKind::Runtime,
            code,
            message: message.into(),
        }
    }

    fn exit_code(&self) -> i32 {
        match self.kind {
            market_cli::error::ErrorKind::User => 2,
            market_cli::error::ErrorKind::Runtime => 1,
        }
    }
}

impl Cli {
    fn command_name(&self) -> &'static str {
        match &self.command {
            Commands::Fx { .. } => "market.fx",
            Commands::Crypto { .. } => "market.crypto",
            Commands::Expr { .. } => "market.expr",
            Commands::Favorites { .. } => "market.favorites",
        }
    }

    fn output_mode_hint(&self) -> OutputMode {
        match &self.command {
            Commands::Fx { output, json, .. } | Commands::Crypto { output, json, .. } => {
                if *json {
                    OutputMode::Json
                } else if let Some(explicit) = output {
                    (*explicit).into()
                } else {
                    OutputMode::Human
                }
            }
            Commands::Expr { output, json, .. } => {
                if *json {
                    OutputMode::Json
                } else if let Some(explicit) = output {
                    (*explicit).into()
                } else {
                    OutputMode::AlfredJson
                }
            }
            Commands::Favorites { output, json, .. } => {
                if *json {
                    OutputMode::Json
                } else if let Some(explicit) = output {
                    (*explicit).into()
                } else {
                    OutputMode::AlfredJson
                }
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let command = cli.command_name();
    let output_mode = cli.output_mode_hint();
    match run(cli) {
        Ok(output) => {
            println!("{output}");
        }
        Err(error) => {
            emit_error(command, output_mode, &error);
            std::process::exit(error.exit_code());
        }
    }
}

fn run(cli: Cli) -> Result<String, CliError> {
    let config = RuntimeConfig::from_env();
    let providers = HttpProviders::new()
        .map_err(|error| runtime_error(ERROR_CODE_RUNTIME_PROVIDER_INIT, error.to_string()))?;
    run_with(cli, &config, &providers, Utc::now)
}

fn run_with<P, N>(
    cli: Cli,
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
) -> Result<String, CliError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    match cli.command {
        Commands::Fx {
            base,
            quote,
            amount,
            output,
            json,
        } => run_market_command(
            config,
            providers,
            now_fn,
            MarketCommandArgs {
                command: "market.fx",
                kind: MarketKind::Fx,
                base: &base,
                quote: &quote,
                amount: &amount,
                output,
                json_flag: json,
                default_mode: OutputMode::Human,
            },
        ),
        Commands::Crypto {
            base,
            quote,
            amount,
            output,
            json,
        } => run_market_command(
            config,
            providers,
            now_fn,
            MarketCommandArgs {
                command: "market.crypto",
                kind: MarketKind::Crypto,
                base: &base,
                quote: &quote,
                amount: &amount,
                output,
                json_flag: json,
                default_mode: OutputMode::Human,
            },
        ),
        Commands::Expr {
            query,
            default_fiat,
            output,
            json,
        } => {
            let feedback =
                expression::evaluate_query(config, providers, now_fn, &query, &default_fiat)
                    .map_err(map_app_error)?;
            let output_mode =
                select_output_mode(output.map(Into::into), json, OutputMode::AlfredJson).map_err(
                    |error| user_error(ERROR_CODE_USER_OUTPUT_MODE_CONFLICT, error.to_string()),
                )?;
            let alfred_json = feedback.to_json().map_err(|error| {
                runtime_error(
                    ERROR_CODE_RUNTIME_SERIALIZE,
                    format!("failed to serialize Alfred feedback: {error}"),
                )
            })?;

            match output_mode {
                OutputMode::AlfredJson => Ok(alfred_json),
                OutputMode::Json => Ok(build_success_envelope(
                    "market.expr",
                    EnvelopePayloadKind::Result,
                    &alfred_json,
                )),
                OutputMode::Human => format_expr_human_output(&alfred_json),
            }
        }
        Commands::Favorites {
            list,
            default_fiat,
            output,
            json,
        } => {
            let symbols = parse_favorites_list(list.as_deref(), &default_fiat)
                .map_err(|error| user_error(ERROR_CODE_USER_INVALID_INPUT, error.to_string()))?;
            let default_fiat = normalize_fx_symbol(&default_fiat, "default_fiat")
                .map_err(|error| user_error(ERROR_CODE_USER_INVALID_INPUT, error.to_string()))?;
            let output_mode =
                select_output_mode(output.map(Into::into), json, OutputMode::AlfredJson).map_err(
                    |error| user_error(ERROR_CODE_USER_OUTPUT_MODE_CONFLICT, error.to_string()),
                )?;

            match output_mode {
                OutputMode::Human => Ok(format_favorites_human_output(&symbols)),
                OutputMode::AlfredJson | OutputMode::Json => {
                    let alfred_json = render_favorites_alfred_output(
                        config,
                        providers,
                        now_fn,
                        &symbols,
                        &default_fiat,
                    )?;

                    match output_mode {
                        OutputMode::AlfredJson => Ok(alfred_json),
                        OutputMode::Json => Ok(build_success_envelope(
                            "market.favorites",
                            EnvelopePayloadKind::Result,
                            &alfred_json,
                        )),
                        OutputMode::Human => unreachable!("handled above"),
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MarketCommandArgs<'a> {
    command: &'static str,
    kind: MarketKind,
    base: &'a str,
    quote: &'a str,
    amount: &'a str,
    output: Option<OutputModeArg>,
    json_flag: bool,
    default_mode: OutputMode,
}

fn run_market_command<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    args: MarketCommandArgs<'_>,
) -> Result<String, CliError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc>,
{
    let output_mode = select_output_mode(
        args.output.map(Into::into),
        args.json_flag,
        args.default_mode,
    )
    .map_err(|error| user_error(ERROR_CODE_USER_OUTPUT_MODE_CONFLICT, error.to_string()))?;
    let request = MarketRequest::new(args.kind, args.base, args.quote, args.amount)
        .map_err(|error| user_error(ERROR_CODE_USER_INVALID_INPUT, error.to_string()))?;
    let result =
        service::resolve_market(config, providers, now_fn, &request).map_err(map_app_error)?;

    match output_mode {
        OutputMode::Json => {
            let raw = serde_json::to_string(&result).map_err(|error| {
                runtime_error(
                    ERROR_CODE_RUNTIME_SERIALIZE,
                    format!("failed to serialize output: {error}"),
                )
            })?;
            Ok(build_success_envelope(
                args.command,
                EnvelopePayloadKind::Result,
                &raw,
            ))
        }
        OutputMode::Human => Ok(format_market_human_output(&result)),
        OutputMode::AlfredJson => render_market_alfred_output(&result),
    }
}

fn format_market_human_output(output: &market_cli::model::MarketOutput) -> String {
    format!(
        "{} {} {} -> {} {} (price={} provider={} cache={})",
        output.kind.as_str().to_ascii_uppercase(),
        output.amount,
        output.base,
        output.converted,
        output.quote,
        output.unit_price,
        output.provider,
        cache_status_label(output.cache.status),
    )
}

fn render_market_alfred_output(
    output: &market_cli::model::MarketOutput,
) -> Result<String, CliError> {
    let payload = json!({
        "items": [{
            "title": format!("{} {} = {} {}", output.amount, output.base, output.converted, output.quote),
            "subtitle": format!(
                "price={} provider={} cache={}",
                output.unit_price,
                output.provider,
                cache_status_label(output.cache.status)
            ),
            "arg": output.converted,
            "valid": false
        }]
    });

    serde_json::to_string(&payload).map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to serialize Alfred output: {error}"),
        )
    })
}

fn format_favorites_human_output(symbols: &[String]) -> String {
    format!("favorites: {}", symbols.join(", "))
}

fn render_favorites_alfred_output<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    symbols: &[String],
    default_fiat: &str,
) -> Result<String, CliError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    let mut items = Vec::with_capacity(symbols.len() + 1);
    items.push(
        Item::new(FAVORITES_PROMPT_TITLE)
            .with_uid("market-favorites-prompt")
            .with_subtitle(format!(
                "{FAVORITES_PROMPT_EXAMPLE} (default fiat: {default_fiat})"
            ))
            .with_valid(false),
    );

    for symbol in symbols {
        items.push(build_favorite_quote_item(
            config,
            providers,
            now_fn,
            symbol,
            default_fiat,
        ));
    }

    Feedback::new(items).to_json().map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to serialize favorites Alfred output: {error}"),
        )
    })
}

fn build_favorite_quote_item<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    symbol: &str,
    default_fiat: &str,
) -> Item
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    if symbol == default_fiat {
        return Item::new(format!("1 {symbol} = 1 {default_fiat}"))
            .with_uid(favorite_item_uid(symbol, default_fiat))
            .with_subtitle("provider: identity · freshness: fixed")
            .with_valid(false);
    }

    match expression::resolve_symbol_output(config, providers, now_fn, symbol, default_fiat) {
        Ok(output) => favorite_quote_success_item(symbol, default_fiat, &output),
        Err(_) => Item::new(symbol)
            .with_uid(favorite_item_uid(symbol, default_fiat))
            .with_subtitle(FAVORITES_QUOTE_UNAVAILABLE_SUBTITLE)
            .with_valid(false),
    }
}

fn favorite_quote_success_item(
    symbol: &str,
    default_fiat: &str,
    output: &market_cli::model::MarketOutput,
) -> Item {
    let rendered_price = output
        .unit_price
        .parse::<Decimal>()
        .map(expression::format_market_decimal)
        .unwrap_or_else(|_| output.unit_price.clone());

    Item::new(format!("1 {symbol} = {rendered_price} {default_fiat}"))
        .with_uid(favorite_item_uid(symbol, default_fiat))
        .with_subtitle(format!(
            "provider: {} · freshness: {}",
            output.provider,
            cache_status_label(output.cache.status)
        ))
        .with_valid(false)
}

fn favorite_item_uid(symbol: &str, default_fiat: &str) -> String {
    format!(
        "market-favorite-{}-{}",
        symbol.to_ascii_lowercase(),
        default_fiat.to_ascii_lowercase()
    )
}

fn format_expr_human_output(alfred_json: &str) -> Result<String, CliError> {
    let parsed: serde_json::Value = serde_json::from_str(alfred_json).map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to parse Alfred payload: {error}"),
        )
    })?;

    let title = parsed
        .get("items")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("title"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let subtitle = parsed
        .get("items")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("subtitle"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    if subtitle.is_empty() {
        Ok(title.to_string())
    } else {
        Ok(format!("{title} | {subtitle}"))
    }
}

fn emit_error(command: &str, output_mode: OutputMode, error: &CliError) {
    match output_mode {
        OutputMode::Json => {
            let details = build_error_details_json(error_kind_label(error.kind), error.exit_code());
            println!(
                "{}",
                build_error_envelope(command, error.code, &error.message, Some(&details))
            );
        }
        OutputMode::AlfredJson => {
            println!(
                "{}",
                build_alfred_error_feedback(error.code, &error.message)
            );
        }
        OutputMode::Human => {
            eprintln!(
                "error[{}]: {}",
                error.code,
                redact_sensitive(&error.message)
            );
        }
    }
}

fn user_error(code: &'static str, message: impl Into<String>) -> CliError {
    CliError::user(code, message)
}

fn runtime_error(code: &'static str, message: impl Into<String>) -> CliError {
    CliError::runtime(code, message)
}

fn map_app_error(error: AppError) -> CliError {
    match error.kind {
        market_cli::error::ErrorKind::User => {
            user_error(ERROR_CODE_USER_INVALID_INPUT, error.message)
        }
        market_cli::error::ErrorKind::Runtime => {
            runtime_error(ERROR_CODE_RUNTIME_PROVIDER_FAILED, error.message)
        }
    }
}

fn error_kind_label(kind: market_cli::error::ErrorKind) -> &'static str {
    match kind {
        market_cli::error::ErrorKind::User => "user",
        market_cli::error::ErrorKind::Runtime => "runtime",
    }
}

fn cache_status_label(status: market_cli::model::CacheStatus) -> &'static str {
    match status {
        market_cli::model::CacheStatus::Live => "live",
        market_cli::model::CacheStatus::CacheFresh => "cache_fresh",
        market_cli::model::CacheStatus::CacheStaleFallback => "cache_stale_fallback",
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use market_cli::{model::MarketQuote, providers::ProviderError};
    use serde_json::Value;

    use super::*;

    struct FakeProviders {
        fx_result: Result<MarketQuote, ProviderError>,
        crypto_coinbase_result: Result<MarketQuote, ProviderError>,
        crypto_kraken_result: Result<MarketQuote, ProviderError>,
    }

    impl FakeProviders {
        fn ok() -> Self {
            let now = Utc
                .with_ymd_and_hms(2026, 2, 10, 12, 0, 0)
                .single()
                .expect("time");
            Self {
                fx_result: Ok(MarketQuote::new(
                    "frankfurter",
                    rust_decimal::Decimal::new(321, 1),
                    now,
                )),
                crypto_coinbase_result: Ok(MarketQuote::new(
                    "coinbase",
                    rust_decimal::Decimal::new(670001, 1),
                    now,
                )),
                crypto_kraken_result: Ok(MarketQuote::new(
                    "kraken",
                    rust_decimal::Decimal::new(670000, 1),
                    now,
                )),
            }
        }
    }

    impl ProviderApi for FakeProviders {
        fn fetch_fx_rate(&self, _base: &str, _quote: &str) -> Result<MarketQuote, ProviderError> {
            self.fx_result.clone()
        }

        fn fetch_crypto_coinbase(
            &self,
            _base: &str,
            _quote: &str,
        ) -> Result<MarketQuote, ProviderError> {
            self.crypto_coinbase_result.clone()
        }

        fn fetch_crypto_kraken(
            &self,
            _base: &str,
            _quote: &str,
        ) -> Result<MarketQuote, ProviderError> {
            self.crypto_kraken_result.clone()
        }
    }

    struct FavoritesProviders;

    impl ProviderApi for FavoritesProviders {
        fn fetch_fx_rate(&self, base: &str, quote: &str) -> Result<MarketQuote, ProviderError> {
            let now = Utc
                .with_ymd_and_hms(2026, 2, 10, 12, 0, 0)
                .single()
                .expect("time");

            match (base, quote) {
                ("JPY", "USD") => Ok(MarketQuote::new(
                    "frankfurter",
                    rust_decimal::Decimal::new(67, 4),
                    now,
                )),
                _ => Err(ProviderError::UnsupportedPair(format!("{base}/{quote}"))),
            }
        }

        fn fetch_crypto_coinbase(
            &self,
            base: &str,
            quote: &str,
        ) -> Result<MarketQuote, ProviderError> {
            let now = Utc
                .with_ymd_and_hms(2026, 2, 10, 12, 0, 0)
                .single()
                .expect("time");

            match (base, quote) {
                ("BTC", "USD") => Ok(MarketQuote::new(
                    "coinbase",
                    rust_decimal::Decimal::new(68194, 0),
                    now,
                )),
                ("ETH", "USD") => Ok(MarketQuote::new(
                    "coinbase",
                    rust_decimal::Decimal::new(1980, 0),
                    now,
                )),
                _ => Err(ProviderError::UnsupportedPair(format!("{base}/{quote}"))),
            }
        }

        fn fetch_crypto_kraken(
            &self,
            _base: &str,
            _quote: &str,
        ) -> Result<MarketQuote, ProviderError> {
            Err(ProviderError::Transport(
                "kraken disabled in tests".to_string(),
            ))
        }
    }

    fn config_in_tempdir() -> RuntimeConfig {
        RuntimeConfig {
            cache_dir: tempfile::tempdir().expect("tempdir").path().to_path_buf(),
            fx_cache_ttl_secs: market_cli::config::FX_TTL_SECS,
            crypto_cache_ttl_secs: market_cli::config::CRYPTO_TTL_SECS,
        }
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 10, 12, 5, 0)
            .single()
            .expect("time")
    }

    #[test]
    fn main_outputs_fx_json_contract() {
        let cli = Cli::parse_from([
            "market-cli",
            "fx",
            "--base",
            "USD",
            "--quote",
            "TWD",
            "--amount",
            "100",
            "--json",
        ]);

        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("fx should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("v1")
        );
        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("market.fx")
        );
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("kind"))
                .and_then(Value::as_str),
            Some("fx")
        );
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("base"))
                .and_then(Value::as_str),
            Some("USD")
        );
        assert!(
            json.get("result")
                .and_then(|result| result.get("cache"))
                .is_some()
        );
    }

    #[test]
    fn main_outputs_crypto_json_contract() {
        let cli = Cli::parse_from([
            "market-cli",
            "crypto",
            "--base",
            "BTC",
            "--quote",
            "USD",
            "--amount",
            "0.5",
            "--json",
        ]);

        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("crypto should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("kind"))
                .and_then(Value::as_str),
            Some("crypto")
        );
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("provider"))
                .and_then(Value::as_str),
            Some("coinbase")
        );
        assert!(
            json.get("result")
                .and_then(|result| result.get("converted"))
                .is_some()
        );
    }

    #[test]
    fn main_maps_invalid_symbols_to_user_error() {
        let cli = Cli::parse_from([
            "market-cli",
            "fx",
            "--base",
            "USDT",
            "--quote",
            "TWD",
            "--amount",
            "100",
        ]);

        let err = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect_err("must fail");
        assert_eq!(err.kind, market_cli::error::ErrorKind::User);
        assert_eq!(err.code, ERROR_CODE_USER_INVALID_INPUT);
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_maps_runtime_provider_failure() {
        let cli = Cli::parse_from([
            "market-cli",
            "crypto",
            "--base",
            "BTC",
            "--quote",
            "USD",
            "--amount",
            "1",
        ]);

        let providers = FakeProviders {
            crypto_coinbase_result: Err(ProviderError::Transport("timeout".to_string())),
            crypto_kraken_result: Err(ProviderError::Http {
                status: 503,
                message: "down".to_string(),
            }),
            ..FakeProviders::ok()
        };

        let err =
            run_with(cli, &config_in_tempdir(), &providers, fixed_now).expect_err("must fail");
        assert_eq!(err.kind, market_cli::error::ErrorKind::Runtime);
        assert_eq!(err.code, ERROR_CODE_RUNTIME_PROVIDER_FAILED);
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn main_outputs_expr_alfred_json_contract() {
        let cli = Cli::parse_from(["market-cli", "expr", "--query", "1+5"]);
        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("expr should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be array");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].get("title").and_then(Value::as_str), Some("6"));
    }

    #[test]
    fn main_maps_expr_syntax_error_to_user_error() {
        let cli = Cli::parse_from(["market-cli", "expr", "--query", "2 btc + 5"]);
        let err = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect_err("must fail");

        assert_eq!(err.kind, market_cli::error::ErrorKind::User);
        assert_eq!(err.code, ERROR_CODE_USER_INVALID_INPUT);
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_outputs_fx_human_mode_by_default() {
        let cli = Cli::parse_from([
            "market-cli",
            "fx",
            "--base",
            "USD",
            "--quote",
            "TWD",
            "--amount",
            "100",
        ]);
        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("fx should pass");

        assert!(output.contains("USD"));
        assert!(output.contains("provider=frankfurter"));
    }

    #[test]
    fn main_outputs_fx_alfred_json_mode_when_requested() {
        let cli = Cli::parse_from([
            "market-cli",
            "fx",
            "--base",
            "USD",
            "--quote",
            "TWD",
            "--amount",
            "100",
            "--output",
            "alfred-json",
        ]);
        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("fx should pass");
        let json: Value = serde_json::from_str(&output).expect("json");
        let first_item = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .expect("first item");

        assert!(first_item.get("title").is_some());
    }

    #[test]
    fn main_outputs_expr_json_envelope_when_requested() {
        let cli = Cli::parse_from(["market-cli", "expr", "--query", "1+5", "--json"]);
        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("expr should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("v1")
        );
        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("market.expr")
        );
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
        assert!(json.get("result").is_some());
    }

    #[test]
    fn main_outputs_favorites_human_mode_without_quote_resolution() {
        let cli = Cli::parse_from([
            "market-cli",
            "favorites",
            "--list",
            "btc,eth,usd,jpy",
            "--default-fiat",
            "USD",
            "--output",
            "human",
        ]);
        let failing_providers = FakeProviders {
            fx_result: Err(ProviderError::Transport("offline".to_string())),
            crypto_coinbase_result: Err(ProviderError::Transport("offline".to_string())),
            crypto_kraken_result: Err(ProviderError::Transport("offline".to_string())),
        };

        let output = run_with(cli, &config_in_tempdir(), &failing_providers, fixed_now)
            .expect("favorites human output should not resolve quotes");

        assert_eq!(output, "favorites: BTC, ETH, USD, JPY");
    }

    #[test]
    fn main_outputs_favorites_alfred_json_with_prompt_and_quote_rows() {
        let cli = Cli::parse_from([
            "market-cli",
            "favorites",
            "--list",
            "btc,eth,usd,jpy",
            "--default-fiat",
            "USD",
            "--output",
            "alfred-json",
        ]);

        let output = run_with(cli, &config_in_tempdir(), &FavoritesProviders, fixed_now)
            .expect("favorites Alfred output should pass");
        let json: Value = serde_json::from_str(&output).expect("json");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be array");

        assert_eq!(items.len(), 5);
        assert_eq!(
            items[0].get("title").and_then(Value::as_str),
            Some(FAVORITES_PROMPT_TITLE)
        );
        assert_eq!(
            items[0].get("subtitle").and_then(Value::as_str),
            Some("Example: 1 BTC + 3 ETH to JPY (default fiat: USD)")
        );
        assert_eq!(
            items[1].get("title").and_then(Value::as_str),
            Some("1 BTC = 68194 USD")
        );
        assert_eq!(
            items[2].get("title").and_then(Value::as_str),
            Some("1 ETH = 1980 USD")
        );
        assert_eq!(
            items[3].get("title").and_then(Value::as_str),
            Some("1 USD = 1 USD")
        );
        assert_eq!(
            items[4].get("title").and_then(Value::as_str),
            Some("1 JPY = 0.01 USD")
        );
        assert!(items.iter().all(|item| item.get("uid").is_some()));
        assert!(
            items
                .iter()
                .all(|item| { item.get("valid").and_then(Value::as_bool) == Some(false) })
        );
    }

    #[test]
    fn main_favorites_quote_failures_fall_back_to_symbol_hint_rows() {
        let cli = Cli::parse_from([
            "market-cli",
            "favorites",
            "--list",
            "btc",
            "--default-fiat",
            "USD",
            "--output",
            "alfred-json",
        ]);
        let failing_providers = FakeProviders {
            fx_result: Err(ProviderError::Transport("offline".to_string())),
            crypto_coinbase_result: Err(ProviderError::Transport("offline".to_string())),
            crypto_kraken_result: Err(ProviderError::Transport("offline".to_string())),
        };

        let output = run_with(cli, &config_in_tempdir(), &failing_providers, fixed_now)
            .expect("favorites output should degrade gracefully");
        let json: Value = serde_json::from_str(&output).expect("json");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be array");

        assert_eq!(items.len(), 2);
        assert_eq!(items[1].get("title").and_then(Value::as_str), Some("BTC"));
        assert_eq!(
            items[1].get("subtitle").and_then(Value::as_str),
            Some(FAVORITES_QUOTE_UNAVAILABLE_SUBTITLE)
        );
        assert_eq!(items[1].get("valid").and_then(Value::as_bool), Some(false));
    }

    #[test]
    fn main_outputs_favorites_json_envelope_when_requested() {
        let alfred_cli = Cli::parse_from([
            "market-cli",
            "favorites",
            "--list",
            "btc,eth,usd,jpy",
            "--default-fiat",
            "USD",
            "--output",
            "alfred-json",
        ]);
        let json_cli = Cli::parse_from([
            "market-cli",
            "favorites",
            "--list",
            "btc,eth,usd,jpy",
            "--default-fiat",
            "USD",
            "--json",
        ]);

        let direct = run_with(
            alfred_cli,
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("favorites Alfred output should pass");
        let envelope = run_with(
            json_cli,
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("favorites JSON output should pass");
        let envelope_json: Value = serde_json::from_str(&envelope).expect("json");
        let direct_json: Value = serde_json::from_str(&direct).expect("json");

        assert_eq!(
            envelope_json.get("command").and_then(Value::as_str),
            Some("market.favorites")
        );
        assert_eq!(envelope_json.get("ok").and_then(Value::as_bool), Some(true));
        assert_eq!(envelope_json.get("result"), Some(&direct_json));
    }

    #[test]
    fn main_rejects_conflicting_json_flags() {
        let cli = Cli::parse_from([
            "market-cli",
            "fx",
            "--base",
            "USD",
            "--quote",
            "TWD",
            "--amount",
            "100",
            "--json",
            "--output",
            "human",
        ]);
        let err = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect_err("must fail");

        assert_eq!(err.kind, market_cli::error::ErrorKind::User);
        assert_eq!(err.code, ERROR_CODE_USER_OUTPUT_MODE_CONFLICT);
    }

    #[test]
    fn main_redacts_sensitive_error_fragments() {
        let redacted = redact_sensitive(
            "authorization: Bearer abc token=xyz secret=hidden client_secret:demo",
        );
        assert!(!redacted.contains("abc"));
        assert!(!redacted.contains("xyz"));
        assert!(!redacted.contains("hidden"));
        assert!(!redacted.contains("demo"));
        assert!(redacted.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn main_help_flag_is_supported() {
        let help = Cli::try_parse_from(["market-cli", "--help"]).expect_err("help");
        assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
    }
}
