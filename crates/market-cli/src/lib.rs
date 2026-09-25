use std::collections::HashSet;

use workflow_common::parse_ordered_list_with;

pub mod cache;
pub mod config;
pub mod error;
pub mod expression;
pub mod icons;
pub mod model;
pub mod providers;
pub mod service;

use crate::model::{
    ValidationError, is_fiat_currency, normalize_crypto_symbol, normalize_fx_symbol,
};

const FAVORITE_TOKEN_EXPECTED_FORMAT: &str =
    "2-10 uppercase alphanumeric symbol or 3-letter FX pair BASE/QUOTE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FavoriteTarget {
    Symbol { symbol: String, quote: String },
    FxPair { base: String, quote: String },
}

impl FavoriteTarget {
    pub fn base(&self) -> &str {
        match self {
            Self::Symbol { symbol, .. } => symbol,
            Self::FxPair { base, .. } => base,
        }
    }

    pub fn quote(&self) -> &str {
        match self {
            Self::Symbol { quote, .. } | Self::FxPair { quote, .. } => quote,
        }
    }

    pub fn display_token(&self) -> String {
        match self {
            Self::Symbol { symbol, .. } => symbol.clone(),
            Self::FxPair { base, quote } => format!("{base}/{quote}"),
        }
    }

    fn dedup_key(&self) -> String {
        format!("{}/{}", self.base(), self.quote())
    }
}

pub fn icon_asset_filename(symbol: &str) -> Result<String, ValidationError> {
    let normalized = normalize_crypto_symbol(symbol, "icon_symbol")?;
    Ok(format!("{}.png", normalized.to_ascii_lowercase()))
}

const DEFAULT_FAVORITE_SYMBOLS: [&str; 3] = ["BTC", "ETH", "JPY"];

pub fn parse_favorites_list(
    raw: Option<&str>,
    default_fiat: &str,
) -> Result<Vec<FavoriteTarget>, ValidationError> {
    let default_fiat = normalize_fx_symbol(default_fiat, "default_fiat")?;
    let fallback = default_favorites(&default_fiat);

    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(fallback);
    };
    let expanded = value.replace("\\n", "\n");

    let mut seen = HashSet::new();
    let parsed = parse_ordered_list_with(&expanded, |token| {
        let favorite = parse_favorite_token(token, &default_fiat)?;
        if !seen.insert(favorite.dedup_key()) {
            return Ok(None);
        }

        Ok(Some(favorite))
    })?;

    if parsed.is_empty() {
        return Ok(fallback);
    }

    Ok(parsed)
}

fn parse_favorite_token(raw: &str, default_fiat: &str) -> Result<FavoriteTarget, ValidationError> {
    let token = raw.trim();
    if token.contains('/') {
        return parse_explicit_fx_pair(token);
    }

    let symbol =
        normalize_crypto_symbol(token, "favorite").map_err(|_| invalid_favorite_token(token))?;
    Ok(FavoriteTarget::Symbol {
        symbol,
        quote: default_fiat.to_string(),
    })
}

fn parse_explicit_fx_pair(raw: &str) -> Result<FavoriteTarget, ValidationError> {
    let mut parts = raw.split('/');
    let Some(base_raw) = parts.next() else {
        return Err(invalid_favorite_token(raw));
    };
    let Some(quote_raw) = parts.next() else {
        return Err(invalid_favorite_token(raw));
    };

    if parts.next().is_some() {
        return Err(invalid_favorite_token(raw));
    }

    let base = normalize_fx_symbol(base_raw.trim(), "favorite")
        .map_err(|_| invalid_favorite_token(raw))?;
    let quote = normalize_fx_symbol(quote_raw.trim(), "favorite")
        .map_err(|_| invalid_favorite_token(raw))?;

    Ok(FavoriteTarget::FxPair { base, quote })
}

fn invalid_favorite_token(raw: &str) -> ValidationError {
    ValidationError::InvalidSymbol {
        field: "favorite",
        value: raw.to_string(),
        expected: FAVORITE_TOKEN_EXPECTED_FORMAT,
    }
}

/// Favorites derived from an external preference projection watchlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchlistFavorites {
    pub favorites: Vec<FavoriteTarget>,
    /// Entries market-cli rejected (unsupported symbol, or a fiat entry
    /// without a valid quote currency). Values are intentionally not kept.
    pub skipped: usize,
}

/// Map an ordered projection watchlist to favorites.
///
/// - An ISO 4217 fiat entry becomes the explicit pair `SYM/<quote_currency>`;
///   a fiat equal to `quote_currency` is omitted.
/// - Any other entry that passes crypto symbol validation stays a bare symbol
///   quoted in `default_fiat` (the workflow's `MARKET_DEFAULT_FIAT`).
/// - Rejected entries are counted in `skipped`; duplicates are dropped.
pub fn favorites_from_watchlist(
    watchlist: &[String],
    quote_currency: &str,
    default_fiat: &str,
) -> Result<WatchlistFavorites, ValidationError> {
    let default_fiat = normalize_fx_symbol(default_fiat, "default_fiat")?;
    let quote_currency = normalize_fx_symbol(quote_currency, "quote_currency")
        .ok()
        .filter(|code| is_fiat_currency(code));

    let mut seen = HashSet::new();
    let mut favorites = Vec::new();
    let mut skipped = 0;

    for entry in watchlist {
        let favorite = if is_fiat_currency(entry) {
            let base = normalize_fx_symbol(entry, "favorite")?;
            let Some(quote) = quote_currency.as_ref() else {
                skipped += 1;
                continue;
            };
            if &base == quote {
                continue;
            }
            FavoriteTarget::FxPair {
                base,
                quote: quote.clone(),
            }
        } else {
            let Ok(symbol) = normalize_crypto_symbol(entry, "favorite") else {
                skipped += 1;
                continue;
            };
            FavoriteTarget::Symbol {
                symbol,
                quote: default_fiat.clone(),
            }
        };

        if seen.insert(favorite.dedup_key()) {
            favorites.push(favorite);
        }
    }

    Ok(WatchlistFavorites { favorites, skipped })
}

fn default_favorites(default_fiat: &str) -> Vec<FavoriteTarget> {
    let mut seen = HashSet::new();
    let mut defaults = Vec::new();

    for symbol in [
        DEFAULT_FAVORITE_SYMBOLS[0].to_string(),
        DEFAULT_FAVORITE_SYMBOLS[1].to_string(),
        default_fiat.to_string(),
        DEFAULT_FAVORITE_SYMBOLS[2].to_string(),
    ] {
        if seen.insert(symbol.clone()) {
            defaults.push(FavoriteTarget::Symbol {
                symbol,
                quote: default_fiat.to_string(),
            });
        }
    }

    defaults
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_asset_filename_normalizes_symbol_to_lowercase_png() {
        assert_eq!(icon_asset_filename(" btc ").as_deref(), Ok("btc.png"));
    }

    #[test]
    fn icon_asset_filename_rejects_invalid_symbol() {
        let err = icon_asset_filename("eth!");
        assert!(err.is_err());
    }

    #[test]
    fn parse_favorites_list_supports_symbols_and_explicit_fx_pairs() {
        let favorites = parse_favorites_list(Some("btc, jpy/twd, usd/jpy"), "USD")
            .expect("favorites should parse");

        assert_eq!(
            favorites,
            vec![
                FavoriteTarget::Symbol {
                    symbol: "BTC".to_string(),
                    quote: "USD".to_string(),
                },
                FavoriteTarget::FxPair {
                    base: "JPY".to_string(),
                    quote: "TWD".to_string(),
                },
                FavoriteTarget::FxPair {
                    base: "USD".to_string(),
                    quote: "JPY".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_favorites_list_dedups_by_effective_base_quote_pair() {
        let favorites = parse_favorites_list(Some("usd,USD/TWD,jpy/twd,jpy"), "TWD")
            .expect("favorites should parse");

        assert_eq!(
            favorites,
            vec![
                FavoriteTarget::Symbol {
                    symbol: "USD".to_string(),
                    quote: "TWD".to_string(),
                },
                FavoriteTarget::FxPair {
                    base: "JPY".to_string(),
                    quote: "TWD".to_string(),
                },
            ]
        );
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn symbol(symbol: &str, quote: &str) -> FavoriteTarget {
        FavoriteTarget::Symbol {
            symbol: symbol.to_string(),
            quote: quote.to_string(),
        }
    }

    fn pair(base: &str, quote: &str) -> FavoriteTarget {
        FavoriteTarget::FxPair {
            base: base.to_string(),
            quote: quote.to_string(),
        }
    }

    #[test]
    fn watchlist_maps_fiat_to_quote_pairs_and_crypto_to_default_fiat() {
        let mapped = favorites_from_watchlist(
            &strings(&["USD", "JPY", "BTC", "ETH", "ADA", "DOT"]),
            "TWD",
            "USD",
        )
        .expect("watchlist should map");

        assert_eq!(
            mapped.favorites,
            vec![
                pair("USD", "TWD"),
                pair("JPY", "TWD"),
                symbol("BTC", "USD"),
                symbol("ETH", "USD"),
                symbol("ADA", "USD"),
                symbol("DOT", "USD"),
            ]
        );
        assert_eq!(mapped.skipped, 0);
    }

    #[test]
    fn watchlist_crypto_uses_workflow_default_fiat_not_projection_quote() {
        let mapped = favorites_from_watchlist(&strings(&["btc", "eur"]), "TWD", "EUR")
            .expect("watchlist should map");

        assert_eq!(
            mapped.favorites,
            vec![symbol("BTC", "EUR"), pair("EUR", "TWD")]
        );
    }

    #[test]
    fn watchlist_skips_fiat_equal_to_quote_and_dedups_case_variants() {
        let mapped =
            favorites_from_watchlist(&strings(&["TWD", "usd", "USD", "Btc", "BTC"]), "twd", "USD")
                .expect("watchlist should map");

        assert_eq!(
            mapped.favorites,
            vec![pair("USD", "TWD"), symbol("BTC", "USD")]
        );
        assert_eq!(mapped.skipped, 0);
    }

    #[test]
    fn watchlist_drops_rejected_entries_with_count() {
        let mapped = favorites_from_watchlist(
            &strings(&["BTC-USD", "X", "東京", "ABCDEFGHIJKLMNOP", "ETH"]),
            "TWD",
            "USD",
        )
        .expect("watchlist should map");

        assert_eq!(mapped.favorites, vec![symbol("ETH", "USD")]);
        assert_eq!(mapped.skipped, 4);
    }

    #[test]
    fn watchlist_fiat_entries_are_skipped_without_valid_quote_currency() {
        for quote in ["", "BTC", "TOOLONG"] {
            let mapped = favorites_from_watchlist(&strings(&["JPY", "BTC"]), quote, "USD")
                .expect("watchlist should map");
            assert_eq!(mapped.favorites, vec![symbol("BTC", "USD")]);
            assert_eq!(mapped.skipped, 1);
        }
    }

    #[test]
    fn watchlist_empty_input_yields_empty_favorites() {
        let mapped = favorites_from_watchlist(&[], "TWD", "USD").expect("empty maps");
        assert!(mapped.favorites.is_empty());
        assert_eq!(mapped.skipped, 0);
    }

    #[test]
    fn parse_favorites_list_rejects_invalid_fx_pair_format() {
        let err = parse_favorites_list(Some("usd//twd"), "USD").expect_err("must fail");

        assert_eq!(
            err,
            ValidationError::InvalidSymbol {
                field: "favorite",
                value: "usd//twd".to_string(),
                expected: FAVORITE_TOKEN_EXPECTED_FORMAT,
            }
        );
    }
}
