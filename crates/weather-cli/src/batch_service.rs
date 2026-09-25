use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};

use crate::cache::{
    CacheRecord, cache_path, evaluate_freshness, parse_fetched_at, read_cache, write_cache,
};
use crate::config::RuntimeConfig;
use crate::error::AppError;
use crate::geocoding::{
    ResolvedLocation, coordinate_label, read_cached_city_location, write_cached_city_location,
};
use crate::model::{
    CacheMetadata, ForecastBatchEntry, ForecastBatchOutput, ForecastDay, ForecastOutput,
    ForecastPeriod, FreshnessStatus, normalize_cities,
};
use crate::providers::{ProviderApi, ProviderForecast, ProviderForecastDay};
use crate::weather_code;

pub fn resolve_forecast_batch<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    period: ForecastPeriod,
    raw_cities: &[String],
) -> Result<ForecastBatchOutput, AppError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc>,
{
    let cities = normalize_cities(raw_cities.iter().map(String::as_str)).map_err(AppError::from)?;
    let now = now_fn();
    let locations = resolve_locations(config, providers, &cities);

    let mut entries = std::iter::repeat_with(|| None)
        .take(cities.len())
        .collect::<Vec<Option<ForecastBatchEntry>>>();
    let mut pending = Vec::new();
    let mut listed_places = HashSet::new();

    for (index, city) in cities.iter().enumerate() {
        let location = match &locations[index] {
            Ok(location) => location.clone(),
            Err(message) => {
                entries[index] = Some(error_entry(city, message.clone()));
                continue;
            }
        };
        // Two spellings of one place geocode together; list it once, where it
        // first appears.
        if !listed_places.insert(coordinate_label(location.latitude, location.longitude)) {
            continue;
        }

        let cache_key = location.cache_key();
        let path = cache_path(&config.cache_dir, period, &cache_key);
        let output_context = OutputContext {
            period,
            cache_key,
            ttl_secs: config.cache_ttl_secs,
        };

        let cached = match read_cache(&path) {
            Ok(cached) => cached,
            Err(error) => {
                entries[index] = Some(error_entry(city, error.to_string()));
                continue;
            }
        };
        let cached_state = cached.as_ref().map(|record| {
            let freshness = evaluate_freshness(record, now, config.cache_ttl_secs);
            (record.clone(), freshness.age_secs, freshness.is_fresh)
        });

        if let Some((record, age_secs, true)) = &cached_state {
            entries[index] = Some(success_entry(
                city,
                build_output_from_record(
                    record,
                    &location,
                    FreshnessStatus::CacheFresh,
                    *age_secs,
                    &output_context,
                ),
            ));
            continue;
        }

        pending.push(PendingForecast {
            index,
            city: city.clone(),
            location,
            path,
            cached_state,
            output_context,
        });
    }

    if !pending.is_empty() {
        resolve_pending_forecasts(providers, &mut entries, pending)?;
    }

    Ok(ForecastBatchOutput {
        period,
        entries: entries.into_iter().flatten().collect(),
    })
}

fn resolve_locations<P: ProviderApi>(
    config: &RuntimeConfig,
    providers: &P,
    cities: &[String],
) -> Vec<Result<ResolvedLocation, String>> {
    let mut resolved = std::iter::repeat_with(|| None)
        .take(cities.len())
        .collect::<Vec<Option<Result<ResolvedLocation, String>>>>();
    let mut misses = Vec::new();

    for (index, city) in cities.iter().enumerate() {
        if let Ok(Some(cached)) = read_cached_city_location(&config.cache_dir, city) {
            resolved[index] = Some(Ok(cached));
            continue;
        }
        misses.push((index, city.clone()));
    }

    if !misses.is_empty() {
        let miss_cities = misses
            .iter()
            .map(|(_, city)| city.clone())
            .collect::<Vec<_>>();
        let geocoded = providers.geocode_cities(&miss_cities);

        for (miss_index, (result_index, city)) in misses.iter().enumerate() {
            let result = geocoded.get(miss_index).cloned().unwrap_or_else(|| {
                Err(crate::providers::ProviderError::Transport(
                    "missing parallel geocode result".to_string(),
                ))
            });

            resolved[*result_index] = Some(match result {
                Ok(location) => {
                    let _ = write_cached_city_location(&config.cache_dir, city, &location);
                    Ok(location)
                }
                Err(error) => Err(format!("failed to resolve city '{city}': {error}")),
            });
        }
    }

    resolved
        .into_iter()
        .zip(cities)
        .map(|(result, city)| {
            result.unwrap_or_else(|| {
                Err(format!(
                    "failed to resolve city '{city}': missing geocode batch result"
                ))
            })
        })
        .collect()
}

fn resolve_pending_forecasts<P: ProviderApi>(
    providers: &P,
    entries: &mut [Option<ForecastBatchEntry>],
    pending: Vec<PendingForecast>,
) -> Result<(), AppError> {
    let locations = pending
        .iter()
        .map(|item| item.location.clone())
        .collect::<Vec<_>>();
    let forecast_days = pending[0].output_context.period.forecast_days();

    match providers.fetch_open_meteo_forecasts_batch(&locations, forecast_days) {
        Ok(forecasts) => {
            if forecasts.len() != pending.len() {
                let error = format!(
                    "open_meteo batch result length mismatch: expected {}, got {}",
                    pending.len(),
                    forecasts.len()
                );
                resolve_met_no_fallback(providers, entries, pending, &error, forecast_days);
                return Ok(());
            }

            for (item, forecast) in pending.into_iter().zip(forecasts) {
                let entry = match build_live_output(
                    &item.path,
                    &item.location,
                    forecast,
                    "open_meteo",
                    Vec::new(),
                    &item.output_context,
                ) {
                    Ok(output) => success_entry(&item.city, output),
                    Err(error) => error_entry(&item.city, error.message),
                };
                entries[item.index] = Some(entry);
            }
        }
        Err(error) => {
            resolve_met_no_fallback(
                providers,
                entries,
                pending,
                &format!("open_meteo: {error}"),
                forecast_days,
            );
        }
    }

    Ok(())
}

fn resolve_met_no_fallback<P: ProviderApi>(
    providers: &P,
    entries: &mut [Option<ForecastBatchEntry>],
    pending: Vec<PendingForecast>,
    open_meteo_error: &str,
    forecast_days: usize,
) {
    let locations = pending
        .iter()
        .map(|item| item.location.clone())
        .collect::<Vec<_>>();
    let fallback_results = providers.fetch_met_no_forecasts_batch(&locations, forecast_days);

    for (index, item) in pending.into_iter().enumerate() {
        let entry_index = item.index;
        let source_trace = vec![open_meteo_error.to_string()];
        let entry = match fallback_results.get(index) {
            Some(Ok(forecast)) => match build_live_output(
                &item.path,
                &item.location,
                forecast.clone(),
                "met_no",
                source_trace,
                &item.output_context,
            ) {
                Ok(output) => success_entry(&item.city, output),
                Err(error) => error_entry(&item.city, error.message),
            },
            Some(Err(error)) => fallback_or_error(item, open_meteo_error, error),
            None => fallback_or_error_missing(item, open_meteo_error),
        };
        entries[entry_index] = Some(entry);
    }
}

fn fallback_or_error(
    item: PendingForecast,
    open_meteo_error: &str,
    met_no_error: &crate::providers::ProviderError,
) -> ForecastBatchEntry {
    let trace = vec![
        open_meteo_error.to_string(),
        format!("met_no: {met_no_error}"),
    ];

    match item.cached_state {
        Some((record, age_secs, false)) => success_entry(
            &item.city,
            build_output_from_record(
                &record,
                &item.location,
                FreshnessStatus::CacheStaleFallback,
                age_secs,
                &item.output_context,
            ),
        ),
        _ => error_entry(
            &item.city,
            AppError::runtime_with_trace("failed to fetch forecast from providers", &trace).message,
        ),
    }
}

fn fallback_or_error_missing(item: PendingForecast, open_meteo_error: &str) -> ForecastBatchEntry {
    let trace = vec![
        open_meteo_error.to_string(),
        "met_no: missing batch result".to_string(),
    ];

    match item.cached_state {
        Some((record, age_secs, false)) => success_entry(
            &item.city,
            build_output_from_record(
                &record,
                &item.location,
                FreshnessStatus::CacheStaleFallback,
                age_secs,
                &item.output_context,
            ),
        ),
        _ => error_entry(
            &item.city,
            AppError::runtime_with_trace("failed to fetch forecast from providers", &trace).message,
        ),
    }
}

fn build_live_output(
    path: &Path,
    location: &ResolvedLocation,
    provider_forecast: ProviderForecast,
    source: &str,
    source_trace: Vec<String>,
    output_context: &OutputContext,
) -> Result<ForecastOutput, AppError> {
    let ProviderForecast {
        timezone: provider_timezone,
        fetched_at,
        days,
    } = provider_forecast;

    let timezone = if provider_timezone.trim().is_empty() {
        location.timezone.clone()
    } else {
        provider_timezone
    };

    let record = CacheRecord {
        period: output_context.period,
        location: location.to_output_location(),
        timezone,
        forecast: normalize_days(days),
        source: source.to_string(),
        source_trace,
        fetched_at: fetched_at.to_rfc3339_opts(SecondsFormat::Secs, true),
    };

    write_cache(path, &record).map_err(|error| AppError::runtime(error.to_string()))?;

    Ok(build_output_from_record(
        &record,
        location,
        FreshnessStatus::Live,
        0,
        output_context,
    ))
}

fn build_output_from_record(
    record: &CacheRecord,
    location: &ResolvedLocation,
    freshness_status: FreshnessStatus,
    age_secs: u64,
    output_context: &OutputContext,
) -> ForecastOutput {
    let fetched_at = parse_fetched_at(record)
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(SecondsFormat::Secs, true);

    ForecastOutput {
        period: output_context.period,
        location: location.to_output_location(),
        timezone: record.timezone.clone(),
        forecast: record.forecast.clone(),
        source: record.source.clone(),
        source_trace: record.source_trace.clone(),
        fetched_at,
        freshness: CacheMetadata {
            status: freshness_status,
            key: output_context.cache_key.clone(),
            ttl_secs: output_context.ttl_secs,
            age_secs,
        },
    }
}

fn normalize_days(days: Vec<ProviderForecastDay>) -> Vec<ForecastDay> {
    days.into_iter()
        .map(|item| ForecastDay {
            date: item.date,
            weather_code: item.weather_code,
            summary_zh: weather_code::summary_zh(item.weather_code).to_string(),
            temp_min_c: round1(item.temp_min_c),
            temp_max_c: round1(item.temp_max_c),
            precip_prob_max_pct: item.precip_prob_max_pct.min(100),
        })
        .collect()
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn success_entry(city: &str, output: ForecastOutput) -> ForecastBatchEntry {
    ForecastBatchEntry {
        city: city.to_string(),
        result: Some(output),
        error: None,
    }
}

fn error_entry(city: &str, message: String) -> ForecastBatchEntry {
    ForecastBatchEntry {
        city: city.to_string(),
        result: None,
        error: Some(message),
    }
}

#[derive(Debug, Clone)]
struct OutputContext {
    period: ForecastPeriod,
    cache_key: String,
    ttl_secs: u64,
}

#[derive(Debug, Clone)]
struct PendingForecast {
    index: usize,
    city: String,
    location: ResolvedLocation,
    path: PathBuf,
    cached_state: Option<(CacheRecord, u64, bool)>,
    output_context: OutputContext,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use chrono::TimeZone;

    use super::*;
    use crate::providers::{ProviderError, ProviderForecast};

    struct BatchProviders {
        geocode_calls: RefCell<Vec<String>>,
        batch_open_calls: RefCell<usize>,
        batch_met_calls: RefCell<usize>,
        locations: HashMap<String, ResolvedLocation>,
        open_results: HashMap<String, ProviderForecast>,
        open_batch_error: Option<ProviderError>,
        met_results: HashMap<String, Result<ProviderForecast, ProviderError>>,
    }

    impl BatchProviders {
        fn ok() -> Self {
            let now = Utc
                .with_ymd_and_hms(2026, 2, 11, 0, 0, 0)
                .single()
                .expect("time");

            let taipei = ResolvedLocation {
                name: "Taipei".to_string(),
                latitude: 25.033,
                longitude: 121.5654,
                timezone: "Asia/Taipei".to_string(),
            };
            let tokyo = ResolvedLocation {
                name: "Tokyo".to_string(),
                latitude: 35.6762,
                longitude: 139.6503,
                timezone: "Asia/Tokyo".to_string(),
            };

            let forecast_for = |weather_code, min, max, precip| ProviderForecast {
                timezone: "UTC".to_string(),
                fetched_at: now,
                days: vec![ProviderForecastDay {
                    date: "2026-02-11".to_string(),
                    weather_code,
                    temp_min_c: min,
                    temp_max_c: max,
                    precip_prob_max_pct: precip,
                }],
            };

            Self {
                geocode_calls: RefCell::new(Vec::new()),
                batch_open_calls: RefCell::new(0),
                batch_met_calls: RefCell::new(0),
                locations: HashMap::from([
                    ("Taipei".to_string(), taipei.clone()),
                    ("Tokyo".to_string(), tokyo.clone()),
                ]),
                open_results: HashMap::from([
                    (taipei.cache_key(), forecast_for(3, 14.4, 20.1, 20)),
                    (tokyo.cache_key(), forecast_for(2, 5.1, 12.7, 10)),
                ]),
                open_batch_error: None,
                met_results: HashMap::new(),
            }
        }
    }

    impl ProviderApi for BatchProviders {
        fn geocode_city(&self, city: &str) -> Result<ResolvedLocation, ProviderError> {
            self.geocode_calls.borrow_mut().push(city.to_string());
            self.locations
                .get(city)
                .cloned()
                .ok_or_else(|| ProviderError::NotFound(city.to_string()))
        }

        fn geocode_cities(
            &self,
            cities: &[String],
        ) -> Vec<Result<ResolvedLocation, ProviderError>> {
            cities.iter().map(|city| self.geocode_city(city)).collect()
        }

        fn fetch_open_meteo_forecast(
            &self,
            _lat: f64,
            _lon: f64,
            _forecast_days: usize,
        ) -> Result<ProviderForecast, ProviderError> {
            Err(ProviderError::Transport("unused".to_string()))
        }

        fn fetch_open_meteo_forecasts_batch(
            &self,
            locations: &[ResolvedLocation],
            _forecast_days: usize,
        ) -> Result<Vec<ProviderForecast>, ProviderError> {
            *self.batch_open_calls.borrow_mut() += 1;
            if let Some(error) = &self.open_batch_error {
                return Err(error.clone());
            }

            locations
                .iter()
                .map(|location| {
                    self.open_results
                        .get(&location.cache_key())
                        .cloned()
                        .ok_or_else(|| ProviderError::NotFound(location.name.clone()))
                })
                .collect()
        }

        fn fetch_open_meteo_hourly_forecast(
            &self,
            _lat: f64,
            _lon: f64,
            _forecast_hours: usize,
        ) -> Result<crate::providers::ProviderHourlyForecast, ProviderError> {
            Err(ProviderError::Transport("unused".to_string()))
        }

        fn fetch_met_no_forecast(
            &self,
            _lat: f64,
            _lon: f64,
            _forecast_days: usize,
        ) -> Result<ProviderForecast, ProviderError> {
            Err(ProviderError::Transport("unused".to_string()))
        }

        fn fetch_met_no_forecasts_batch(
            &self,
            locations: &[ResolvedLocation],
            _forecast_days: usize,
        ) -> Vec<Result<ProviderForecast, ProviderError>> {
            *self.batch_met_calls.borrow_mut() += 1;
            locations
                .iter()
                .map(|location| {
                    self.met_results
                        .get(&location.cache_key())
                        .cloned()
                        .unwrap_or_else(|| {
                            Err(ProviderError::Transport("met_no unavailable".to_string()))
                        })
                })
                .collect()
        }
    }

    fn config_in_tempdir() -> RuntimeConfig {
        RuntimeConfig {
            cache_dir: tempfile::tempdir().expect("tempdir").path().to_path_buf(),
            cache_ttl_secs: crate::config::WEATHER_CACHE_TTL_SECS,
        }
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 11, 0, 5, 0)
            .single()
            .expect("time")
    }

    #[test]
    fn batch_service_uses_single_open_meteo_batch_call_for_multiple_cities() {
        let providers = BatchProviders::ok();
        let config = config_in_tempdir();
        let cities = vec!["Taipei".to_string(), "Tokyo".to_string()];

        let output = resolve_forecast_batch(
            &config,
            &providers,
            fixed_now,
            ForecastPeriod::Today,
            &cities,
        )
        .expect("batch output");

        assert_eq!(output.entries.len(), 2);
        assert_eq!(*providers.batch_open_calls.borrow(), 1);
        assert_eq!(*providers.batch_met_calls.borrow(), 0);
        assert_eq!(
            output.entries[0].result.as_ref().and_then(|result| result
                .location
                .name
                .as_str()
                .into()),
            Some("Taipei")
        );
        assert_eq!(
            output.entries[1].result.as_ref().and_then(|result| result
                .location
                .name
                .as_str()
                .into()),
            Some("Tokyo")
        );
    }

    #[test]
    fn batch_service_lists_cities_resolving_to_one_place_once() {
        let mut providers = BatchProviders::ok();
        // Only the rounded coordinates decide a match, whatever the resolved
        // name: the alias differs in name and below the fourth decimal.
        let mut alias = providers.locations["Taipei"].clone();
        alias.name = "臺北市".to_string();
        alias.latitude += 0.00004;
        providers.open_results.insert(
            alias.cache_key(),
            providers.open_results[&providers.locations["Taipei"].cache_key()].clone(),
        );
        providers
            .locations
            .insert("Taipei, Taiwan".to_string(), alias);
        let config = config_in_tempdir();
        let cities = vec![
            "Taipei, Taiwan".to_string(),
            "Tokyo".to_string(),
            "Taipei".to_string(),
        ];

        let output = resolve_forecast_batch(
            &config,
            &providers,
            fixed_now,
            ForecastPeriod::Today,
            &cities,
        )
        .expect("batch output");

        let listed = output
            .entries
            .iter()
            .map(|entry| entry.city.as_str())
            .collect::<Vec<_>>();
        assert_eq!(listed, vec!["Taipei, Taiwan", "Tokyo"]);
    }

    #[test]
    fn batch_service_falls_back_to_met_no_or_stale_cache_per_city() {
        let mut providers = BatchProviders::ok();
        providers.open_batch_error = Some(ProviderError::Transport("timeout".to_string()));
        providers.met_results.insert(
            providers.locations["Taipei"].cache_key(),
            Ok(providers.open_results[&providers.locations["Taipei"].cache_key()].clone()),
        );

        let config = config_in_tempdir();
        let stale_location = providers.locations["Tokyo"].clone();
        let stale_path = cache_path(
            &config.cache_dir,
            ForecastPeriod::Today,
            &stale_location.cache_key(),
        );
        let stale_record = CacheRecord {
            period: ForecastPeriod::Today,
            location: stale_location.to_output_location(),
            timezone: stale_location.timezone.clone(),
            forecast: vec![ForecastDay {
                date: "2026-02-11".to_string(),
                weather_code: 3,
                summary_zh: "陰天".to_string(),
                temp_min_c: 9.9,
                temp_max_c: 15.2,
                precip_prob_max_pct: 30,
            }],
            source: "open_meteo".to_string(),
            source_trace: Vec::new(),
            fetched_at: "2026-02-10T00:00:00Z".to_string(),
        };
        write_cache(&stale_path, &stale_record).expect("write stale cache");

        let cities = vec!["Taipei".to_string(), "Tokyo".to_string()];
        let output = resolve_forecast_batch(
            &config,
            &providers,
            fixed_now,
            ForecastPeriod::Today,
            &cities,
        )
        .expect("batch output");

        assert_eq!(*providers.batch_open_calls.borrow(), 1);
        assert_eq!(*providers.batch_met_calls.borrow(), 1);
        assert_eq!(
            output.entries[0]
                .result
                .as_ref()
                .map(|result| result.source.as_str()),
            Some("met_no")
        );
        assert_eq!(
            output.entries[1]
                .result
                .as_ref()
                .map(|result| result.freshness.status),
            Some(FreshnessStatus::CacheStaleFallback)
        );
    }
}
