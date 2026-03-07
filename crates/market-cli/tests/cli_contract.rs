use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use chrono::{TimeZone, Utc};
use market_cli::cache::{CacheRecord, cache_path, write_cache};
use market_cli::config::{
    CRYPTO_TTL_SECS, FX_TTL_SECS, MARKET_CACHE_DIR_ENV, MARKET_CRYPTO_CACHE_TTL_ENV,
    MARKET_FX_CACHE_TTL_ENV, RuntimeConfig,
};
use market_cli::model::{
    CacheMetadata, CacheStatus, MarketKind, MarketQuote, MarketRequest, build_output,
};
use serde_json::Value;

const BTC_ICON_FIXTURE: &[u8] = include_bytes!("fixtures/icons/btc.png");
const GENERIC_ICON_FIXTURE: &[u8] = include_bytes!("fixtures/icons/generic.png");

fn run_cli(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(resolve_cli_path());
    cmd.args(args);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().expect("run market-cli")
}

#[test]
fn config_icon_cache_dir_is_versioned() {
    let config = RuntimeConfig {
        cache_dir: PathBuf::from("/tmp/market-cache"),
        fx_cache_ttl_secs: FX_TTL_SECS,
        crypto_cache_ttl_secs: CRYPTO_TTL_SECS,
    };

    assert_eq!(
        config.icon_cache_dir(),
        PathBuf::from("/tmp/market-cache")
            .join("market-cli")
            .join("icons")
            .join("cryptocurrency-icons")
            .join("0.18.1")
            .join("32/color")
    );
}

#[test]
fn cli_contract_output_contains_required_fields_for_fx() {
    let request = MarketRequest::new(MarketKind::Fx, "USD", "TWD", "100").expect("request");
    let quote = MarketQuote::new(
        "frankfurter",
        rust_decimal::Decimal::new(321, 1),
        Utc.with_ymd_and_hms(2026, 2, 10, 12, 0, 0)
            .single()
            .expect("time"),
    );

    let output = build_output(
        &request,
        &quote,
        CacheMetadata {
            status: CacheStatus::Live,
            key: "fx-usd-twd".to_string(),
            ttl_secs: 86400,
            age_secs: 0,
        },
    );
    let value = serde_json::to_value(output).expect("json");

    for field in [
        "kind",
        "base",
        "quote",
        "amount",
        "unit_price",
        "converted",
        "provider",
        "fetched_at",
        "cache",
    ] {
        assert!(value.get(field).is_some(), "missing field: {field}");
    }
}

#[test]
fn cli_contract_cache_status_serializes_in_snake_case() {
    let request = MarketRequest::new(MarketKind::Crypto, "BTC", "USD", "0.5").expect("request");
    let quote = MarketQuote::new(
        "coinbase",
        rust_decimal::Decimal::new(68000, 0),
        Utc.with_ymd_and_hms(2026, 2, 10, 12, 0, 0)
            .single()
            .expect("time"),
    );

    let output = build_output(
        &request,
        &quote,
        CacheMetadata {
            status: CacheStatus::CacheStaleFallback,
            key: "crypto-btc-usd".to_string(),
            ttl_secs: 300,
            age_secs: 900,
        },
    );

    let value: Value = serde_json::to_value(output).expect("json");
    assert_eq!(
        value
            .get("cache")
            .and_then(|cache| cache.get("status"))
            .and_then(Value::as_str),
        Some("cache_stale_fallback")
    );
}

#[test]
fn cli_contract_amount_validation_rejects_zero() {
    let err = MarketRequest::new(MarketKind::Fx, "USD", "TWD", "0").expect_err("must fail");
    assert!(err.to_string().contains("amount must be positive"));
}

#[test]
fn service_json_error_envelope_has_required_keys() {
    let output = run_cli(
        &[
            "fx", "--base", "USD", "--quote", "TWD", "--amount", "0", "--json",
        ],
        &[("MARKET_TEST_SECRET", "unused")],
    );
    assert_eq!(output.status.code(), Some(2));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(
        json.get("schema_version").and_then(Value::as_str),
        Some("v1")
    );
    assert_eq!(
        json.get("command").and_then(Value::as_str),
        Some("market.fx")
    );
    assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("user.invalid_input")
    );
    assert!(
        json.get("error")
            .and_then(|error| error.get("details"))
            .is_some()
    );
}

#[test]
fn service_json_error_envelope_redacts_secret_like_input() {
    let secret = "market-contract-secret-123";
    let amount = format!("token={secret}");
    let output = run_cli(
        &[
            "fx", "--base", "USD", "--quote", "TWD", "--amount", &amount, "--json",
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(2));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains(secret));
    assert!(!stderr.contains(secret));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
    assert!(
        json.get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .is_some()
    );
}

#[test]
fn cli_contract_fx_alfred_row_falls_back_to_generic_icon_when_symbol_icon_missing() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Fx,
        "TWD",
        "USD",
        "frankfurter",
        "0.031",
    );
    seed_icon_cache(cache_dir.path(), "generic.png", GENERIC_ICON_FIXTURE);

    let cache_dir_value = cache_dir.path().display().to_string();
    let output = run_cli(
        &[
            "fx",
            "--base",
            "TWD",
            "--quote",
            "USD",
            "--amount",
            "100",
            "--output",
            "alfred-json",
        ],
        &[(MARKET_CACHE_DIR_ENV, cache_dir_value.as_str())],
    );

    assert_eq!(output.status.code(), Some(0));
    let items = alfred_items(&output);
    assert_eq!(items.len(), 1);
    assert_icon_path_suffix(&items[0], "/generic.png");
}

#[test]
fn cli_contract_favorites_human_output_preserves_mixed_separator_order() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "btc\neth,usd,jpy",
            "--default-fiat",
            "USD",
            "--output",
            "human",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        favorites_human_output(&output),
        "favorites: BTC, ETH, USD, JPY"
    );
}

#[test]
fn cli_contract_favorites_human_output_supports_explicit_fx_pairs() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "jpy/usd,jpy/twd,btc",
            "--default-fiat",
            "USD",
            "--output",
            "human",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        favorites_human_output(&output),
        "favorites: JPY/USD, JPY/TWD, BTC"
    );
}

#[test]
fn cli_contract_favorites_alfred_rows_include_prompt_and_quotes() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Fx,
        "JPY",
        "USD",
        "frankfurter",
        "0.0067",
    );
    seed_icon_cache(cache_dir.path(), "usd.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "jpy.png", BTC_ICON_FIXTURE);
    let cache_dir_value = cache_dir.path().display().to_string();
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "usd,jpy",
            "--default-fiat",
            "USD",
            "--output",
            "alfred-json",
        ],
        &[(MARKET_CACHE_DIR_ENV, cache_dir_value.as_str())],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        favorite_item_titles(&output),
        vec![
            "Enter a market expression",
            "1 USD = 1 USD",
            "1 JPY = 0.007 USD",
        ]
    );
    for item in favorite_items(&output) {
        assert_eq!(item.get("valid").and_then(Value::as_bool), Some(false));
    }
    let items = favorite_items(&output);
    assert!(items[0].get("icon").is_none());
    assert_icon_path_suffix(&items[1], "/usd.png");
    assert_icon_path_suffix(&items[2], "/jpy.png");
}

#[test]
fn cli_contract_favorites_alfred_rows_include_explicit_fx_pairs() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Fx,
        "JPY",
        "USD",
        "frankfurter",
        "0.0067",
    );
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Fx,
        "JPY",
        "TWD",
        "frankfurter",
        "2.15",
    );
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Fx,
        "USD",
        "JPY",
        "frankfurter",
        "150.25",
    );
    seed_icon_cache(cache_dir.path(), "jpy.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "usd.png", BTC_ICON_FIXTURE);

    let cache_dir_value = cache_dir.path().display().to_string();
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "jpy/usd,jpy/twd,usd/jpy",
            "--default-fiat",
            "USD",
            "--output",
            "alfred-json",
        ],
        &[(MARKET_CACHE_DIR_ENV, cache_dir_value.as_str())],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        favorite_item_titles(&output),
        vec![
            "Enter a market expression",
            "1 JPY = 0.007 USD",
            "1 JPY = 2.150 TWD",
            "1 USD = 150.3 JPY",
        ]
    );
    let items = favorite_items(&output);
    assert_eq!(
        items[1].get("uid").and_then(Value::as_str),
        Some("market-favorite-jpy-usd")
    );
    assert_eq!(
        items[2].get("uid").and_then(Value::as_str),
        Some("market-favorite-jpy-twd")
    );
    assert_eq!(
        items[3].get("uid").and_then(Value::as_str),
        Some("market-favorite-usd-jpy")
    );
    assert_icon_path_suffix(&items[1], "/jpy.png");
    assert_icon_path_suffix(&items[2], "/jpy.png");
    assert_icon_path_suffix(&items[3], "/usd.png");
}

#[test]
fn favorites_rows_include_icon_paths_for_supported_symbols() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Crypto,
        "BTC",
        "USD",
        "coinbase",
        "68194",
    );
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Crypto,
        "ETH",
        "USD",
        "coinbase",
        "1980",
    );
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Fx,
        "JPY",
        "USD",
        "frankfurter",
        "0.0067",
    );
    seed_icon_cache(cache_dir.path(), "btc.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "eth.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "usd.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "jpy.png", BTC_ICON_FIXTURE);

    let cache_dir_value = cache_dir.path().display().to_string();
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "btc,eth,usd,jpy",
            "--default-fiat",
            "USD",
            "--output",
            "alfred-json",
        ],
        &[(MARKET_CACHE_DIR_ENV, cache_dir_value.as_str())],
    );

    assert_eq!(output.status.code(), Some(0));
    let items = favorite_items(&output);
    assert_icon_path_suffix(&items[1], "/btc.png");
    assert_icon_path_suffix(&items[2], "/eth.png");
    assert_icon_path_suffix(&items[3], "/usd.png");
    assert_icon_path_suffix(&items[4], "/jpy.png");
}

#[test]
fn asset_expression_rows_include_icon_paths_for_supported_symbols() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Crypto,
        "BTC",
        "USD",
        "coinbase",
        "68194",
    );
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Crypto,
        "ETH",
        "USD",
        "coinbase",
        "1980",
    );
    seed_icon_cache(cache_dir.path(), "btc.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "eth.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "generic.png", GENERIC_ICON_FIXTURE);

    let cache_dir_value = cache_dir.path().display().to_string();
    let output = run_cli(
        &[
            "expr",
            "--query",
            "1 btc + 3 eth to usd",
            "--output",
            "alfred-json",
        ],
        &[(MARKET_CACHE_DIR_ENV, cache_dir_value.as_str())],
    );

    assert_eq!(output.status.code(), Some(0));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    let items = json
        .get("items")
        .and_then(Value::as_array)
        .expect("items should be array");

    assert_icon_path_suffix(&items[0], "/btc.png");
    assert_icon_path_suffix(&items[1], "/eth.png");
    assert!(
        items[2].get("icon").is_none(),
        "total row should not have icon"
    );
}

#[test]
fn cli_contract_favorites_dedup_preserves_first_occurrence_in_human_mode() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "btc,eth,btc,usd",
            "--default-fiat",
            "USD",
            "--output",
            "human",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(favorites_human_output(&output), "favorites: BTC, ETH, USD");
}

#[test]
fn cli_contract_favorites_dedup_uses_effective_base_quote_pair() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "usd,usd/twd,jpy/twd,jpy",
            "--default-fiat",
            "TWD",
            "--output",
            "human",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(favorites_human_output(&output), "favorites: USD, JPY/TWD");
}

#[test]
fn cli_contract_favorites_empty_list_falls_back_to_default_set_in_human_mode() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "",
            "--default-fiat",
            "TWD",
            "--output",
            "human",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        favorites_human_output(&output),
        "favorites: BTC, ETH, TWD, JPY"
    );
}

#[test]
fn cli_contract_favorites_delimiter_only_list_falls_back_to_default_set_in_human_mode() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            ", ,\n,,",
            "--default-fiat",
            "USD",
            "--output",
            "human",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        favorites_human_output(&output),
        "favorites: BTC, ETH, USD, JPY"
    );
}

#[test]
fn cli_contract_favorites_backslash_n_delimiter_only_list_falls_back_to_default_set_in_human_mode()
{
    let output = run_cli(
        &[
            "favorites",
            "--list",
            ", ,\\n,,",
            "--default-fiat",
            "USD",
            "--output",
            "human",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        favorites_human_output(&output),
        "favorites: BTC, ETH, USD, JPY"
    );
}

#[test]
fn cli_contract_favorites_backslash_n_and_duplicates_preserve_first_occurrence_in_human_mode() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "btc\\neth,BTC,usd,eth,jpy",
            "--default-fiat",
            "USD",
            "--output",
            "human",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        favorites_human_output(&output),
        "favorites: BTC, ETH, USD, JPY"
    );
}

#[test]
fn cli_contract_favorites_duplicate_default_fiat_keeps_stable_fallback_order_in_human_mode() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            ", ,\\n,,",
            "--default-fiat",
            "JPY",
            "--output",
            "human",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(favorites_human_output(&output), "favorites: BTC, ETH, JPY");
}

#[test]
fn service_json_error_envelope_for_invalid_favorites_token_has_required_keys() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "btc,eth!",
            "--default-fiat",
            "USD",
            "--json",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(2));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(
        json.get("schema_version").and_then(Value::as_str),
        Some("v1")
    );
    assert_eq!(
        json.get("command").and_then(Value::as_str),
        Some("market.favorites")
    );
    assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("user.invalid_input")
    );
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("details"))
            .and_then(|details| details.get("kind"))
            .and_then(Value::as_str),
        Some("user")
    );
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("details"))
            .and_then(|details| details.get("exit_code"))
            .and_then(Value::as_i64),
        Some(2)
    );
    assert!(
        json.get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("invalid favorite symbol"))
    );
}

#[test]
fn service_json_error_envelope_for_invalid_favorites_default_fiat_has_required_keys() {
    let output = run_cli(
        &[
            "favorites",
            "--list",
            "",
            "--default-fiat",
            "USDT",
            "--json",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(2));

    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(
        json.get("schema_version").and_then(Value::as_str),
        Some("v1")
    );
    assert_eq!(
        json.get("command").and_then(Value::as_str),
        Some("market.favorites")
    );
    assert_eq!(json.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
        Some("user.invalid_input")
    );
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("details"))
            .and_then(|details| details.get("kind"))
            .and_then(Value::as_str),
        Some("user")
    );
    assert_eq!(
        json.get("error")
            .and_then(|error| error.get("details"))
            .and_then(|details| details.get("exit_code"))
            .and_then(Value::as_i64),
        Some(2)
    );
    assert!(
        json.get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("invalid default_fiat symbol"))
    );
}

#[test]
fn cli_contract_favorites_json_envelope_matches_alfred_output() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Crypto,
        "BTC",
        "USD",
        "coinbase",
        "68194",
    );
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Crypto,
        "ETH",
        "USD",
        "coinbase",
        "1980",
    );
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Fx,
        "JPY",
        "USD",
        "frankfurter",
        "0.0067",
    );
    seed_icon_cache(cache_dir.path(), "btc.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "eth.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "usd.png", BTC_ICON_FIXTURE);
    seed_icon_cache(cache_dir.path(), "jpy.png", BTC_ICON_FIXTURE);
    let cache_dir_value = cache_dir.path().display().to_string();
    let alfred_output = run_cli(
        &[
            "favorites",
            "--list",
            "usd,jpy",
            "--default-fiat",
            "USD",
            "--output",
            "alfred-json",
        ],
        &[(MARKET_CACHE_DIR_ENV, cache_dir_value.as_str())],
    );
    let json_output = run_cli(
        &[
            "favorites",
            "--list",
            "usd,jpy",
            "--default-fiat",
            "USD",
            "--json",
        ],
        &[(MARKET_CACHE_DIR_ENV, cache_dir_value.as_str())],
    );

    assert_eq!(alfred_output.status.code(), Some(0));
    assert_eq!(json_output.status.code(), Some(0));

    let direct: Value =
        serde_json::from_slice(&alfred_output.stdout).expect("alfred stdout should be json");
    let envelope: Value =
        serde_json::from_slice(&json_output.stdout).expect("json stdout should be json");

    assert_eq!(
        envelope.get("schema_version").and_then(Value::as_str),
        Some("v1")
    );
    assert_eq!(
        envelope.get("command").and_then(Value::as_str),
        Some("market.favorites")
    );
    assert_eq!(envelope.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(envelope.get("result"), Some(&direct));
}

#[test]
fn cli_contract_fx_cache_ttl_override_applies_to_cached_fx_json_output() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Fx,
        "USD",
        "TWD",
        "frankfurter",
        "32.1",
    );
    let cache_dir_value = cache_dir.path().display().to_string();
    let output = run_cli(
        &[
            "fx", "--base", "USD", "--quote", "TWD", "--amount", "100", "--json",
        ],
        &[
            (MARKET_CACHE_DIR_ENV, cache_dir_value.as_str()),
            (MARKET_FX_CACHE_TTL_ENV, "15m"),
        ],
    );

    assert_eq!(output.status.code(), Some(0));

    let envelope: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(envelope.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        envelope
            .get("result")
            .and_then(|result| result.get("cache"))
            .and_then(|cache| cache.get("ttl_secs"))
            .and_then(Value::as_u64),
        Some(900)
    );
}

#[test]
fn cli_contract_crypto_cache_ttl_override_applies_to_cached_crypto_json_output() {
    let cache_dir = tempfile::tempdir().expect("tempdir");
    seed_cache_record(
        cache_dir.path(),
        MarketKind::Crypto,
        "BTC",
        "USD",
        "coinbase",
        "68194",
    );
    let cache_dir_value = cache_dir.path().display().to_string();
    let output = run_cli(
        &[
            "crypto", "--base", "BTC", "--quote", "USD", "--amount", "1", "--json",
        ],
        &[
            (MARKET_CACHE_DIR_ENV, cache_dir_value.as_str()),
            (MARKET_CRYPTO_CACHE_TTL_ENV, "1h"),
        ],
    );

    assert_eq!(output.status.code(), Some(0));

    let envelope: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");
    assert_eq!(envelope.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        envelope
            .get("result")
            .and_then(|result| result.get("cache"))
            .and_then(|cache| cache.get("ttl_secs"))
            .and_then(Value::as_u64),
        Some(3600)
    );
}

fn favorites_human_output(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn favorite_item_titles(output: &Output) -> Vec<String> {
    favorite_items(output)
        .iter()
        .map(|item| {
            item.get("title")
                .and_then(Value::as_str)
                .expect("title should exist")
                .to_string()
        })
        .collect()
}

fn alfred_items(output: &Output) -> Vec<Value> {
    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be json");

    json.get("items")
        .and_then(Value::as_array)
        .expect("items should be array")
        .to_vec()
}

fn favorite_items(output: &Output) -> Vec<Value> {
    alfred_items(output)
}

fn assert_icon_path_suffix(item: &Value, expected_suffix: &str) {
    let path = item
        .get("icon")
        .and_then(|icon| icon.get("path"))
        .and_then(Value::as_str)
        .expect("icon.path should exist");
    assert!(
        path.ends_with(expected_suffix),
        "expected icon path to end with {expected_suffix}, got {path}"
    );
}

fn seed_cache_record(
    cache_dir: &Path,
    kind: MarketKind,
    base: &str,
    quote: &str,
    provider: &str,
    unit_price: &str,
) {
    let config = RuntimeConfig {
        cache_dir: cache_dir.to_path_buf(),
        fx_cache_ttl_secs: FX_TTL_SECS,
        crypto_cache_ttl_secs: CRYPTO_TTL_SECS,
    };
    let path = cache_path(&config, kind, base, quote);
    let record = CacheRecord {
        base: base.to_string(),
        quote: quote.to_string(),
        provider: provider.to_string(),
        unit_price: unit_price.to_string(),
        fetched_at: Utc::now().to_rfc3339(),
    };

    write_cache(&path, &record).expect("write cache");
}

fn seed_icon_cache(cache_dir: &Path, filename: &str, bytes: &[u8]) {
    let config = RuntimeConfig {
        cache_dir: cache_dir.to_path_buf(),
        fx_cache_ttl_secs: FX_TTL_SECS,
        crypto_cache_ttl_secs: CRYPTO_TTL_SECS,
    };
    let path = config.icon_cache_dir().join(filename);
    let parent = path.parent().expect("icon cache path parent");
    std::fs::create_dir_all(parent).expect("create icon cache dir");
    std::fs::write(path, bytes).expect("write icon cache");
}

fn resolve_cli_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_market-cli") {
        return PathBuf::from(path);
    }

    if let Ok(current_exe) = std::env::current_exe()
        && let Some(debug_dir) = current_exe.parent().and_then(|deps| deps.parent())
    {
        let candidate = debug_dir.join(format!("market-cli{}", std::env::consts::EXE_SUFFIX));
        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from(env!("CARGO_BIN_EXE_market-cli"))
}
