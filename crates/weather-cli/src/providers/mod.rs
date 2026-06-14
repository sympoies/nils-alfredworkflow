use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

use crate::config::{PROVIDER_TIMEOUT_SECS, RetryPolicy};
use crate::geocoding::ResolvedLocation;

pub mod met_no;
pub mod open_meteo;

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderForecastDay {
    pub date: String,
    pub weather_code: i32,
    pub temp_min_c: f64,
    pub temp_max_c: f64,
    pub precip_prob_max_pct: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderForecast {
    pub timezone: String,
    pub fetched_at: DateTime<Utc>,
    pub days: Vec<ProviderForecastDay>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderForecastHour {
    pub datetime: String,
    pub weather_code: i32,
    pub temp_c: f64,
    pub precip_prob_pct: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderHourlyForecast {
    pub timezone: String,
    pub utc_offset_seconds: i32,
    pub fetched_at: DateTime<Utc>,
    pub hours: Vec<ProviderForecastHour>,
}

pub trait ProviderApi {
    fn geocode_city(&self, city: &str) -> Result<ResolvedLocation, ProviderError>;
    fn geocode_cities(&self, cities: &[String]) -> Vec<Result<ResolvedLocation, ProviderError>> {
        cities.iter().map(|city| self.geocode_city(city)).collect()
    }
    fn fetch_open_meteo_forecast(
        &self,
        lat: f64,
        lon: f64,
        forecast_days: usize,
    ) -> Result<ProviderForecast, ProviderError>;
    fn fetch_open_meteo_forecasts_batch(
        &self,
        locations: &[ResolvedLocation],
        forecast_days: usize,
    ) -> Result<Vec<ProviderForecast>, ProviderError> {
        locations
            .iter()
            .map(|location| {
                self.fetch_open_meteo_forecast(location.latitude, location.longitude, forecast_days)
            })
            .collect()
    }
    fn fetch_open_meteo_hourly_forecast(
        &self,
        lat: f64,
        lon: f64,
        forecast_hours: usize,
    ) -> Result<ProviderHourlyForecast, ProviderError>;
    fn fetch_met_no_forecast(
        &self,
        lat: f64,
        lon: f64,
        forecast_days: usize,
    ) -> Result<ProviderForecast, ProviderError>;
    fn fetch_met_no_forecasts_batch(
        &self,
        locations: &[ResolvedLocation],
        forecast_days: usize,
    ) -> Vec<Result<ProviderForecast, ProviderError>> {
        locations
            .iter()
            .map(|location| {
                self.fetch_met_no_forecast(location.latitude, location.longitude, forecast_days)
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct HttpProviders {
    client: Client,
    retry_policy: RetryPolicy,
}

impl HttpProviders {
    pub fn new() -> Result<Self, ProviderError> {
        let client = build_blocking_client(None, Some(Duration::from_secs(PROVIDER_TIMEOUT_SECS)))
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
    fn geocode_city(&self, city: &str) -> Result<ResolvedLocation, ProviderError> {
        open_meteo::fetch_geocode(&self.client, city, self.retry_policy)
    }

    fn geocode_cities(&self, cities: &[String]) -> Vec<Result<ResolvedLocation, ProviderError>> {
        let (tx, rx) = mpsc::channel();

        thread::scope(|scope| {
            for (index, city) in cities.iter().cloned().enumerate() {
                let tx = tx.clone();
                let providers = self.clone();
                scope.spawn(move || {
                    let result =
                        open_meteo::fetch_geocode(&providers.client, &city, providers.retry_policy);
                    let _ = tx.send((index, result));
                });
            }
            drop(tx);

            let mut ordered = std::iter::repeat_with(|| None)
                .take(cities.len())
                .collect::<Vec<_>>();
            for (index, result) in rx {
                ordered[index] = Some(result);
            }

            ordered
                .into_iter()
                .map(|result| {
                    result.unwrap_or_else(|| {
                        Err(ProviderError::Transport(
                            "missing parallel geocode result".to_string(),
                        ))
                    })
                })
                .collect()
        })
    }

    fn fetch_open_meteo_forecast(
        &self,
        lat: f64,
        lon: f64,
        forecast_days: usize,
    ) -> Result<ProviderForecast, ProviderError> {
        open_meteo::fetch_forecast(&self.client, lat, lon, forecast_days, self.retry_policy)
    }

    fn fetch_open_meteo_forecasts_batch(
        &self,
        locations: &[ResolvedLocation],
        forecast_days: usize,
    ) -> Result<Vec<ProviderForecast>, ProviderError> {
        open_meteo::fetch_forecasts_batch(&self.client, locations, forecast_days, self.retry_policy)
    }

    fn fetch_open_meteo_hourly_forecast(
        &self,
        lat: f64,
        lon: f64,
        forecast_hours: usize,
    ) -> Result<ProviderHourlyForecast, ProviderError> {
        open_meteo::fetch_hourly_forecast(&self.client, lat, lon, forecast_hours, self.retry_policy)
    }

    fn fetch_met_no_forecast(
        &self,
        lat: f64,
        lon: f64,
        forecast_days: usize,
    ) -> Result<ProviderForecast, ProviderError> {
        met_no::fetch_forecast(&self.client, lat, lon, forecast_days, self.retry_policy)
    }

    fn fetch_met_no_forecasts_batch(
        &self,
        locations: &[ResolvedLocation],
        forecast_days: usize,
    ) -> Vec<Result<ProviderForecast, ProviderError>> {
        let (tx, rx) = mpsc::channel();

        thread::scope(|scope| {
            for (index, location) in locations.iter().cloned().enumerate() {
                let tx = tx.clone();
                let providers = self.clone();
                scope.spawn(move || {
                    let result = met_no::fetch_forecast(
                        &providers.client,
                        location.latitude,
                        location.longitude,
                        forecast_days,
                        providers.retry_policy,
                    );
                    let _ = tx.send((index, result));
                });
            }
            drop(tx);

            let mut ordered = std::iter::repeat_with(|| None)
                .take(locations.len())
                .collect::<Vec<_>>();
            for (index, result) in rx {
                ordered[index] = Some(result);
            }

            ordered
                .into_iter()
                .map(|result| {
                    result.unwrap_or_else(|| {
                        Err(ProviderError::Transport(
                            "missing parallel met.no result".to_string(),
                        ))
                    })
                })
                .collect()
        })
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
    #[error("location not found: {0}")]
    NotFound(String),
}

impl ProviderError {
    pub fn retryable(&self) -> bool {
        match self {
            ProviderError::Transport(_) => true,
            ProviderError::Http { status, .. } => *status == 429 || (500..=599).contains(status),
            ProviderError::InvalidResponse(_) => false,
            ProviderError::NotFound(_) => false,
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
            ProviderError::NotFound(message) => {
                ProviderError::NotFound(format!("{provider}: {message}"))
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
    fn timeout_retry_bounds_are_applied() {
        let attempts = Rc::new(RefCell::new(0usize));
        let observed_sleep = Rc::new(RefCell::new(Vec::<u64>::new()));
        let attempts_for_op = Rc::clone(&attempts);
        let sleeps_for_op = Rc::clone(&observed_sleep);

        let result = execute_with_retry(
            "test-provider",
            RetryPolicy {
                max_attempts: 2,
                base_backoff_ms: 25,
            },
            move || {
                let mut value = attempts_for_op.borrow_mut();
                *value += 1;
                if *value < 2 {
                    return Err(ProviderError::Transport("timeout".to_string()));
                }
                Ok("ok")
            },
            move |delay| sleeps_for_op.borrow_mut().push(delay.as_millis() as u64),
        )
        .expect("should succeed on retry");

        assert_eq!(result, "ok");
        assert_eq!(*attempts.borrow(), 2);
        assert_eq!(*observed_sleep.borrow(), vec![25]);
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
}
