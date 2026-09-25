use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketKind {
    Fx,
    Crypto,
}

impl MarketKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fx => "fx",
            Self::Crypto => "crypto",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    Live,
    CacheFresh,
    CacheStaleFallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub status: CacheStatus,
    pub key: String,
    pub ttl_secs: u64,
    pub age_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOutput {
    pub kind: MarketKind,
    pub base: String,
    pub quote: String,
    pub amount: String,
    pub unit_price: String,
    pub converted: String,
    pub provider: String,
    pub fetched_at: String,
    pub cache: CacheMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketRequest {
    pub kind: MarketKind,
    pub base: String,
    pub quote: String,
    pub amount: Decimal,
}

impl MarketRequest {
    pub fn new(
        kind: MarketKind,
        base: &str,
        quote: &str,
        amount: &str,
    ) -> Result<Self, ValidationError> {
        let normalized_base = match kind {
            MarketKind::Fx => normalize_fx_symbol(base, "base")?,
            MarketKind::Crypto => normalize_crypto_symbol(base, "base")?,
        };
        let normalized_quote = match kind {
            MarketKind::Fx => normalize_fx_symbol(quote, "quote")?,
            MarketKind::Crypto => normalize_crypto_symbol(quote, "quote")?,
        };

        let parsed_amount = parse_amount(amount)?;

        Ok(Self {
            kind,
            base: normalized_base,
            quote: normalized_quote,
            amount: parsed_amount,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketQuote {
    pub provider: String,
    pub unit_price: Decimal,
    pub fetched_at: DateTime<Utc>,
}

impl MarketQuote {
    pub fn new(
        provider: impl Into<String>,
        unit_price: Decimal,
        fetched_at: DateTime<Utc>,
    ) -> Self {
        Self {
            provider: provider.into(),
            unit_price,
            fetched_at,
        }
    }
}

pub fn build_output(
    request: &MarketRequest,
    quote: &MarketQuote,
    cache: CacheMetadata,
) -> MarketOutput {
    let converted = (request.amount * quote.unit_price).round_dp(8);

    MarketOutput {
        kind: request.kind,
        base: request.base.clone(),
        quote: request.quote.clone(),
        amount: decimal_to_string(&request.amount),
        unit_price: decimal_to_string(&quote.unit_price),
        converted: decimal_to_string(&converted),
        provider: quote.provider.clone(),
        fetched_at: quote.fetched_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        cache,
    }
}

pub fn decimal_to_string(value: &Decimal) -> String {
    value.normalize().to_string()
}

pub fn normalize_fx_symbol(raw: &str, field: &'static str) -> Result<String, ValidationError> {
    let value = raw.trim().to_ascii_uppercase();
    if value.len() != 3 || !value.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(ValidationError::InvalidSymbol {
            field,
            value: raw.to_string(),
            expected: "3-letter ISO currency code",
        });
    }
    Ok(value)
}

/// Active ISO 4217 national/regional currency codes treated as fiat.
///
/// Precious-metal, fund, and testing codes (`XAU`, `XDR`, `XTS`, ...) are not
/// fiat for favorites classification. A code listed here wins over a crypto
/// asset that happens to share the same ticker.
const ISO_4217_FIAT_CODES: [&str; 158] = [
    "AED", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD", "BDT",
    "BGN", "BHD", "BIF", "BMD", "BND", "BOB", "BRL", "BSD", "BTN", "BWP", "BYN", "BZD", "CAD",
    "CDF", "CHF", "CLP", "CNY", "COP", "CRC", "CUP", "CVE", "CZK", "DJF", "DKK", "DOP", "DZD",
    "EGP", "ERN", "ETB", "EUR", "FJD", "FKP", "GBP", "GEL", "GHS", "GIP", "GMD", "GNF", "GTQ",
    "GYD", "HKD", "HNL", "HTG", "HUF", "IDR", "ILS", "INR", "IQD", "IRR", "ISK", "JMD", "JOD",
    "JPY", "KES", "KGS", "KHR", "KMF", "KPW", "KRW", "KWD", "KYD", "KZT", "LAK", "LBP", "LKR",
    "LRD", "LSL", "LYD", "MAD", "MDL", "MGA", "MKD", "MMK", "MNT", "MOP", "MRU", "MUR", "MVR",
    "MWK", "MXN", "MYR", "MZN", "NAD", "NGN", "NIO", "NOK", "NPR", "NZD", "OMR", "PAB", "PEN",
    "PGK", "PHP", "PKR", "PLN", "PYG", "QAR", "RON", "RSD", "RUB", "RWF", "SAR", "SBD", "SCR",
    "SDG", "SEK", "SGD", "SHP", "SLE", "SLL", "SOS", "SRD", "SSP", "STN", "SVC", "SYP", "SZL",
    "THB", "TJS", "TMT", "TND", "TOP", "TRY", "TTD", "TWD", "TZS", "UAH", "UGX", "USD", "UYU",
    "UZS", "VED", "VES", "VND", "VUV", "WST", "XAF", "XCD", "XCG", "XOF", "XPF", "YER", "ZAR",
    "ZMW", "ZWG",
];

/// Whether `raw` is an active ISO 4217 fiat currency code (case-insensitive).
pub fn is_fiat_currency(raw: &str) -> bool {
    normalize_fx_symbol(raw, "currency")
        .map(|code| ISO_4217_FIAT_CODES.contains(&code.as_str()))
        .unwrap_or(false)
}

pub fn normalize_crypto_symbol(raw: &str, field: &'static str) -> Result<String, ValidationError> {
    let value = raw.trim().to_ascii_uppercase();
    if value.len() < 2
        || value.len() > 10
        || !value
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
    {
        return Err(ValidationError::InvalidSymbol {
            field,
            value: raw.to_string(),
            expected: "2-10 uppercase alphanumeric symbol",
        });
    }
    Ok(value)
}

pub fn parse_amount(raw: &str) -> Result<Decimal, ValidationError> {
    let value = raw.trim();
    let parsed = value
        .parse::<Decimal>()
        .map_err(|_| ValidationError::InvalidAmount(raw.to_string()))?;
    if parsed <= Decimal::ZERO {
        return Err(ValidationError::AmountMustBePositive(raw.to_string()));
    }
    Ok(parsed.normalize())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("invalid {field} symbol: {value} (expected {expected})")]
    InvalidSymbol {
        field: &'static str,
        value: String,
        expected: &'static str,
    },
    #[error("invalid amount: {0}")]
    InvalidAmount(String),
    #[error("amount must be positive: {0}")]
    AmountMustBePositive(String),
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn conversion_build_output_is_deterministic() {
        let request = MarketRequest::new(MarketKind::Fx, "usd", "twd", "100").expect("request");
        let quote = MarketQuote::new(
            "frankfurter",
            Decimal::new(320126, 4),
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

        assert_eq!(output.amount, "100");
        assert_eq!(output.unit_price, "32.0126");
        assert_eq!(output.converted, "3201.26");
        assert_eq!(output.kind, MarketKind::Fx);
    }

    #[test]
    fn numeric_parse_amount_rejects_invalid_text() {
        let err = parse_amount("not-a-number").expect_err("must fail");
        assert_eq!(
            err,
            ValidationError::InvalidAmount("not-a-number".to_string())
        );
    }

    #[test]
    fn numeric_parse_amount_requires_positive_value() {
        let err = parse_amount("0").expect_err("must fail");
        assert_eq!(err, ValidationError::AmountMustBePositive("0".to_string()));
    }

    #[test]
    fn model_normalize_fx_symbol_requires_three_letters() {
        let err = normalize_fx_symbol("USDT", "base").expect_err("must fail");
        assert_eq!(
            err,
            ValidationError::InvalidSymbol {
                field: "base",
                value: "USDT".to_string(),
                expected: "3-letter ISO currency code",
            }
        );
    }

    #[test]
    fn model_is_fiat_currency_uses_iso_catalog_not_symbol_shape() {
        for code in ["USD", "jpy", " TWD ", "EUR", "CHF"] {
            assert!(is_fiat_currency(code), "{code} should be fiat");
        }
        for symbol in ["BTC", "ETH", "ADA", "DOT", "SOL", "XAU", "USDT", "", "US"] {
            assert!(!is_fiat_currency(symbol), "{symbol} should not be fiat");
        }
        let mut sorted = ISO_4217_FIAT_CODES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ISO_4217_FIAT_CODES.len());
    }

    #[test]
    fn model_normalize_crypto_symbol_accepts_letters_and_digits() {
        let parsed = normalize_crypto_symbol(" usdt ", "base").expect("should parse");
        assert_eq!(parsed, "USDT");
    }
}
