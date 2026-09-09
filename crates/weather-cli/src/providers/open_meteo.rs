use chrono::Utc;
use reqwest::blocking::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::RetryPolicy;
use crate::geocoding::ResolvedLocation;

use super::{
    ProviderError, ProviderForecast, ProviderForecastDay, ProviderForecastHour,
    ProviderHourlyForecast, execute_with_retry,
};

const PROVIDER_NAME: &str = "open_meteo";
const GEOCODE_ENDPOINT: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_ENDPOINT: &str = "https://api.open-meteo.com/v1/forecast";
const FORECAST_DAILY_FIELDS: &str =
    "weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max";
const FORECAST_HOURLY_FIELDS: &str = "weather_code,temperature_2m,precipitation_probability";

#[derive(Debug, Serialize)]
struct GeocodeQuery<'a> {
    name: &'a str,
    count: u8,
    language: &'a str,
    format: &'a str,
}

#[derive(Debug, Deserialize)]
struct GeocodeResponse {
    #[serde(default)]
    results: Vec<GeocodeResult>,
}

#[derive(Debug, Deserialize)]
struct GeocodeResult {
    name: String,
    latitude: f64,
    longitude: f64,
    timezone: Option<String>,
}

#[derive(Debug, Serialize)]
struct ForecastQuery<'a> {
    latitude: f64,
    longitude: f64,
    timezone: &'a str,
    forecast_days: usize,
    daily: &'a str,
}

#[derive(Debug, Serialize)]
struct ForecastBatchQuery<'a> {
    latitude: String,
    longitude: String,
    timezone: &'a str,
    forecast_days: usize,
    daily: &'a str,
}

#[derive(Debug, Serialize)]
struct ForecastHourlyQuery<'a> {
    latitude: f64,
    longitude: f64,
    timezone: &'a str,
    forecast_hours: usize,
    hourly: &'a str,
}

#[derive(Debug, Deserialize)]
struct ForecastResponse {
    timezone: Option<String>,
    daily: Option<ForecastDaily>,
}

#[derive(Debug, Deserialize)]
struct ForecastHourlyResponse {
    timezone: Option<String>,
    utc_offset_seconds: Option<i32>,
    hourly: Option<ForecastHourly>,
}

#[derive(Debug, Deserialize)]
struct ForecastDaily {
    #[serde(default)]
    time: Vec<String>,
    #[serde(default)]
    weather_code: Vec<i32>,
    #[serde(default)]
    temperature_2m_max: Vec<f64>,
    #[serde(default)]
    temperature_2m_min: Vec<f64>,
    #[serde(default)]
    precipitation_probability_max: Vec<Option<f64>>,
}

#[derive(Debug, Deserialize)]
struct ForecastHourly {
    #[serde(default)]
    time: Vec<String>,
    #[serde(default)]
    weather_code: Vec<i32>,
    #[serde(default)]
    temperature_2m: Vec<f64>,
    #[serde(default)]
    precipitation_probability: Vec<Option<f64>>,
}

pub fn fetch_geocode(
    client: &Client,
    city: &str,
    retry_policy: RetryPolicy,
) -> Result<ResolvedLocation, ProviderError> {
    fetch_geocode_with_lookup(city, |query| {
        execute_with_retry(
            PROVIDER_NAME,
            retry_policy,
            || fetch_geocode_once(client, query),
            std::thread::sleep,
        )
    })
}

fn fetch_geocode_with_lookup(
    city: &str,
    mut lookup: impl FnMut(&str) -> Result<ResolvedLocation, ProviderError>,
) -> Result<ResolvedLocation, ProviderError> {
    let original = lookup(city);
    if !matches!(&original, Err(ProviderError::NotFound(_)))
        || !matches!(city, "台中" | "臺中" | "台中市" | "臺中市")
    {
        return original;
    }

    // Open-Meteo can omit these local names; keep provider results authoritative.
    match lookup("Taichung,Taiwan") {
        Err(ProviderError::NotFound(_)) => original,
        result => result,
    }
}

pub fn fetch_forecast(
    client: &Client,
    lat: f64,
    lon: f64,
    forecast_days: usize,
    retry_policy: RetryPolicy,
) -> Result<ProviderForecast, ProviderError> {
    execute_with_retry(
        PROVIDER_NAME,
        retry_policy,
        || fetch_forecast_once(client, lat, lon, forecast_days),
        std::thread::sleep,
    )
}

pub fn fetch_forecasts_batch(
    client: &Client,
    locations: &[ResolvedLocation],
    forecast_days: usize,
    retry_policy: RetryPolicy,
) -> Result<Vec<ProviderForecast>, ProviderError> {
    if locations.is_empty() {
        return Ok(Vec::new());
    }

    execute_with_retry(
        PROVIDER_NAME,
        retry_policy,
        || fetch_forecasts_batch_once(client, locations, forecast_days),
        std::thread::sleep,
    )
}

pub fn fetch_hourly_forecast(
    client: &Client,
    lat: f64,
    lon: f64,
    forecast_hours: usize,
    retry_policy: RetryPolicy,
) -> Result<ProviderHourlyForecast, ProviderError> {
    execute_with_retry(
        PROVIDER_NAME,
        retry_policy,
        || fetch_hourly_forecast_once(client, lat, lon, forecast_hours),
        std::thread::sleep,
    )
}

fn fetch_geocode_once(client: &Client, city: &str) -> Result<ResolvedLocation, ProviderError> {
    let query = GeocodeQuery {
        name: city,
        count: 1,
        language: "en",
        format: "json",
    };

    let body = execute_request(client.get(GEOCODE_ENDPOINT).query(&query))?;
    parse_geocode_response(&body, city)
}

fn fetch_forecast_once(
    client: &Client,
    lat: f64,
    lon: f64,
    forecast_days: usize,
) -> Result<ProviderForecast, ProviderError> {
    let query = ForecastQuery {
        latitude: lat,
        longitude: lon,
        timezone: "auto",
        forecast_days,
        daily: FORECAST_DAILY_FIELDS,
    };

    let body = execute_request(client.get(FORECAST_ENDPOINT).query(&query))?;
    parse_forecast_response(&body)
}

fn fetch_forecasts_batch_once(
    client: &Client,
    locations: &[ResolvedLocation],
    forecast_days: usize,
) -> Result<Vec<ProviderForecast>, ProviderError> {
    if locations.len() == 1 {
        let location = &locations[0];
        return fetch_forecast_once(client, location.latitude, location.longitude, forecast_days)
            .map(|forecast| vec![forecast]);
    }

    let query = ForecastBatchQuery {
        latitude: locations
            .iter()
            .map(|location| location.latitude.to_string())
            .collect::<Vec<_>>()
            .join(","),
        longitude: locations
            .iter()
            .map(|location| location.longitude.to_string())
            .collect::<Vec<_>>()
            .join(","),
        timezone: "auto",
        forecast_days,
        daily: FORECAST_DAILY_FIELDS,
    };

    let body = execute_request(client.get(FORECAST_ENDPOINT).query(&query))?;
    parse_forecast_batch_response(&body, locations.len())
}

fn fetch_hourly_forecast_once(
    client: &Client,
    lat: f64,
    lon: f64,
    forecast_hours: usize,
) -> Result<ProviderHourlyForecast, ProviderError> {
    let query = ForecastHourlyQuery {
        latitude: lat,
        longitude: lon,
        timezone: "auto",
        forecast_hours,
        hourly: FORECAST_HOURLY_FIELDS,
    };

    let body = execute_request(client.get(FORECAST_ENDPOINT).query(&query))?;
    parse_hourly_response(&body)
}

fn execute_request(request: RequestBuilder) -> Result<String, ProviderError> {
    let response = request
        .send()
        .map_err(|error| ProviderError::Transport(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| ProviderError::Transport(error.to_string()))?;

    if status.is_success() {
        return Ok(body);
    }

    let message = extract_error_message(&body).unwrap_or_else(|| {
        status
            .canonical_reason()
            .unwrap_or("request failed")
            .to_string()
    });

    Err(ProviderError::Http {
        status: status.as_u16(),
        message,
    })
}

fn parse_geocode_response(body: &str, city: &str) -> Result<ResolvedLocation, ProviderError> {
    let payload: GeocodeResponse = serde_json::from_str(body)
        .map_err(|error| ProviderError::InvalidResponse(format!("geocode payload: {error}")))?;

    let Some(result) = payload.results.into_iter().next() else {
        return Err(ProviderError::NotFound(city.to_string()));
    };

    if result.name.trim().is_empty() {
        return Err(ProviderError::InvalidResponse(
            "geocode payload: empty location name".to_string(),
        ));
    }

    let timezone = result
        .timezone
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ProviderError::InvalidResponse("geocode payload: missing timezone".to_string())
        })?;

    Ok(ResolvedLocation {
        name: result.name,
        latitude: result.latitude,
        longitude: result.longitude,
        timezone,
    })
}

fn parse_forecast_response(body: &str) -> Result<ProviderForecast, ProviderError> {
    let payload: ForecastResponse = serde_json::from_str(body)
        .map_err(|error| ProviderError::InvalidResponse(format!("forecast payload: {error}")))?;
    provider_forecast_from_response(payload)
}

fn parse_forecast_batch_response(
    body: &str,
    expected_count: usize,
) -> Result<Vec<ProviderForecast>, ProviderError> {
    if expected_count == 1 {
        return parse_forecast_response(body).map(|forecast| vec![forecast]);
    }

    let payloads: Vec<ForecastResponse> = serde_json::from_str(body).map_err(|error| {
        ProviderError::InvalidResponse(format!("batch forecast payload: {error}"))
    })?;

    if payloads.len() != expected_count {
        return Err(ProviderError::InvalidResponse(format!(
            "batch forecast payload length mismatch: expected {expected_count}, got {}",
            payloads.len()
        )));
    }

    payloads
        .into_iter()
        .map(provider_forecast_from_response)
        .collect()
}

fn provider_forecast_from_response(
    payload: ForecastResponse,
) -> Result<ProviderForecast, ProviderError> {
    let timezone = payload
        .timezone
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ProviderError::InvalidResponse("forecast payload: missing timezone".to_string())
        })?;

    let daily = payload
        .daily
        .ok_or_else(|| ProviderError::InvalidResponse("forecast payload: missing daily".into()))?;

    let days = build_forecast_days(daily)?;

    Ok(ProviderForecast {
        timezone,
        fetched_at: Utc::now(),
        days,
    })
}

fn parse_hourly_response(body: &str) -> Result<ProviderHourlyForecast, ProviderError> {
    let payload: ForecastHourlyResponse = serde_json::from_str(body)
        .map_err(|error| ProviderError::InvalidResponse(format!("hourly payload: {error}")))?;

    let timezone = payload
        .timezone
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ProviderError::InvalidResponse("hourly payload: missing timezone".to_string())
        })?;

    let hourly = payload
        .hourly
        .ok_or_else(|| ProviderError::InvalidResponse("hourly payload: missing hourly".into()))?;

    let hours = build_forecast_hours(hourly)?;

    Ok(ProviderHourlyForecast {
        timezone,
        utc_offset_seconds: payload.utc_offset_seconds.unwrap_or(0),
        fetched_at: Utc::now(),
        hours,
    })
}

fn build_forecast_days(daily: ForecastDaily) -> Result<Vec<ProviderForecastDay>, ProviderError> {
    let length = daily.time.len();

    if daily.weather_code.len() != length
        || daily.temperature_2m_max.len() != length
        || daily.temperature_2m_min.len() != length
        || daily.precipitation_probability_max.len() != length
    {
        return Err(ProviderError::InvalidResponse(
            "forecast payload: daily arrays length mismatch".to_string(),
        ));
    }

    let mut days = Vec::with_capacity(length);
    for index in 0..length {
        let date = daily.time[index].trim().to_string();
        if date.is_empty() {
            return Err(ProviderError::InvalidResponse(
                "forecast payload: empty date in daily.time".to_string(),
            ));
        }

        let precip = daily.precipitation_probability_max[index].unwrap_or(0.0);
        days.push(ProviderForecastDay {
            date,
            weather_code: daily.weather_code[index],
            temp_max_c: daily.temperature_2m_max[index],
            temp_min_c: daily.temperature_2m_min[index],
            precip_prob_max_pct: clamp_percentage(precip),
        });
    }

    Ok(days)
}

fn build_forecast_hours(
    hourly: ForecastHourly,
) -> Result<Vec<ProviderForecastHour>, ProviderError> {
    let length = hourly.time.len();

    if hourly.weather_code.len() != length
        || hourly.temperature_2m.len() != length
        || hourly.precipitation_probability.len() != length
    {
        return Err(ProviderError::InvalidResponse(
            "hourly payload: hourly arrays length mismatch".to_string(),
        ));
    }

    let mut hours = Vec::with_capacity(length);
    for index in 0..length {
        let datetime = hourly.time[index].trim().to_string();
        if datetime.is_empty() {
            return Err(ProviderError::InvalidResponse(
                "hourly payload: empty datetime in hourly.time".to_string(),
            ));
        }

        let precip = hourly.precipitation_probability[index].unwrap_or(0.0);
        hours.push(ProviderForecastHour {
            datetime,
            weather_code: hourly.weather_code[index],
            temp_c: hourly.temperature_2m[index],
            precip_prob_pct: clamp_percentage(precip),
        });
    }

    Ok(hours)
}

fn clamp_percentage(value: f64) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    value.clamp(0.0, 100.0).round() as u8
}

fn extract_error_message(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }

    let from_json = serde_json::from_str::<Value>(trimmed)
        .ok()
        .and_then(|json| {
            for key in ["reason", "message", "error", "detail", "description"] {
                if let Some(value) = json.get(key).and_then(Value::as_str) {
                    let message = value.trim();
                    if !message.is_empty() {
                        return Some(message.to_string());
                    }
                }
            }
            None
        });

    from_json.or_else(|| Some(trimmed.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geocode_alias_fallback_resolves_each_taichung_alias() {
        let body = r#"{"results":[{"name":"Taichung","latitude":24.1469,"longitude":120.6839,"timezone":"Asia/Taipei"}]}"#;
        for city in ["台中", "臺中", "台中市", "臺中市"] {
            let mut queries = Vec::new();
            let result = fetch_geocode_with_lookup(city, |query| {
                queries.push(query.to_string());
                parse_geocode_response(if query == city { "{}" } else { body }, query)
            });
            assert_eq!(result, parse_geocode_response(body, city), "alias {city}");
            assert_eq!(queries, [city, "Taichung,Taiwan"]);
        }
    }

    #[test]
    fn geocode_alias_fallback_preserves_original_success() {
        let location = ResolvedLocation {
            name: "Provider result".into(),
            latitude: 24.1,
            longitude: 120.6,
            timezone: "Asia/Taipei".into(),
        };
        let mut queries = Vec::new();
        let result = fetch_geocode_with_lookup("台中", |query| {
            queries.push(query.to_string());
            Ok(location.clone())
        });
        assert_eq!(result, Ok(location));
        assert_eq!(queries, ["台中"]);
    }

    #[test]
    fn geocode_alias_fallback_does_not_retry_other_errors() {
        for error in [
            ProviderError::Transport("connection failed".into()),
            ProviderError::Http {
                status: 429,
                message: "rate limited".into(),
            },
            ProviderError::Http {
                status: 503,
                message: "unavailable".into(),
            },
            ProviderError::InvalidResponse("malformed payload".into()),
        ] {
            let mut queries = Vec::new();
            let result = fetch_geocode_with_lookup("台中", |query| {
                queries.push(query.to_string());
                Err(error.clone())
            });
            assert_eq!(result, Err(error));
            assert_eq!(queries, ["台中"]);
        }
    }

    #[test]
    fn geocode_alias_fallback_leaves_unknown_queries_unchanged() {
        for city in ["Nowhere", "台中區", "台中,Taiwan", "Taichung"] {
            let mut queries = Vec::new();
            let result = fetch_geocode_with_lookup(city, |query| {
                queries.push(query.to_string());
                Err(ProviderError::NotFound(query.to_string()))
            });
            assert_eq!(result, Err(ProviderError::NotFound(city.to_string())));
            assert_eq!(queries, [city]);
        }
    }

    #[test]
    fn geocode_alias_fallback_preserves_original_not_found_error() {
        let mut queries = Vec::new();
        let result = fetch_geocode_with_lookup("臺中市", |query| {
            queries.push(query.to_string());
            Err(ProviderError::NotFound(format!("open_meteo: {query}")))
        });
        assert_eq!(
            result,
            Err(ProviderError::NotFound("open_meteo: 臺中市".into()))
        );
        assert_eq!(queries, ["臺中市", "Taichung,Taiwan"]);
    }

    #[test]
    fn geocode_alias_fallback_propagates_fallback_transport_error() {
        let mut queries = Vec::new();
        let error = ProviderError::Transport("open_meteo: connection failed".into());
        let result = fetch_geocode_with_lookup("台中", |query| {
            queries.push(query.to_string());
            if query == "台中" {
                Err(ProviderError::NotFound(query.into()))
            } else {
                Err(error.clone())
            }
        });
        assert_eq!(result, Err(error));
        assert_eq!(queries, ["台中", "Taichung,Taiwan"]);
    }

    #[test]
    fn open_meteo_geocode_parses_first_result() {
        let body = r#"{
            "results": [
                {
                    "name": "Taipei",
                    "latitude": 25.033,
                    "longitude": 121.5654,
                    "timezone": "Asia/Taipei"
                },
                {
                    "name": "Taipei County",
                    "latitude": 25.05,
                    "longitude": 121.52,
                    "timezone": "Asia/Taipei"
                }
            ]
        }"#;

        let location = parse_geocode_response(body, "Taipei").expect("location");
        assert_eq!(location.name, "Taipei");
        assert_eq!(location.latitude, 25.033);
        assert_eq!(location.longitude, 121.5654);
        assert_eq!(location.timezone, "Asia/Taipei");
    }

    #[test]
    fn open_meteo_geocode_returns_not_found_when_empty() {
        let body = r#"{"results":[]}"#;
        let error = parse_geocode_response(body, "Nowhere").expect_err("must fail");

        assert_eq!(error, ProviderError::NotFound("Nowhere".to_string()));
    }

    #[test]
    fn open_meteo_forecast_builds_days_and_clamps_precip() {
        let body = r#"{
            "timezone": "Asia/Taipei",
            "daily": {
                "time": ["2025-02-10", "2025-02-11"],
                "weather_code": [2, 61],
                "temperature_2m_max": [26.4, 24.1],
                "temperature_2m_min": [18.2, 17.0],
                "precipitation_probability_max": [120, -3]
            }
        }"#;

        let forecast = parse_forecast_response(body).expect("forecast");
        assert_eq!(forecast.timezone, "Asia/Taipei");
        assert_eq!(forecast.days.len(), 2);
        assert_eq!(forecast.days[0].precip_prob_max_pct, 100);
        assert_eq!(forecast.days[1].precip_prob_max_pct, 0);
    }

    #[test]
    fn open_meteo_forecast_rejects_mismatched_daily_lengths() {
        let body = r#"{
            "timezone": "Asia/Taipei",
            "daily": {
                "time": ["2025-02-10", "2025-02-11"],
                "weather_code": [2],
                "temperature_2m_max": [26.4, 24.1],
                "temperature_2m_min": [18.2, 17.0],
                "precipitation_probability_max": [30, 50]
            }
        }"#;

        let error = parse_forecast_response(body).expect_err("must fail");
        assert!(
            matches!(error, ProviderError::InvalidResponse(message) if message.contains("length mismatch"))
        );
    }

    #[test]
    fn open_meteo_hourly_builds_hours_and_clamps_precip() {
        let body = r#"{
            "timezone": "Asia/Tokyo",
            "utc_offset_seconds": 32400,
            "hourly": {
                "time": ["2026-02-12T00:00", "2026-02-12T01:00"],
                "weather_code": [3, 61],
                "temperature_2m": [4.4, 3.8],
                "precipitation_probability": [125, -5]
            }
        }"#;

        let forecast = parse_hourly_response(body).expect("hourly");
        assert_eq!(forecast.timezone, "Asia/Tokyo");
        assert_eq!(forecast.utc_offset_seconds, 32400);
        assert_eq!(forecast.hours.len(), 2);
        assert_eq!(forecast.hours[0].precip_prob_pct, 100);
        assert_eq!(forecast.hours[1].precip_prob_pct, 0);
    }

    #[test]
    fn open_meteo_hourly_rejects_mismatched_lengths() {
        let body = r#"{
            "timezone": "Asia/Tokyo",
            "hourly": {
                "time": ["2026-02-12T00:00", "2026-02-12T01:00"],
                "weather_code": [3],
                "temperature_2m": [4.4, 3.8],
                "precipitation_probability": [20, 30]
            }
        }"#;

        let error = parse_hourly_response(body).expect_err("must fail");
        assert!(
            matches!(error, ProviderError::InvalidResponse(message) if message.contains("length mismatch"))
        );
    }

    #[test]
    fn open_meteo_extract_error_message_prefers_reason() {
        let body = r#"{"error": true, "reason": "rate limit exceeded"}"#;
        assert_eq!(
            extract_error_message(body),
            Some("rate limit exceeded".to_string())
        );
    }
}
