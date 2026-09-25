use std::path::Path;

use alfred_core::{Feedback, Item, ItemIcon};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use rust_decimal::Decimal;
use workflow_common::{
    AppError as CliError, EnvelopePayloadKind, OutputMode, build_alfred_error_feedback,
    build_error_details_json, build_error_envelope, build_success_envelope,
    preference_projection::{ProjectionStatus, load_preference_projection},
    redact_sensitive,
};

use market_cli::{
    FavoriteTarget,
    config::RuntimeConfig,
    error::AppError,
    expression, favorites_from_watchlist, icons,
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
        #[arg(long, value_enum, default_value_t = OutputMode::Human)]
        output: OutputMode,
    },
    /// Query crypto spot price (Coinbase with Kraken fallback).
    Crypto {
        #[arg(long)]
        base: String,
        #[arg(long)]
        quote: String,
        #[arg(long)]
        amount: String,
        #[arg(long, value_enum, default_value_t = OutputMode::Human)]
        output: OutputMode,
    },
    /// Evaluate market expressions and return Alfred Script Filter JSON.
    Expr {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "USD")]
        default_fiat: String,
        #[arg(long, value_enum, default_value_t = OutputMode::AlfredJson)]
        output: OutputMode,
    },
    /// Render configured market favorites as non-actionable Alfred rows.
    Favorites {
        #[arg(long)]
        list: Option<String>,
        #[arg(long, default_value = "USD")]
        default_fiat: String,
        /// Optional external preference projection file. When valid and
        /// fresh, its market watchlist replaces `--list`. Read-only.
        #[arg(long, value_name = "PATH")]
        preference_projection_file: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputMode::AlfredJson)]
        output: OutputMode,
    },
}

const ERROR_CODE_USER_INVALID_INPUT: &str = "NILS_MARKET_001";
const ERROR_CODE_RUNTIME_PROVIDER_INIT: &str = "NILS_MARKET_002";
const ERROR_CODE_RUNTIME_PROVIDER_FAILED: &str = "NILS_MARKET_002";
const ERROR_CODE_RUNTIME_SERIALIZE: &str = "NILS_COMMON_005";
const FAVORITES_PROMPT_TITLE: &str = "Enter a market expression";
const FAVORITES_PROMPT_EXAMPLE: &str = "Example: 1 BTC + 3 ETH to JPY";
const FAVORITES_PROMPT_UID: &str = "market-favorites-ordered-prompt-v1";
const FAVORITES_UID_NAMESPACE: &str = "market-favorite-ordered-v1";
const FAVORITES_QUOTE_UNAVAILABLE_SUBTITLE: &str =
    "Favorite quote. Type an expression to convert. Quote unavailable.";
const FAVORITES_PROJECTION_USED_HINT: &str =
    "Favorites from the external preference projection. Type an expression to override.";

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
            Commands::Fx { output, .. }
            | Commands::Crypto { output, .. }
            | Commands::Expr { output, .. }
            | Commands::Favorites { output, .. } => *output,
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
    P: ProviderApi + Clone + Send,
    N: Fn() -> DateTime<Utc> + Copy + Send,
{
    match cli.command {
        Commands::Fx {
            base,
            quote,
            amount,
            output,
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
            },
        ),
        Commands::Crypto {
            base,
            quote,
            amount,
            output,
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
            },
        ),
        Commands::Expr {
            query,
            default_fiat,
            output,
        } => {
            let feedback =
                expression::evaluate_query(config, providers, now_fn, &query, &default_fiat)
                    .map_err(map_app_error)?;
            let output_mode = output;
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
            preference_projection_file,
            output,
        } => {
            let now = now_fn();
            let projection_path = preference_projection_file
                .as_deref()
                .filter(|path| !path.is_empty())
                .map(Path::new);
            let (favorites, projection_status) =
                resolve_favorites(list.as_deref(), &default_fiat, projection_path, now).map_err(
                    |error| user_error(ERROR_CODE_USER_INVALID_INPUT, error.to_string()),
                )?;
            let default_fiat = normalize_fx_symbol(&default_fiat, "default_fiat")
                .map_err(|error| user_error(ERROR_CODE_USER_INVALID_INPUT, error.to_string()))?;
            let status_item = projection_status
                .as_ref()
                .map(|status| status.to_item(now, FAVORITES_PROJECTION_USED_HINT));
            let output_mode = output;

            match output_mode {
                OutputMode::Human => Ok(format_favorites_human_output(
                    &favorites,
                    projection_status.as_ref().map(|status| status.title(now)),
                )),
                OutputMode::AlfredJson | OutputMode::Json => {
                    let alfred_json = render_favorites_alfred_output(
                        config,
                        providers,
                        now_fn,
                        &favorites,
                        &default_fiat,
                        status_item,
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
    output: OutputMode,
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
    let output_mode = args.output;
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
        OutputMode::AlfredJson => render_market_alfred_output(config, &result),
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
    config: &RuntimeConfig,
    output: &market_cli::model::MarketOutput,
) -> Result<String, CliError> {
    let item = Item::new(format!(
        "{} {} = {} {}",
        output.amount, output.base, output.converted, output.quote
    ))
    .with_subtitle(format!(
        "price={} provider={} cache={}",
        output.unit_price,
        output.provider,
        cache_status_label(output.cache.status)
    ))
    .with_arg(output.converted.clone())
    .with_valid(false);
    let item = with_symbol_icon(item, config, &output.base);

    Feedback::new(vec![item]).to_json().map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to serialize Alfred output: {error}"),
        )
    })
}

/// Resolve favorites with precedence: valid fresh projection watchlist, then
/// `--list`, then the built-in default set. Returns the projection status when
/// a projection path is configured.
fn resolve_favorites(
    list: Option<&str>,
    default_fiat: &str,
    projection_path: Option<&Path>,
    now: DateTime<Utc>,
) -> Result<(Vec<FavoriteTarget>, Option<ProjectionStatus>), market_cli::model::ValidationError> {
    let Some(path) = projection_path else {
        return Ok((parse_favorites_list(list, default_fiat)?, None));
    };

    let status = match load_preference_projection(path, now) {
        Ok(projection) => {
            let mapped = favorites_from_watchlist(
                &projection.market.watchlist,
                &projection.market.default_quote_currency,
                default_fiat,
            )?;
            if !mapped.favorites.is_empty() {
                let status = ProjectionStatus::Used {
                    revision: projection.revision,
                    generated_at: projection.generated_at,
                    skipped: mapped.skipped,
                };
                return Ok((mapped.favorites, Some(status)));
            }
            ProjectionStatus::Empty {
                revision: projection.revision,
                skipped: mapped.skipped,
            }
        }
        Err(error) => ProjectionStatus::Failed(error),
    };

    Ok((parse_favorites_list(list, default_fiat)?, Some(status)))
}

fn format_favorites_human_output(
    favorites: &[FavoriteTarget],
    projection_status_title: Option<String>,
) -> String {
    let line = format!(
        "favorites: {}",
        favorites
            .iter()
            .map(FavoriteTarget::display_token)
            .collect::<Vec<_>>()
            .join(", ")
    );

    match projection_status_title {
        Some(title) => format!("{line}\n{title}"),
        None => line,
    }
}

fn render_favorites_alfred_output<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    favorites: &[FavoriteTarget],
    default_fiat: &str,
    status_item: Option<Item>,
) -> Result<String, CliError>
where
    P: ProviderApi + Clone + Send,
    N: Fn() -> DateTime<Utc> + Copy + Send,
{
    let mut items = Vec::with_capacity(favorites.len() + 2);
    items.push(
        Item::new(FAVORITES_PROMPT_TITLE)
            .with_uid(FAVORITES_PROMPT_UID)
            .with_subtitle(format!(
                "{FAVORITES_PROMPT_EXAMPLE} (default fiat: {default_fiat})"
            ))
            .with_valid(false),
    );
    items.extend(status_item);

    let favorite_items =
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(favorites.len());

            for favorite in favorites {
                let config = config.clone();
                let providers = providers.clone();
                let favorite = favorite.clone();

                handles.push(scope.spawn(move || {
                    build_favorite_quote_item(&config, &providers, now_fn, &favorite)
                }));
            }

            handles
                .into_iter()
                .map(|handle| handle.join().expect("favorite worker panicked"))
                .collect::<Vec<_>>()
        });
    items.extend(favorite_items);

    Feedback::new(items)
        .with_skip_knowledge(true)
        .to_json()
        .map_err(|error| {
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
    favorite: &FavoriteTarget,
) -> Item
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    let base = favorite.base();
    let quote = favorite.quote();

    if base == quote {
        return with_symbol_icon(
            Item::new(format!("1 {base} = 1 {quote}"))
                .with_uid(favorite_item_uid(base, quote))
                .with_subtitle("provider: identity · freshness: fixed")
                .with_valid(false),
            config,
            base,
        );
    }

    match resolve_favorite_output(config, providers, now_fn, favorite) {
        Ok(output) => favorite_quote_success_item(config, favorite, &output),
        Err(_) => with_symbol_icon(
            Item::new(favorite.display_token())
                .with_uid(favorite_item_uid(base, quote))
                .with_subtitle(FAVORITES_QUOTE_UNAVAILABLE_SUBTITLE)
                .with_valid(false),
            config,
            base,
        ),
    }
}

fn favorite_quote_success_item(
    config: &RuntimeConfig,
    favorite: &FavoriteTarget,
    output: &market_cli::model::MarketOutput,
) -> Item {
    let base = favorite.base();
    let quote = favorite.quote();
    let rendered_price = output
        .unit_price
        .parse::<Decimal>()
        .map(expression::format_market_decimal)
        .unwrap_or_else(|_| output.unit_price.clone());

    with_symbol_icon(
        Item::new(format!("1 {base} = {rendered_price} {quote}"))
            .with_uid(favorite_item_uid(base, quote))
            .with_subtitle(format!(
                "provider: {} · freshness: {}",
                output.provider,
                cache_status_label(output.cache.status)
            ))
            .with_valid(false),
        config,
        base,
    )
}

fn resolve_favorite_output<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    favorite: &FavoriteTarget,
) -> Result<market_cli::model::MarketOutput, AppError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    match favorite {
        FavoriteTarget::Symbol { symbol, quote } => {
            expression::resolve_symbol_output(config, providers, now_fn, symbol, quote)
        }
        FavoriteTarget::FxPair { base, quote } => {
            let request =
                MarketRequest::new(MarketKind::Fx, base, quote, "1").map_err(AppError::from)?;
            service::resolve_market(config, providers, now_fn, &request)
        }
    }
}

fn favorite_item_uid(base: &str, quote: &str) -> String {
    format!(
        "{FAVORITES_UID_NAMESPACE}-{}-{}",
        base.to_ascii_lowercase(),
        quote.to_ascii_lowercase()
    )
}

fn with_symbol_icon(item: Item, config: &RuntimeConfig, symbol: &str) -> Item {
    if let Some(path) = icons::resolve_icon_path(config, symbol) {
        return item.with_icon(ItemIcon::new(path.to_string_lossy().into_owned()));
    }

    item
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
            let details = build_error_details_json(error.kind().as_str(), error.exit_code());
            println!(
                "{}",
                build_error_envelope(command, error.code(), error.message(), Some(&details))
            );
        }
        OutputMode::AlfredJson => {
            println!(
                "{}",
                build_alfred_error_feedback(error.code(), error.message())
            );
        }
        OutputMode::Human => {
            eprintln!(
                "error[{}]: {}",
                error.code(),
                redact_sensitive(error.message())
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

fn cache_status_label(status: market_cli::model::CacheStatus) -> &'static str {
    match status {
        market_cli::model::CacheStatus::Live => "live",
        market_cli::model::CacheStatus::CacheFresh => "cache_fresh",
        market_cli::model::CacheStatus::CacheStaleFallback => "cache_stale_fallback",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use chrono::TimeZone;
    use market_cli::{
        cache::{CacheRecord, cache_path, write_cache},
        icon_asset_filename,
        model::{MarketKind, MarketQuote},
        providers::ProviderError,
    };
    use serde_json::Value;
    use workflow_common::CliErrorKind;

    use super::*;

    #[derive(Clone)]
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

    #[derive(Clone, Copy)]
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
                ("JPY", "TWD") => Ok(MarketQuote::new(
                    "frankfurter",
                    rust_decimal::Decimal::new(215, 2),
                    now,
                )),
                ("USD", "JPY") => Ok(MarketQuote::new(
                    "frankfurter",
                    rust_decimal::Decimal::new(15025, 2),
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
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().to_path_buf();
        std::mem::forget(dir);

        let config = RuntimeConfig {
            cache_dir,
            fx_cache_ttl_secs: market_cli::config::FX_TTL_SECS,
            crypto_cache_ttl_secs: market_cli::config::CRYPTO_TTL_SECS,
        };
        seed_icon_files(&config, &["BTC", "ETH", "USD", "JPY"]);
        config
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 10, 12, 5, 0)
            .single()
            .expect("time")
    }

    fn seed_icon_files(config: &RuntimeConfig, symbols: &[&str]) {
        let icon_dir = config.icon_cache_dir();
        fs::create_dir_all(&icon_dir).expect("create icon dir");
        fs::write(
            icon_dir.join(market_cli::config::ICON_GENERIC_BASENAME),
            b"generic-icon",
        )
        .expect("write generic icon");

        for symbol in symbols {
            seed_icon_file_at(config, symbol);
        }
    }

    fn seed_icon_file_at(config: &RuntimeConfig, symbol: &str) -> String {
        let file_name = icon_asset_filename(symbol).expect("icon filename");
        fs::write(
            config.icon_cache_dir().join(&file_name),
            format!("icon-{symbol}"),
        )
        .expect("write icon file");
        file_name
    }

    fn seed_quote_cache(
        config: &RuntimeConfig,
        kind: MarketKind,
        base: &str,
        quote: &str,
        provider: &str,
        unit_price: &str,
    ) {
        let path = cache_path(config, kind, base, quote);
        let record = CacheRecord {
            base: base.to_string(),
            quote: quote.to_string(),
            provider: provider.to_string(),
            unit_price: unit_price.to_string(),
            fetched_at: Utc::now().to_rfc3339(),
        };

        write_cache(&path, &record).expect("write cache");
    }

    fn item_icon_path(item: &Value) -> Option<&str> {
        item.get("icon")
            .and_then(|icon| icon.get("path"))
            .and_then(Value::as_str)
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
            "--output",
            "json",
        ]);

        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("fx should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
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
            "--output",
            "json",
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
        assert_eq!(err.kind(), CliErrorKind::User);
        assert_eq!(err.code(), ERROR_CODE_USER_INVALID_INPUT);
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
        assert_eq!(err.kind(), CliErrorKind::Runtime);
        assert_eq!(err.code(), ERROR_CODE_RUNTIME_PROVIDER_FAILED);
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

        assert_eq!(err.kind(), CliErrorKind::User);
        assert_eq!(err.code(), ERROR_CODE_USER_INVALID_INPUT);
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
        let config = config_in_tempdir();
        let expected_file = seed_icon_file_at(&config, "USD");
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
        let output =
            run_with(cli, &config, &FakeProviders::ok(), fixed_now).expect("fx should pass");
        let json: Value = serde_json::from_str(&output).expect("json");
        let first_item = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .expect("first item");

        assert!(first_item.get("title").is_some());
        let icon_path = item_icon_path(first_item).expect("icon path");
        assert!(icon_path.ends_with(expected_file.as_str()));
    }

    #[test]
    fn main_outputs_fx_alfred_json_with_generic_fallback_icon() {
        let config = config_in_tempdir();
        let cli = Cli::parse_from([
            "market-cli",
            "fx",
            "--base",
            "TWD",
            "--quote",
            "USD",
            "--amount",
            "100",
            "--output",
            "alfred-json",
        ]);
        let output =
            run_with(cli, &config, &FakeProviders::ok(), fixed_now).expect("fx should pass");
        let json: Value = serde_json::from_str(&output).expect("json");
        let first_item = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .expect("first item");

        let icon_path = item_icon_path(first_item).expect("icon path");
        assert!(icon_path.ends_with(market_cli::config::ICON_GENERIC_BASENAME));
    }

    #[test]
    fn main_outputs_crypto_alfred_json_with_symbol_icon() {
        let config = config_in_tempdir();
        let expected_file = seed_icon_file_at(&config, "BTC");
        let cli = Cli::parse_from([
            "market-cli",
            "crypto",
            "--base",
            "BTC",
            "--quote",
            "USD",
            "--amount",
            "1",
            "--output",
            "alfred-json",
        ]);
        let output = run_with(cli, &config, &FakeProviders::ok(), fixed_now).expect("crypto");
        let json: Value = serde_json::from_str(&output).expect("json");
        let first_item = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .expect("first item");

        let icon_path = item_icon_path(first_item).expect("icon path");
        assert!(icon_path.ends_with(expected_file.as_str()));
    }

    #[test]
    fn main_outputs_expr_json_envelope_when_requested() {
        let cli = Cli::parse_from(["market-cli", "expr", "--query", "1+5", "--output", "json"]);
        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("expr should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("cli-envelope@v1")
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
    fn main_outputs_favorites_human_mode_with_explicit_fx_pairs() {
        let cli = Cli::parse_from([
            "market-cli",
            "favorites",
            "--list",
            "jpy/usd,jpy/twd,btc",
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

        assert_eq!(output, "favorites: JPY/USD, JPY/TWD, BTC");
    }

    #[test]
    fn favorites_rows_include_icon_paths_for_supported_symbols() {
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
        assert_eq!(
            json.get("skipknowledge").and_then(Value::as_bool),
            Some(true),
            "favorite uid rows must preserve configured order"
        );
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
            Some("1 JPY = 0.007 USD")
        );
        assert!(items.iter().all(|item| item.get("uid").is_some()));
        assert!(items[0].get("icon").is_none());
        assert!(item_icon_path(&items[1]).is_some_and(|path| path.ends_with("btc.png")));
        assert!(item_icon_path(&items[2]).is_some_and(|path| path.ends_with("eth.png")));
        assert!(item_icon_path(&items[3]).is_some_and(|path| path.ends_with("usd.png")));
        assert!(item_icon_path(&items[4]).is_some_and(|path| path.ends_with("jpy.png")));
        assert!(
            items
                .iter()
                .all(|item| { item.get("valid").and_then(Value::as_bool) == Some(false) })
        );
    }

    #[test]
    fn favorites_rows_include_explicit_fx_pairs() {
        let cli = Cli::parse_from([
            "market-cli",
            "favorites",
            "--list",
            "jpy/usd,jpy/twd,usd/jpy",
            "--default-fiat",
            "USD",
            "--output",
            "alfred-json",
        ]);

        let output = run_with(cli, &config_in_tempdir(), &FavoritesProviders, fixed_now)
            .expect("favorites Alfred output with FX pairs should pass");
        let json: Value = serde_json::from_str(&output).expect("json");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be array");

        assert_eq!(items.len(), 4);
        assert_eq!(
            items[1].get("uid").and_then(Value::as_str),
            Some("market-favorite-ordered-v1-jpy-usd")
        );
        assert_eq!(
            items[1].get("title").and_then(Value::as_str),
            Some("1 JPY = 0.007 USD")
        );
        assert_eq!(
            items[2].get("uid").and_then(Value::as_str),
            Some("market-favorite-ordered-v1-jpy-twd")
        );
        assert_eq!(
            items[2].get("title").and_then(Value::as_str),
            Some("1 JPY = 2.150 TWD")
        );
        assert_eq!(
            items[3].get("uid").and_then(Value::as_str),
            Some("market-favorite-ordered-v1-usd-jpy")
        );
        assert_eq!(
            items[3].get("title").and_then(Value::as_str),
            Some("1 USD = 150.3 JPY")
        );
        assert!(item_icon_path(&items[1]).is_some_and(|path| path.ends_with("jpy.png")));
        assert!(item_icon_path(&items[2]).is_some_and(|path| path.ends_with("jpy.png")));
        assert!(item_icon_path(&items[3]).is_some_and(|path| path.ends_with("usd.png")));
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
        assert!(item_icon_path(&items[1]).is_some_and(|path| path.ends_with("btc.png")));
    }

    #[test]
    fn main_favorite_fx_pair_failures_fall_back_to_pair_hint_rows() {
        let cli = Cli::parse_from([
            "market-cli",
            "favorites",
            "--list",
            "jpy/twd",
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
            .expect("favorite FX pair output should degrade gracefully");
        let json: Value = serde_json::from_str(&output).expect("json");
        let items = json
            .get("items")
            .and_then(Value::as_array)
            .expect("items should be array");

        assert_eq!(items.len(), 2);
        assert_eq!(
            items[1].get("title").and_then(Value::as_str),
            Some("JPY/TWD")
        );
        assert_eq!(
            items[1].get("subtitle").and_then(Value::as_str),
            Some(FAVORITES_QUOTE_UNAVAILABLE_SUBTITLE)
        );
        assert_eq!(items[1].get("valid").and_then(Value::as_bool), Some(false));
        assert!(item_icon_path(&items[1]).is_some_and(|path| path.ends_with("jpy.png")));
    }

    #[test]
    fn main_outputs_favorites_json_envelope_when_requested() {
        let config = config_in_tempdir();
        seed_quote_cache(
            &config,
            MarketKind::Crypto,
            "BTC",
            "USD",
            "coinbase",
            "68194",
        );
        seed_quote_cache(
            &config,
            MarketKind::Crypto,
            "ETH",
            "USD",
            "coinbase",
            "1980",
        );
        seed_quote_cache(
            &config,
            MarketKind::Fx,
            "JPY",
            "USD",
            "frankfurter",
            "0.0067",
        );
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
            "--output",
            "json",
        ]);

        let direct = run_with(alfred_cli, &config, &FavoritesProviders, fixed_now)
            .expect("favorites Alfred output should pass");
        let envelope = run_with(json_cli, &config, &FavoritesProviders, fixed_now)
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

    fn write_projection(dir: &tempfile::TempDir, generated_at: &str, watchlist: Value) -> PathBuf {
        let path = dir.path().join("preference-projection.json");
        let document = serde_json::json!({
            "schema": "sympoies.alfred-preference-projection/v1",
            "generatedAt": generated_at,
            "revision": 2,
            "digest": format!("sha256:{}", "a".repeat(64)),
            "market": {"default_quote_currency": "TWD", "watchlist": watchlist},
            "weather": {"default_location": "Synthetic City", "saved_locations": []},
            "sources": {
                "market.default_quote_currency": "profile",
                "market.watchlist": "owner_override",
                "weather.default_location": "profile",
                "weather.saved_locations": "profile"
            }
        });
        fs::write(&path, document.to_string()).expect("write projection");
        path
    }

    fn favorites_cli(list: &str, projection: Option<&Path>, output: &str) -> Cli {
        let mut args = vec![
            "market-cli".to_string(),
            "favorites".to_string(),
            "--list".to_string(),
            list.to_string(),
            "--default-fiat".to_string(),
            "USD".to_string(),
            "--output".to_string(),
            output.to_string(),
        ];
        if let Some(path) = projection {
            args.push("--preference-projection-file".to_string());
            args.push(path.to_string_lossy().into_owned());
        }
        Cli::parse_from(args)
    }

    fn items_of(output: &str) -> Vec<Value> {
        let json: Value = serde_json::from_str(output).expect("json");
        json.get("items")
            .and_then(Value::as_array)
            .cloned()
            .expect("items should be array")
    }

    #[test]
    fn favorites_prefer_valid_projection_watchlist_over_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_projection(
            &dir,
            "2026-02-10T11:53:00Z",
            serde_json::json!(["USD", "JPY", "BTC", "ETH"]),
        );

        let output = run_with(
            favorites_cli("eth", Some(&path), "alfred-json"),
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("favorites should pass");
        let items = items_of(&output);
        let titles: Vec<&str> = items
            .iter()
            .filter_map(|item| item.get("title").and_then(Value::as_str))
            .collect();

        assert_eq!(
            titles,
            vec![
                FAVORITES_PROMPT_TITLE,
                "Preferences: projection revision 2 · synced 12m ago",
                "USD/TWD",
                "1 JPY = 2.150 TWD",
                "1 BTC = 68194 USD",
                "1 ETH = 1980 USD",
            ]
        );
        assert_eq!(
            items[1].get("subtitle").and_then(Value::as_str),
            Some(FAVORITES_PROJECTION_USED_HINT)
        );
        assert_eq!(items[1].get("valid").and_then(Value::as_bool), Some(false));
        assert!(items[1].get("uid").is_none());
        assert!(!output.contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn favorites_human_output_reports_projection_mapping_without_quotes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_projection(
            &dir,
            "2026-02-10T11:53:00Z",
            serde_json::json!(["USD", "JPY", "BTC", "ETH", "ADA", "DOT", "BTC-USD"]),
        );

        let output = run_with(
            favorites_cli("", Some(&path), "human"),
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("favorites should pass");

        assert_eq!(
            output,
            "favorites: USD/TWD, JPY/TWD, BTC, ETH, ADA, DOT\n\
             Preferences: projection revision 2 · synced 12m ago"
        );
    }

    #[test]
    fn favorites_fall_back_to_list_with_status_when_projection_unusable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let stale = write_projection(&dir, "2026-02-01T12:00:00Z", serde_json::json!(["BTC"]));
        let stale_output = run_with(
            favorites_cli("eth,jpy", Some(&stale), "human"),
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("stale projection falls back");
        assert_eq!(
            stale_output,
            "favorites: ETH, JPY\nPreferences: projection stale — using workflow settings"
        );

        let missing = dir.path().join("missing.json");
        let missing_output = run_with(
            favorites_cli("eth", Some(&missing), "human"),
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("missing projection falls back");
        assert_eq!(
            missing_output,
            "favorites: ETH\nPreferences: projection unavailable — using workflow settings"
        );

        let malformed = dir.path().join("malformed.json");
        fs::write(&malformed, "{\"schema\":").expect("write");
        let malformed_output = run_with(
            favorites_cli("", Some(&malformed), "human"),
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("malformed projection falls back");
        assert_eq!(
            malformed_output,
            "favorites: BTC, ETH, USD, JPY\nPreferences: projection invalid — using workflow settings"
        );

        let future = write_projection(&dir, "2026-02-10T13:00:00Z", serde_json::json!(["BTC"]));
        let future_output = run_with(
            favorites_cli("eth", Some(&future), "human"),
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("future projection falls back");
        assert!(
            future_output.ends_with("Preferences: projection invalid — using workflow settings")
        );
    }

    #[test]
    fn favorites_fall_back_when_projection_watchlist_has_no_usable_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        for watchlist in [serde_json::json!([]), serde_json::json!(["TWD", "BTC-USD"])] {
            let path = write_projection(&dir, "2026-02-10T11:53:00Z", watchlist);
            let output = run_with(
                favorites_cli("eth", Some(&path), "alfred-json"),
                &config_in_tempdir(),
                &FavoritesProviders,
                fixed_now,
            )
            .expect("empty projection falls back");
            let items = items_of(&output);
            assert_eq!(
                items[1].get("title").and_then(Value::as_str),
                Some(
                    "Preferences: projection revision 2 has no usable entries — using workflow settings"
                )
            );
            assert_eq!(
                items[2].get("title").and_then(Value::as_str),
                Some("1 ETH = 1980 USD")
            );
        }
    }

    #[test]
    fn favorites_without_projection_emit_no_status_row() {
        let empty_path = run_with(
            Cli::parse_from([
                "market-cli",
                "favorites",
                "--list",
                "eth",
                "--default-fiat",
                "USD",
                "--preference-projection-file",
                "",
                "--output",
                "alfred-json",
            ]),
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("empty projection path is ignored");
        let unset = run_with(
            favorites_cli("eth", None, "alfred-json"),
            &config_in_tempdir(),
            &FavoritesProviders,
            fixed_now,
        )
        .expect("unset projection path");

        for output in [empty_path, unset] {
            let items = items_of(&output);
            assert_eq!(items.len(), 2);
            assert_eq!(
                items[1].get("title").and_then(Value::as_str),
                Some("1 ETH = 1980 USD")
            );
        }
    }

    #[test]
    fn main_rejects_unknown_output_value() {
        let result = Cli::try_parse_from([
            "market-cli",
            "fx",
            "--base",
            "USD",
            "--quote",
            "TWD",
            "--amount",
            "100",
            "--output",
            "yaml",
        ]);
        let err = result.expect_err("must fail when output value is invalid");
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
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
