use std::collections::{HashMap, HashSet};

use alfred_core::{Feedback, Item, ItemIcon};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, RoundingStrategy};

use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::icons;
use crate::model::{
    CacheStatus, MarketKind, MarketOutput, MarketRequest, decimal_to_string, normalize_fx_symbol,
};
use crate::providers::ProviderApi;
use crate::service;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedTerm {
    Numeric(Decimal),
    Asset { amount: Decimal, symbol: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionMode {
    Numeric,
    Asset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedExpression {
    terms: Vec<ParsedTerm>,
    operators: Vec<char>,
    target_fiat: String,
    mode: ExpressionMode,
}

#[derive(Debug, Clone)]
struct ResolvedAssetQuote {
    unit_price: Decimal,
    provider: String,
    cache_status: CacheStatus,
}

#[derive(Debug, Clone)]
struct AssetTerm {
    amount: Decimal,
    symbol: String,
}

pub fn evaluate_query<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    query: &str,
    default_fiat: &str,
) -> Result<Feedback, AppError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    let parsed = parse_expression(query, default_fiat)?;
    match parsed.mode {
        ExpressionMode::Numeric => evaluate_numeric_feedback(&parsed),
        ExpressionMode::Asset => evaluate_asset_feedback(config, providers, now_fn, &parsed),
    }
}

fn parse_expression(query: &str, default_fiat: &str) -> Result<ParsedExpression, AppError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(AppError::user("query must not be empty"));
    }

    let (expression_source, target_fiat) = split_target_clause(trimmed, default_fiat)?;

    let parser = ExpressionParser::new(&expression_source);
    let (terms, operators) = parser.parse()?;

    let has_numeric = terms
        .iter()
        .any(|term| matches!(term, ParsedTerm::Numeric(_)));
    let has_asset = terms
        .iter()
        .any(|term| matches!(term, ParsedTerm::Asset { .. }));

    let mode = match (has_numeric, has_asset) {
        (true, true) => {
            return Err(AppError::user(
                "mixed numeric and asset terms are not supported",
            ));
        }
        (true, false) => ExpressionMode::Numeric,
        (false, true) => ExpressionMode::Asset,
        (false, false) => return Err(AppError::user("expression must contain at least one term")),
    };

    if mode == ExpressionMode::Asset
        && operators
            .iter()
            .any(|operator| !matches!(operator, '+' | '-'))
    {
        return Err(AppError::user(
            "unsupported operator in asset expression: only + and - are supported",
        ));
    }

    Ok(ParsedExpression {
        terms,
        operators,
        target_fiat,
        mode,
    })
}

fn split_target_clause(
    trimmed_query: &str,
    default_fiat: &str,
) -> Result<(String, String), AppError> {
    let tokens: Vec<&str> = trimmed_query.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(AppError::user("query must not be empty"));
    }

    if tokens
        .last()
        .is_some_and(|token| token.eq_ignore_ascii_case("to"))
    {
        return Err(AppError::user(
            "incomplete to clause: expected a 3-letter fiat code",
        ));
    }

    if tokens.len() >= 2 && tokens[tokens.len() - 2].eq_ignore_ascii_case("to") {
        let expression_tokens = &tokens[..tokens.len() - 2];
        if expression_tokens.is_empty() {
            return Err(AppError::user(
                "expression must not be empty before to clause",
            ));
        }

        let target =
            normalize_fx_symbol(tokens[tokens.len() - 1], "target").map_err(AppError::from)?;
        return Ok((expression_tokens.join(" "), target));
    }

    let target = normalize_fx_symbol(default_fiat, "default_fiat").map_err(AppError::from)?;
    Ok((trimmed_query.to_string(), target))
}

fn evaluate_numeric_feedback(parsed: &ParsedExpression) -> Result<Feedback, AppError> {
    let mut values = parsed.terms.iter();
    let first = values
        .next()
        .expect("numeric expression always has at least one term");
    let mut total = match first {
        ParsedTerm::Numeric(value) => *value,
        ParsedTerm::Asset { .. } => unreachable!("numeric mode cannot include asset term"),
    };

    for (operator, term) in parsed.operators.iter().zip(values) {
        let value = match term {
            ParsedTerm::Numeric(value) => *value,
            ParsedTerm::Asset { .. } => unreachable!("numeric mode cannot include asset term"),
        };

        match operator {
            '+' => total += value,
            '-' => total -= value,
            '*' => total *= value,
            '/' => {
                total = total
                    .checked_div(value)
                    .ok_or_else(|| AppError::user("division by zero is not allowed"))?;
            }
            _ => unreachable!("parser only permits +, -, * and /"),
        }
    }

    let rendered = format_plain_decimal(total);
    let item = Item::new(rendered.clone())
        .with_subtitle("Numeric result")
        .with_arg(rendered)
        .with_valid(true);
    Ok(Feedback::new(vec![item]))
}

fn evaluate_asset_feedback<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    parsed: &ParsedExpression,
) -> Result<Feedback, AppError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    let asset_terms: Vec<AssetTerm> = parsed
        .terms
        .iter()
        .map(|term| match term {
            ParsedTerm::Asset { amount, symbol } => Ok(AssetTerm {
                amount: *amount,
                symbol: symbol.clone(),
            }),
            ParsedTerm::Numeric(_) => Err(AppError::user(
                "mixed numeric and asset terms are not supported",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut ordered_symbols = Vec::<String>::new();
    let mut seen = HashSet::<String>::new();
    for term in &asset_terms {
        if seen.insert(term.symbol.clone()) {
            ordered_symbols.push(term.symbol.clone());
        }
    }

    let mut quotes = HashMap::<String, ResolvedAssetQuote>::new();
    for symbol in &ordered_symbols {
        let quote = resolve_asset_quote(config, providers, now_fn, symbol, &parsed.target_fiat)?;
        quotes.insert(symbol.clone(), quote);
    }

    let mut items = Vec::new();
    for symbol in ordered_symbols {
        let quote = quotes
            .get(&symbol)
            .expect("quote lookup for resolved symbol must exist");
        let rendered_price = format_market_decimal(quote.unit_price);
        let title = format!("1 {symbol} = {rendered_price} {}", parsed.target_fiat);
        let subtitle = format!(
            "provider: {} · freshness: {}",
            quote.provider,
            cache_status_label(quote.cache_status)
        );

        items.push(with_symbol_icon(
            Item::new(title)
                .with_subtitle(subtitle)
                .with_arg(format!("{rendered_price} {}", parsed.target_fiat))
                .with_valid(true),
            config,
            &symbol,
        ));
    }

    let (total, formula) = evaluate_asset_total(&asset_terms, &parsed.operators, &quotes);
    let rendered_total = format_market_decimal(total);

    items.push(
        Item::new(format!("Total = {rendered_total} {}", parsed.target_fiat))
            .with_subtitle(format!(
                "{formula} = {rendered_total} {}",
                parsed.target_fiat
            ))
            .with_arg(format!("{rendered_total} {}", parsed.target_fiat))
            .with_valid(true),
    );

    Ok(Feedback::new(items))
}

fn with_symbol_icon(item: Item, config: &RuntimeConfig, symbol: &str) -> Item {
    if let Some(path) = icons::resolve_icon_path(config, symbol) {
        return item.with_icon(ItemIcon::new(path.to_string_lossy().into_owned()));
    }

    item
}

fn evaluate_asset_total(
    terms: &[AssetTerm],
    operators: &[char],
    quotes: &HashMap<String, ResolvedAssetQuote>,
) -> (Decimal, String) {
    let first = terms
        .first()
        .expect("asset expression has at least one term");
    let first_quote = quotes
        .get(&first.symbol)
        .expect("first asset quote must exist");
    let mut total = first.amount * first_quote.unit_price;
    let mut formula = format!(
        "{}*{}({})",
        format_plain_decimal(first.amount),
        format_market_decimal(first_quote.unit_price),
        first.symbol
    );

    for (operator, term) in operators.iter().zip(terms.iter().skip(1)) {
        let quote = quotes
            .get(&term.symbol)
            .expect("asset quote must exist for all parsed terms");
        let piece = format!(
            "{}*{}({})",
            format_plain_decimal(term.amount),
            format_market_decimal(quote.unit_price),
            term.symbol
        );

        let amount = term.amount * quote.unit_price;
        match operator {
            '+' => total += amount,
            '-' => total -= amount,
            _ => unreachable!("parser only permits + and -"),
        }

        formula.push(' ');
        formula.push(*operator);
        formula.push(' ');
        formula.push_str(&piece);
    }

    (total, format!("Formula: {formula}"))
}

fn resolve_asset_quote<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    symbol: &str,
    target_fiat: &str,
) -> Result<ResolvedAssetQuote, AppError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    let output = resolve_symbol_output(config, providers, now_fn, symbol, target_fiat)?;
    convert_output(symbol, output)
}

fn convert_output(symbol: &str, output: MarketOutput) -> Result<ResolvedAssetQuote, AppError> {
    let unit_price = output.unit_price.parse::<Decimal>().map_err(|_| {
        AppError::runtime(format!(
            "provider returned invalid unit price for {symbol}: {}",
            output.unit_price
        ))
    })?;

    Ok(ResolvedAssetQuote {
        unit_price,
        provider: output.provider,
        cache_status: output.cache.status,
    })
}

pub fn resolve_symbol_output<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    symbol: &str,
    target_fiat: &str,
) -> Result<MarketOutput, AppError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    let mut trace = Vec::<String>::new();

    if looks_like_fiat_symbol(symbol) {
        let fx_request =
            MarketRequest::new(MarketKind::Fx, symbol, target_fiat, "1").map_err(AppError::from)?;
        match service::resolve_market(config, providers, now_fn, &fx_request) {
            Ok(output) => return Ok(output),
            Err(error) => trace.push(format!("fx: {}", error.message)),
        }
    }

    let crypto_request =
        MarketRequest::new(MarketKind::Crypto, symbol, target_fiat, "1").map_err(AppError::from)?;
    match service::resolve_market(config, providers, now_fn, &crypto_request) {
        Ok(output) => Ok(output),
        Err(error) => {
            trace.push(format!("crypto: {}", error.message));
            Err(AppError::runtime_with_trace(
                &format!("failed to resolve quote for {symbol}/{target_fiat}"),
                &trace,
            ))
        }
    }
}

fn looks_like_fiat_symbol(symbol: &str) -> bool {
    symbol.len() == 3 && symbol.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn cache_status_label(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Live => "live",
        CacheStatus::CacheFresh => "cache_fresh",
        CacheStatus::CacheStaleFallback => "cache_stale_fallback",
    }
}

fn format_plain_decimal(value: Decimal) -> String {
    if value.is_zero() {
        return "0".to_string();
    }

    decimal_to_string(&value)
}

pub fn format_market_decimal(value: Decimal) -> String {
    let abs = value.abs();
    let precision = if abs < Decimal::from(10) {
        3
    } else if abs < Decimal::from(100) {
        2
    } else if abs < Decimal::from(1000) {
        1
    } else {
        0
    };

    let rounded = value.round_dp_with_strategy(precision, RoundingStrategy::MidpointAwayFromZero);
    format!("{rounded:.precision$}", precision = precision as usize)
}

struct ExpressionParser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> ExpressionParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            cursor: 0,
        }
    }

    fn parse(mut self) -> Result<(Vec<ParsedTerm>, Vec<char>), AppError> {
        self.skip_whitespace();
        if self.peek().is_none() {
            return Err(AppError::user("expression must not be empty"));
        }

        let mut terms = Vec::new();
        let mut operators = Vec::new();
        terms.push(self.parse_term()?);

        loop {
            self.skip_whitespace();
            let Some(token) = self.peek() else {
                break;
            };

            let operator = match token {
                b'+' => '+',
                b'-' => '-',
                b'*' => '*',
                b'/' => '/',
                _ => {
                    return Err(AppError::user(format!(
                        "invalid token near `{}`",
                        self.remaining_fragment()
                    )));
                }
            };

            self.cursor += 1;
            operators.push(operator);
            self.skip_whitespace();
            if self.peek().is_none() {
                return Err(AppError::user("expression cannot end with an operator"));
            }

            terms.push(self.parse_term()?);
        }

        Ok((terms, operators))
    }

    fn parse_term(&mut self) -> Result<ParsedTerm, AppError> {
        let amount = self.parse_decimal()?;
        let spaces = self.skip_whitespace();

        if spaces > 0
            && self
                .peek()
                .is_some_and(|token| token.is_ascii_alphanumeric())
        {
            let symbol = self.parse_asset_symbol(true)?;
            return Ok(ParsedTerm::Asset { amount, symbol });
        }

        // Relaxed compact form: allow "1btc", "3eth" without whitespace.
        // To avoid accidentally treating scientific-like tokens as assets
        // (for example 1e2), compact suffix must be letters only.
        if spaces == 0 && self.peek().is_some_and(|token| token.is_ascii_alphabetic()) {
            let symbol = self.parse_asset_symbol(false)?;
            return Ok(ParsedTerm::Asset { amount, symbol });
        }

        Ok(ParsedTerm::Numeric(amount))
    }

    fn parse_decimal(&mut self) -> Result<Decimal, AppError> {
        let start = self.cursor;

        if self
            .peek()
            .is_some_and(|token| token == b'+' || token == b'-')
        {
            self.cursor += 1;
        }

        let mut integer_digits = 0usize;
        while self.peek().is_some_and(|token| token.is_ascii_digit()) {
            self.cursor += 1;
            integer_digits += 1;
        }

        let mut saw_dot = false;
        let mut fractional_digits = 0usize;
        if self.peek() == Some(b'.') {
            saw_dot = true;
            self.cursor += 1;
            while self.peek().is_some_and(|token| token.is_ascii_digit()) {
                self.cursor += 1;
                fractional_digits += 1;
            }
        }

        if integer_digits == 0 && fractional_digits == 0 {
            return Err(AppError::user(format!(
                "invalid number token near `{}`",
                self.remaining_fragment()
            )));
        }

        if saw_dot && fractional_digits == 0 {
            return Err(AppError::user(
                "invalid number token: decimal point must be followed by digits",
            ));
        }

        let token = self.slice(start, self.cursor)?;
        token
            .parse::<Decimal>()
            .map_err(|_| AppError::user(format!("invalid number token: {token}")))
    }

    fn parse_asset_symbol(&mut self, allow_digits: bool) -> Result<String, AppError> {
        let start = self.cursor;
        while self.peek().is_some_and(|token| {
            token.is_ascii_alphabetic() || (allow_digits && token.is_ascii_digit())
        }) {
            self.cursor += 1;
        }

        let symbol = self.slice(start, self.cursor)?;
        let normalized = symbol.to_ascii_uppercase();
        if normalized.len() < 2 || normalized.len() > 10 {
            return Err(AppError::user(format!("invalid asset token: {symbol}")));
        }
        if !normalized
            .chars()
            .all(|token| token.is_ascii_uppercase() || token.is_ascii_digit())
        {
            return Err(AppError::user(format!("invalid asset token: {symbol}")));
        }

        Ok(normalized)
    }

    fn slice(&self, start: usize, end: usize) -> Result<&'a str, AppError> {
        std::str::from_utf8(&self.bytes[start..end])
            .map_err(|_| AppError::user("expression must contain valid UTF-8 text"))
    }

    fn skip_whitespace(&mut self) -> usize {
        let start = self.cursor;
        while self.peek().is_some_and(|token| token.is_ascii_whitespace()) {
            self.cursor += 1;
        }
        self.cursor - start
    }

    fn remaining_fragment(&self) -> String {
        self.input
            .get(self.cursor..)
            .unwrap_or_default()
            .chars()
            .take(16)
            .collect()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.cursor).copied()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;

    use chrono::TimeZone;

    use super::*;
    use crate::icon_asset_filename;
    use crate::providers::ProviderError;

    #[derive(Debug)]
    struct FakeProviders {
        fx_calls: Cell<usize>,
        coinbase_calls: Cell<usize>,
        kraken_calls: Cell<usize>,
    }

    impl FakeProviders {
        fn new() -> Self {
            Self {
                fx_calls: Cell::new(0),
                coinbase_calls: Cell::new(0),
                kraken_calls: Cell::new(0),
            }
        }
    }

    impl ProviderApi for FakeProviders {
        fn fetch_fx_rate(
            &self,
            base: &str,
            quote: &str,
        ) -> Result<crate::model::MarketQuote, ProviderError> {
            self.fx_calls.set(self.fx_calls.get() + 1);
            let now = Utc
                .with_ymd_and_hms(2026, 2, 10, 12, 0, 0)
                .single()
                .expect("time");
            match (base, quote) {
                ("USD", "JPY") => Ok(crate::model::MarketQuote::new(
                    "frankfurter",
                    Decimal::new(150, 0),
                    now,
                )),
                ("EUR", "JPY") => Ok(crate::model::MarketQuote::new(
                    "frankfurter",
                    Decimal::new(160, 0),
                    now,
                )),
                _ => Err(ProviderError::Http {
                    status: 400,
                    message: "unsupported fx pair".to_string(),
                }),
            }
        }

        fn fetch_crypto_coinbase(
            &self,
            base: &str,
            quote: &str,
        ) -> Result<crate::model::MarketQuote, ProviderError> {
            self.coinbase_calls.set(self.coinbase_calls.get() + 1);
            let now = Utc
                .with_ymd_and_hms(2026, 2, 10, 12, 0, 0)
                .single()
                .expect("time");

            match (base, quote) {
                ("BTC", "JPY") => Ok(crate::model::MarketQuote::new(
                    "coinbase",
                    Decimal::new(10_000_000, 0),
                    now,
                )),
                ("ETH", "JPY") => Ok(crate::model::MarketQuote::new(
                    "coinbase",
                    Decimal::new(350_000, 0),
                    now,
                )),
                ("BTC", "USD") => Ok(crate::model::MarketQuote::new(
                    "coinbase",
                    Decimal::new(60_000, 0),
                    now,
                )),
                ("ETH", "USD") => Ok(crate::model::MarketQuote::new(
                    "coinbase",
                    Decimal::new(3_000, 0),
                    now,
                )),
                _ => Err(ProviderError::UnsupportedPair(format!("{base}/{quote}"))),
            }
        }

        fn fetch_crypto_kraken(
            &self,
            _base: &str,
            _quote: &str,
        ) -> Result<crate::model::MarketQuote, ProviderError> {
            self.kraken_calls.set(self.kraken_calls.get() + 1);
            Err(ProviderError::Transport(
                "kraken disabled in tests".to_string(),
            ))
        }
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 10, 12, 5, 0)
            .single()
            .expect("time")
    }

    fn config_in_tempdir() -> RuntimeConfig {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().to_path_buf();
        std::mem::forget(dir);

        let config = RuntimeConfig {
            cache_dir,
            fx_cache_ttl_secs: crate::config::FX_TTL_SECS,
            crypto_cache_ttl_secs: crate::config::CRYPTO_TTL_SECS,
        };
        seed_icon_files(&config, &["BTC", "ETH", "USD", "JPY"]);
        config
    }

    fn seed_icon_files(config: &RuntimeConfig, symbols: &[&str]) {
        fs::create_dir_all(config.icon_cache_dir()).expect("create icon dir");
        fs::write(
            config
                .icon_cache_dir()
                .join(crate::config::ICON_GENERIC_BASENAME),
            b"generic-icon",
        )
        .expect("write generic");

        for symbol in symbols {
            let file_name = icon_asset_filename(symbol).expect("icon filename");
            fs::write(
                config.icon_cache_dir().join(file_name),
                format!("icon-{symbol}"),
            )
            .expect("write icon");
        }
    }

    fn icon_path(item: &Item) -> Option<&str> {
        item.icon.as_ref().map(|icon| icon.path.as_str())
    }

    #[test]
    fn expression_numeric_mode_returns_single_result_item() {
        let providers = FakeProviders::new();
        let feedback = evaluate_query(&config_in_tempdir(), &providers, fixed_now, "1+5", "USD")
            .expect("must pass");

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].title, "6");
    }

    #[test]
    fn expression_numeric_mode_supports_multiply_and_divide() {
        let providers = FakeProviders::new();
        let feedback = evaluate_query(&config_in_tempdir(), &providers, fixed_now, "8/2*3", "USD")
            .expect("must pass");

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].title, "12");
    }

    #[test]
    fn expression_numeric_mode_rejects_division_by_zero() {
        let providers = FakeProviders::new();
        let err = evaluate_query(&config_in_tempdir(), &providers, fixed_now, "10/0", "USD")
            .expect_err("must fail");

        assert_eq!(err.kind, crate::error::ErrorKind::User);
        assert!(err.message.contains("division by zero"));
    }

    #[test]
    fn asset_expression_rows_include_icon_paths_for_supported_symbols() {
        let providers = FakeProviders::new();
        let feedback = evaluate_query(
            &config_in_tempdir(),
            &providers,
            fixed_now,
            "1 btc + 3 eth to jpy",
            "USD",
        )
        .expect("must pass");

        assert_eq!(feedback.items.len(), 3);
        assert_eq!(feedback.items[0].title, "1 BTC = 10000000 JPY");
        assert_eq!(feedback.items[1].title, "1 ETH = 350000 JPY");
        assert_eq!(feedback.items[2].title, "Total = 11050000 JPY");
        assert!(icon_path(&feedback.items[0]).is_some_and(|path| path.ends_with("btc.png")));
        assert!(icon_path(&feedback.items[1]).is_some_and(|path| path.ends_with("eth.png")));
        assert_eq!(icon_path(&feedback.items[2]), None);
    }

    #[test]
    fn expression_asset_mode_accepts_compact_terms_without_spaces() {
        let providers = FakeProviders::new();
        let feedback = evaluate_query(
            &config_in_tempdir(),
            &providers,
            fixed_now,
            "1btc+3eth to jpy",
            "USD",
        )
        .expect("must pass");

        assert_eq!(feedback.items.len(), 3);
        assert_eq!(feedback.items[0].title, "1 BTC = 10000000 JPY");
        assert_eq!(feedback.items[1].title, "1 ETH = 350000 JPY");
        assert_eq!(feedback.items[2].title, "Total = 11050000 JPY");
        assert!(icon_path(&feedback.items[0]).is_some_and(|path| path.ends_with("btc.png")));
        assert!(icon_path(&feedback.items[1]).is_some_and(|path| path.ends_with("eth.png")));
        assert_eq!(icon_path(&feedback.items[2]), None);
    }

    #[test]
    fn expression_repeated_asset_deduplicates_unit_price_rows() {
        let providers = FakeProviders::new();
        let feedback = evaluate_query(
            &config_in_tempdir(),
            &providers,
            fixed_now,
            "1 btc + 3 btc",
            "USD",
        )
        .expect("must pass");

        assert_eq!(feedback.items.len(), 2);
        assert_eq!(feedback.items[0].title, "1 BTC = 60000 USD");
        assert_eq!(feedback.items[1].title, "Total = 240000 USD");
        assert!(icon_path(&feedback.items[0]).is_some_and(|path| path.ends_with("btc.png")));
        assert_eq!(icon_path(&feedback.items[1]), None);
        assert_eq!(providers.coinbase_calls.get(), 1);
    }

    #[test]
    fn expression_mixed_numeric_and_asset_terms_fail_as_user_error() {
        let providers = FakeProviders::new();
        let err = evaluate_query(
            &config_in_tempdir(),
            &providers,
            fixed_now,
            "2 btc + 5",
            "USD",
        )
        .expect_err("must fail");

        assert_eq!(err.kind, crate::error::ErrorKind::User);
        assert!(err.message.contains("mixed numeric and asset"));
    }

    #[test]
    fn expression_rejects_unsupported_operators() {
        let providers = FakeProviders::new();
        let err = evaluate_query(
            &config_in_tempdir(),
            &providers,
            fixed_now,
            "1 btc * 2 eth",
            "USD",
        )
        .expect_err("must fail");

        assert_eq!(err.kind, crate::error::ErrorKind::User);
        assert!(err.message.contains("unsupported operator"));
    }

    #[test]
    fn expression_compact_terms_do_not_treat_scientific_like_input_as_asset() {
        let providers = FakeProviders::new();
        let err = evaluate_query(&config_in_tempdir(), &providers, fixed_now, "1e2+3", "USD")
            .expect_err("must fail");

        assert_eq!(err.kind, crate::error::ErrorKind::User);
        assert!(
            err.message.contains("invalid asset token") || err.message.contains("invalid token")
        );
    }

    #[test]
    fn expression_requires_complete_to_clause() {
        let providers = FakeProviders::new();
        let err = evaluate_query(
            &config_in_tempdir(),
            &providers,
            fixed_now,
            "1 btc + 2 eth to",
            "USD",
        )
        .expect_err("must fail");

        assert_eq!(err.kind, crate::error::ErrorKind::User);
        assert!(err.message.contains("incomplete to clause"));
    }

    #[test]
    fn expression_rounding_rule_follows_thresholds_and_half_up() {
        assert_eq!(format_market_decimal(Decimal::new(98764, 4)), "9.876");
        assert_eq!(format_market_decimal(Decimal::new(98765, 4)), "9.877");
        assert_eq!(format_market_decimal(Decimal::new(12345, 3)), "12.35");
        assert_eq!(format_market_decimal(Decimal::new(45678, 2)), "456.8");
        assert_eq!(format_market_decimal(Decimal::new(123456, 2)), "1235");
    }

    #[test]
    fn expression_tries_fx_then_crypto_for_three_letter_symbols() {
        let providers = FakeProviders::new();
        let feedback = evaluate_query(
            &config_in_tempdir(),
            &providers,
            fixed_now,
            "1 btc to usd",
            "USD",
        )
        .expect("must pass");

        assert_eq!(feedback.items.len(), 2);
        assert_eq!(providers.fx_calls.get(), 1);
        assert_eq!(providers.coinbase_calls.get(), 1);
    }
}
