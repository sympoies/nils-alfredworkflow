use std::collections::HashMap;

use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::config::RetryPolicy;
use crate::model::MarketQuote;

use super::{ProviderError, execute_with_retry};

const ENDPOINT_BASE: &str = "https://www.floatrates.com/daily";

pub fn fetch_fx_rate(
    client: &Client,
    base: &str,
    quote: &str,
    retry_policy: RetryPolicy,
) -> Result<MarketQuote, ProviderError> {
    execute_with_retry(
        "floatrates",
        retry_policy,
        || fetch_once(client, base, quote),
        std::thread::sleep,
    )
}

fn fetch_once(client: &Client, base: &str, quote: &str) -> Result<MarketQuote, ProviderError> {
    let url = format!("{ENDPOINT_BASE}/{}.json", base.to_ascii_lowercase());
    let response = client
        .get(&url)
        .send()
        .map_err(|error| ProviderError::Transport(error.to_string()))?;

    let status = response.status().as_u16();
    let body = response
        .text()
        .map_err(|error| ProviderError::Transport(error.to_string()))?;
    let (unit_price, fetched_at) = parse_fx_body(status, &body, quote)?;

    Ok(MarketQuote::new("floatrates", unit_price, fetched_at))
}

pub fn parse_fx_body(
    status: u16,
    body: &str,
    quote: &str,
) -> Result<(Decimal, DateTime<Utc>), ProviderError> {
    if !(200..=299).contains(&status) {
        return Err(ProviderError::Http {
            status,
            message: format!("HTTP {status}"),
        });
    }

    let payload = serde_json::from_str::<HashMap<String, FloatRatesEntry>>(body)
        .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
    let entry = payload
        .get(&quote.to_ascii_lowercase())
        .ok_or_else(|| ProviderError::UnsupportedPair(quote.to_string()))?;

    let fetched_at = DateTime::parse_from_rfc2822(entry.date.trim())
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            ProviderError::InvalidResponse(format!("invalid timestamp for {quote}: {error}"))
        })?;

    let unit_price = parse_decimal_value(&entry.rate).ok_or_else(|| {
        ProviderError::InvalidResponse(format!("invalid numeric rate for {quote}: {}", entry.rate))
    })?;

    Ok((unit_price, fetched_at))
}

fn parse_decimal_value(value: &serde_json::Value) -> Option<Decimal> {
    match value {
        serde_json::Value::String(text) => text.parse::<Decimal>().ok(),
        serde_json::Value::Number(number) => number.to_string().parse::<Decimal>().ok(),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct FloatRatesEntry {
    rate: serde_json::Value,
    date: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floatrates_parse_success_body_extracts_rate_and_timestamp() {
        let body = r#"{
            "twd": {
                "code": "TWD",
                "alphaCode": "TWD",
                "rate": 31.776104567796,
                "date": "Fri, 6 Mar 2026 22:55:02 GMT"
            }
        }"#;

        let (rate, fetched_at) = parse_fx_body(200, body, "TWD").expect("must parse");
        assert_eq!(rate.to_string(), "31.776104567796");
        assert_eq!(fetched_at.to_rfc3339(), "2026-03-06T22:55:02+00:00");
    }

    #[test]
    fn floatrates_parse_rejects_missing_rate() {
        let body = r#"{"jpy":{"rate":157.79,"date":"Fri, 6 Mar 2026 22:55:02 GMT"}}"#;
        let err = parse_fx_body(200, body, "TWD").expect_err("must fail");
        assert_eq!(err, ProviderError::UnsupportedPair("TWD".to_string()));
    }

    #[test]
    fn floatrates_parse_rejects_invalid_timestamp() {
        let body = r#"{"twd":{"rate":31.7,"date":"not-a-date"}}"#;
        let err = parse_fx_body(200, body, "TWD").expect_err("must fail");
        assert!(matches!(err, ProviderError::InvalidResponse(_)));
    }
}
