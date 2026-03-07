use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::cache::{
    CacheRecord, cache_key, cache_path, evaluate_freshness, parse_fetched_at, read_cache,
    write_cache,
};
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::model::{
    CacheMetadata, CacheStatus, MarketOutput, MarketQuote, MarketRequest, build_output,
};
use crate::providers::ProviderApi;

pub fn resolve_market<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    request: &MarketRequest,
) -> Result<MarketOutput, AppError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc>,
{
    let now = now_fn();
    let path = cache_path(config, request.kind, &request.base, &request.quote);
    let key = cache_key(request.kind, &request.base, &request.quote);
    let ttl_secs = config.cache_ttl_secs_for_kind(request.kind);

    let cached = read_cache(&path).map_err(|error| AppError::runtime(error.to_string()))?;
    let cached_state = cached
        .as_ref()
        .and_then(|record| parse_cache_quote(record).map(|quote| (record, quote)))
        .map(|(record, quote)| {
            let freshness = evaluate_freshness(record, now, ttl_secs);
            (quote, freshness.age_secs, freshness.is_fresh)
        });

    if let Some((cached_quote, age_secs, true)) = &cached_state {
        return Ok(build_output(
            request,
            cached_quote,
            CacheMetadata {
                status: CacheStatus::CacheFresh,
                key,
                ttl_secs,
                age_secs: *age_secs,
            },
        ));
    }

    match request.kind {
        crate::model::MarketKind::Fx => {
            resolve_fx(request, providers, now, &path, ttl_secs, cached_state, key)
        }
        crate::model::MarketKind::Crypto => {
            resolve_crypto(request, providers, now, &path, ttl_secs, cached_state, key)
        }
    }
}

fn resolve_fx<P: ProviderApi>(
    request: &MarketRequest,
    providers: &P,
    now: DateTime<Utc>,
    cache_path: &std::path::Path,
    ttl_secs: u64,
    cached_state: Option<(MarketQuote, u64, bool)>,
    key: String,
) -> Result<MarketOutput, AppError> {
    match providers.fetch_fx_rate(&request.base, &request.quote) {
        Ok(quote) => build_live_output(request, quote, now, cache_path, ttl_secs, key),
        Err(error) => fallback_or_error(
            "failed to fetch fx rate",
            vec![format!("fx provider: {error}")],
            request,
            cached_state,
            ttl_secs,
            key,
        ),
    }
}

fn resolve_crypto<P: ProviderApi>(
    request: &MarketRequest,
    providers: &P,
    now: DateTime<Utc>,
    cache_path: &std::path::Path,
    ttl_secs: u64,
    cached_state: Option<(MarketQuote, u64, bool)>,
    key: String,
) -> Result<MarketOutput, AppError> {
    let mut trace = Vec::new();

    match providers.fetch_crypto_coinbase(&request.base, &request.quote) {
        Ok(quote) => return build_live_output(request, quote, now, cache_path, ttl_secs, key),
        Err(error) => trace.push(format!("coinbase: {error}")),
    }

    match providers.fetch_crypto_kraken(&request.base, &request.quote) {
        Ok(quote) => build_live_output(request, quote, now, cache_path, ttl_secs, key),
        Err(error) => {
            trace.push(format!("kraken: {error}"));
            fallback_or_error(
                "failed to fetch crypto spot price",
                trace,
                request,
                cached_state,
                ttl_secs,
                key,
            )
        }
    }
}

fn build_live_output(
    request: &MarketRequest,
    quote: MarketQuote,
    now: DateTime<Utc>,
    path: &std::path::Path,
    ttl_secs: u64,
    key: String,
) -> Result<MarketOutput, AppError> {
    let record = CacheRecord {
        base: request.base.clone(),
        quote: request.quote.clone(),
        provider: quote.provider.clone(),
        unit_price: quote.unit_price.normalize().to_string(),
        fetched_at: quote.fetched_at.to_rfc3339(),
    };

    write_cache(path, &record).map_err(|error| AppError::runtime(error.to_string()))?;

    let output = build_output(
        request,
        &MarketQuote::new(quote.provider, quote.unit_price, now),
        CacheMetadata {
            status: CacheStatus::Live,
            key,
            ttl_secs,
            age_secs: 0,
        },
    );
    Ok(output)
}

fn fallback_or_error(
    prefix: &str,
    trace: Vec<String>,
    request: &MarketRequest,
    cached_state: Option<(MarketQuote, u64, bool)>,
    ttl_secs: u64,
    key: String,
) -> Result<MarketOutput, AppError> {
    if let Some((quote, age_secs, false)) = cached_state {
        return Ok(build_output(
            request,
            &quote,
            CacheMetadata {
                status: CacheStatus::CacheStaleFallback,
                key,
                ttl_secs,
                age_secs,
            },
        ));
    }

    Err(AppError::runtime_with_trace(prefix, &trace))
}

fn parse_cache_quote(record: &CacheRecord) -> Option<MarketQuote> {
    let fetched_at = parse_fetched_at(record)?;
    let unit_price = record.unit_price.parse::<Decimal>().ok()?;
    Some(MarketQuote::new(
        record.provider.clone(),
        unit_price,
        fetched_at,
    ))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::model::{MarketKind, MarketRequest};
    use crate::providers::ProviderError;

    struct FakeProviders {
        fx_result: Result<MarketQuote, ProviderError>,
        coinbase_result: Result<MarketQuote, ProviderError>,
        kraken_result: Result<MarketQuote, ProviderError>,
        fx_calls: Cell<usize>,
        coinbase_calls: Cell<usize>,
        kraken_calls: Cell<usize>,
    }

    impl FakeProviders {
        fn new() -> Self {
            let now = Utc
                .with_ymd_and_hms(2026, 2, 10, 12, 0, 0)
                .single()
                .expect("time");
            Self {
                fx_result: Ok(MarketQuote::new("frankfurter", Decimal::new(321, 1), now)),
                coinbase_result: Ok(MarketQuote::new("coinbase", Decimal::new(670001, 1), now)),
                kraken_result: Ok(MarketQuote::new("kraken", Decimal::new(669999, 1), now)),
                fx_calls: Cell::new(0),
                coinbase_calls: Cell::new(0),
                kraken_calls: Cell::new(0),
            }
        }
    }

    impl ProviderApi for FakeProviders {
        fn fetch_fx_rate(&self, _base: &str, _quote: &str) -> Result<MarketQuote, ProviderError> {
            self.fx_calls.set(self.fx_calls.get() + 1);
            self.fx_result.clone()
        }

        fn fetch_crypto_coinbase(
            &self,
            _base: &str,
            _quote: &str,
        ) -> Result<MarketQuote, ProviderError> {
            self.coinbase_calls.set(self.coinbase_calls.get() + 1);
            self.coinbase_result.clone()
        }

        fn fetch_crypto_kraken(
            &self,
            _base: &str,
            _quote: &str,
        ) -> Result<MarketQuote, ProviderError> {
            self.kraken_calls.set(self.kraken_calls.get() + 1);
            self.kraken_result.clone()
        }
    }

    fn fixture_config(cache_dir: PathBuf) -> RuntimeConfig {
        RuntimeConfig {
            cache_dir,
            fx_cache_ttl_secs: crate::config::FX_TTL_SECS,
            crypto_cache_ttl_secs: crate::config::CRYPTO_TTL_SECS,
        }
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 10, 12, 5, 0)
            .single()
            .expect("time")
    }

    #[test]
    fn service_crypto_falls_back_to_kraken() {
        let dir = tempfile::tempdir().expect("tempdir");
        let providers = FakeProviders {
            coinbase_result: Err(ProviderError::Transport("timeout".to_string())),
            ..FakeProviders::new()
        };
        let request = MarketRequest::new(MarketKind::Crypto, "BTC", "USD", "1").expect("request");

        let output = resolve_market(
            &fixture_config(dir.path().to_path_buf()),
            &providers,
            fixed_now,
            &request,
        )
        .expect("must succeed");

        assert_eq!(output.provider, "kraken");
        assert_eq!(providers.coinbase_calls.get(), 1);
        assert_eq!(providers.kraken_calls.get(), 1);
    }

    #[test]
    fn service_short_circuits_on_fresh_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = fixture_config(dir.path().to_path_buf());
        let request = MarketRequest::new(MarketKind::Fx, "USD", "TWD", "100").expect("request");
        let path = cache_path(&config, request.kind, &request.base, &request.quote);
        let record = CacheRecord {
            base: "USD".to_string(),
            quote: "TWD".to_string(),
            provider: "frankfurter".to_string(),
            unit_price: "32".to_string(),
            fetched_at: "2026-02-10T12:04:00Z".to_string(),
        };
        write_cache(&path, &record).expect("cache write");

        let providers = FakeProviders::new();
        let output = resolve_market(&config, &providers, fixed_now, &request).expect("must pass");

        assert_eq!(output.cache.status, CacheStatus::CacheFresh);
        assert_eq!(providers.fx_calls.get(), 0);
    }

    #[test]
    fn service_writes_cache_after_live_success() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = fixture_config(dir.path().to_path_buf());
        let providers = FakeProviders::new();
        let request = MarketRequest::new(MarketKind::Fx, "USD", "TWD", "1").expect("request");

        let output = resolve_market(&config, &providers, fixed_now, &request).expect("must pass");
        assert_eq!(output.cache.status, CacheStatus::Live);

        let path = cache_path(&config, request.kind, &request.base, &request.quote);
        let cached = read_cache(&path).expect("read").expect("cache exists");
        assert_eq!(cached.provider, "frankfurter");
    }

    #[test]
    fn service_uses_stale_cache_on_provider_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = fixture_config(dir.path().to_path_buf());
        let request = MarketRequest::new(MarketKind::Crypto, "BTC", "USD", "1").expect("request");
        let path = cache_path(&config, request.kind, &request.base, &request.quote);

        let stale = CacheRecord {
            base: "BTC".to_string(),
            quote: "USD".to_string(),
            provider: "coinbase".to_string(),
            unit_price: "60000".to_string(),
            fetched_at: "2026-02-10T11:00:00Z".to_string(),
        };
        write_cache(&path, &stale).expect("write");

        let providers = FakeProviders {
            coinbase_result: Err(ProviderError::Transport("timeout".to_string())),
            kraken_result: Err(ProviderError::Transport("unavailable".to_string())),
            ..FakeProviders::new()
        };
        let output = resolve_market(&config, &providers, fixed_now, &request).expect("fallback");

        assert_eq!(output.cache.status, CacheStatus::CacheStaleFallback);
        assert_eq!(output.provider, "coinbase");
    }

    #[test]
    fn service_fails_without_cache_when_all_providers_fail() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = fixture_config(dir.path().to_path_buf());
        let request = MarketRequest::new(MarketKind::Crypto, "BTC", "USD", "1").expect("request");
        let providers = FakeProviders {
            coinbase_result: Err(ProviderError::Transport("timeout".to_string())),
            kraken_result: Err(ProviderError::Http {
                status: 503,
                message: "service unavailable".to_string(),
            }),
            ..FakeProviders::new()
        };

        let err = resolve_market(&config, &providers, fixed_now, &request).expect_err("must fail");
        assert_eq!(err.kind, crate::error::ErrorKind::Runtime);
        assert!(err.message.contains("provider trace"));
    }

    #[test]
    fn service_stale_payload_preserves_provider_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = fixture_config(dir.path().to_path_buf());
        let request = MarketRequest::new(MarketKind::Fx, "USD", "JPY", "2").expect("request");
        let path = cache_path(&config, request.kind, &request.base, &request.quote);

        write_cache(
            &path,
            &CacheRecord {
                base: "USD".to_string(),
                quote: "JPY".to_string(),
                provider: "frankfurter".to_string(),
                unit_price: "149.5".to_string(),
                fetched_at: "2026-02-09T00:00:00Z".to_string(),
            },
        )
        .expect("write");

        let providers = FakeProviders {
            fx_result: Err(ProviderError::Transport("timeout".to_string())),
            ..FakeProviders::new()
        };

        let output = resolve_market(&config, &providers, fixed_now, &request).expect("fallback");
        assert_eq!(output.provider, "frankfurter");
        assert_eq!(output.unit_price, "149.5");
    }
}
