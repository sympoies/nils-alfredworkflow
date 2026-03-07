use std::time::Duration;

use reqwest::blocking::Client;
use thiserror::Error;

use crate::config::{PROVIDER_TIMEOUT_SECS, RetryPolicy};
use crate::model::MarketQuote;

pub mod coinbase;
pub mod floatrates;
pub mod frankfurter;
pub mod kraken;

pub trait ProviderApi {
    fn fetch_fx_rate(&self, base: &str, quote: &str) -> Result<MarketQuote, ProviderError>;
    fn fetch_crypto_coinbase(&self, base: &str, quote: &str) -> Result<MarketQuote, ProviderError>;
    fn fetch_crypto_kraken(&self, base: &str, quote: &str) -> Result<MarketQuote, ProviderError>;
}

#[derive(Debug, Clone)]
pub struct HttpProviders {
    client: Client,
    retry_policy: RetryPolicy,
}

impl HttpProviders {
    pub fn new() -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(PROVIDER_TIMEOUT_SECS))
            .build()
            .map_err(|error| ProviderError::Transport(error.to_string()))?;

        Ok(Self {
            client,
            retry_policy: RetryPolicy::default(),
        })
    }

    pub fn with_retry_policy(retry_policy: RetryPolicy) -> Result<Self, ProviderError> {
        let mut providers = Self::new()?;
        providers.retry_policy = retry_policy;
        Ok(providers)
    }
}

impl ProviderApi for HttpProviders {
    fn fetch_fx_rate(&self, base: &str, quote: &str) -> Result<MarketQuote, ProviderError> {
        resolve_fx_with_fallback(
            frankfurter::fetch_fx_rate(&self.client, base, quote, self.retry_policy),
            || floatrates::fetch_fx_rate(&self.client, base, quote, self.retry_policy),
        )
    }

    fn fetch_crypto_coinbase(&self, base: &str, quote: &str) -> Result<MarketQuote, ProviderError> {
        coinbase::fetch_crypto_spot(&self.client, base, quote, self.retry_policy)
    }

    fn fetch_crypto_kraken(&self, base: &str, quote: &str) -> Result<MarketQuote, ProviderError> {
        kraken::fetch_crypto_spot(&self.client, base, quote, self.retry_policy)
    }
}

fn resolve_fx_with_fallback<F>(
    primary: Result<MarketQuote, ProviderError>,
    fallback: F,
) -> Result<MarketQuote, ProviderError>
where
    F: FnOnce() -> Result<MarketQuote, ProviderError>,
{
    match primary {
        Ok(quote) => Ok(quote),
        Err(primary_error) => match fallback() {
            Ok(quote) => Ok(quote),
            Err(fallback_error) => Err(ProviderError::InvalidResponse(format!(
                "primary provider failed ({primary_error}); fallback provider failed ({fallback_error})"
            ))),
        },
    }
}

pub fn execute_with_retry<T, F, S>(
    provider_name: &'static str,
    policy: RetryPolicy,
    mut operation: F,
    mut sleep_fn: S,
) -> Result<T, ProviderError>
where
    F: FnMut() -> Result<T, ProviderError>,
    S: FnMut(Duration),
{
    // Retry is intentionally bounded to protect unauthenticated endpoints:
    // attempt 1 + (max_attempts-1) retries with deterministic exponential backoff.
    let max_attempts = policy.max_attempts.max(1);

    for attempt in 1..=max_attempts {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => {
                if !error.retryable() || attempt == max_attempts {
                    return Err(error.with_provider(provider_name));
                }

                let delay = policy.backoff_for_attempt(attempt + 1);
                sleep_fn(Duration::from_millis(delay));
            }
        }
    }

    Err(ProviderError::InvalidResponse(format!(
        "{provider_name}: exhausted retry attempts"
    )))
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("http error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid provider response: {0}")]
    InvalidResponse(String),
    #[error("unsupported trading pair: {0}")]
    UnsupportedPair(String),
}

impl ProviderError {
    pub fn retryable(&self) -> bool {
        match self {
            ProviderError::Transport(_) => true,
            ProviderError::Http { status, .. } => *status == 429 || (500..=599).contains(status),
            ProviderError::InvalidResponse(_) => false,
            ProviderError::UnsupportedPair(_) => false,
        }
    }

    pub fn with_provider(self, provider: &'static str) -> Self {
        match self {
            ProviderError::Transport(message) => {
                ProviderError::Transport(format!("{provider}: {message}"))
            }
            ProviderError::Http { status, message } => ProviderError::Http {
                status,
                message: format!("{provider}: {message}"),
            },
            ProviderError::InvalidResponse(message) => {
                ProviderError::InvalidResponse(format!("{provider}: {message}"))
            }
            ProviderError::UnsupportedPair(message) => {
                ProviderError::UnsupportedPair(format!("{provider}: {message}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    #[test]
    fn provider_retries_transient_failures_with_backoff() {
        let attempts = Rc::new(RefCell::new(0usize));
        let observed_sleep = Rc::new(RefCell::new(Vec::<u64>::new()));
        let attempts_for_op = Rc::clone(&attempts);
        let sleeps_for_op = Rc::clone(&observed_sleep);

        let result = execute_with_retry(
            "test-provider",
            RetryPolicy {
                max_attempts: 3,
                base_backoff_ms: 5,
            },
            move || {
                let mut value = attempts_for_op.borrow_mut();
                *value += 1;
                if *value < 3 {
                    return Err(ProviderError::Transport("timeout".to_string()));
                }
                Ok("ok")
            },
            move |delay| sleeps_for_op.borrow_mut().push(delay.as_millis() as u64),
        )
        .expect("should succeed on third attempt");

        assert_eq!(result, "ok");
        assert_eq!(*attempts.borrow(), 3);
        assert_eq!(*observed_sleep.borrow(), vec![5, 10]);
    }

    #[test]
    fn provider_does_not_retry_non_retryable_failures() {
        let attempts = Rc::new(RefCell::new(0usize));
        let attempts_for_op = Rc::clone(&attempts);

        let error = execute_with_retry(
            "test-provider",
            RetryPolicy {
                max_attempts: 3,
                base_backoff_ms: 5,
            },
            move || {
                let mut value = attempts_for_op.borrow_mut();
                *value += 1;
                Result::<(), ProviderError>::Err(ProviderError::UnsupportedPair(
                    "ABCUSD".to_string(),
                ))
            },
            |_| {},
        )
        .expect_err("must fail");

        assert_eq!(
            error,
            ProviderError::UnsupportedPair("test-provider: ABCUSD".to_string())
        );
        assert_eq!(*attempts.borrow(), 1);
    }

    #[test]
    fn provider_error_mapping_marks_retryable_http_statuses() {
        assert!(
            ProviderError::Http {
                status: 429,
                message: "rate limit".to_string()
            }
            .retryable()
        );

        assert!(
            ProviderError::Http {
                status: 503,
                message: "unavailable".to_string()
            }
            .retryable()
        );

        assert!(
            !ProviderError::Http {
                status: 400,
                message: "bad request".to_string()
            }
            .retryable()
        );
    }

    #[test]
    fn fx_provider_falls_back_when_primary_fails() {
        let now = chrono::Utc::now();
        let result = resolve_fx_with_fallback(
            Err(ProviderError::Http {
                status: 404,
                message: "frankfurter: not found".to_string(),
            }),
            || {
                Ok(MarketQuote::new(
                    "floatrates",
                    rust_decimal::Decimal::new(318, 1),
                    now,
                ))
            },
        )
        .expect("fallback should succeed");

        assert_eq!(result.provider, "floatrates");
        assert_eq!(result.unit_price.to_string(), "31.8");
    }

    #[test]
    fn fx_provider_surfaces_both_primary_and_fallback_failures() {
        let error = resolve_fx_with_fallback(
            Err(ProviderError::Http {
                status: 404,
                message: "frankfurter: not found".to_string(),
            }),
            || {
                Err(ProviderError::UnsupportedPair(
                    "floatrates: TWD".to_string(),
                ))
            },
        )
        .expect_err("both providers should fail");

        assert!(matches!(error, ProviderError::InvalidResponse(_)));
        assert!(error.to_string().contains("primary provider failed"));
        assert!(error.to_string().contains("fallback provider failed"));
    }
}
