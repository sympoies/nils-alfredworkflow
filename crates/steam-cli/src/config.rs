use std::collections::{HashMap, HashSet};

use thiserror::Error;
use workflow_common::parse_ordered_list_with;

const REGION_ENV: &str = "STEAM_REGION";
const REGION_OPTIONS_ENV: &str = "STEAM_REGION_OPTIONS";
const SHOW_REGION_OPTIONS_ENV: &str = "STEAM_SHOW_REGION_OPTIONS";
const MAX_RESULTS_ENV: &str = "STEAM_MAX_RESULTS";
const LANGUAGE_ENV: &str = "STEAM_LANGUAGE";
const SEARCH_API_ENV: &str = "STEAM_SEARCH_API";

const MIN_RESULTS: i32 = 1;
const MAX_RESULTS: i32 = 50;
pub const DEFAULT_MAX_RESULTS: u8 = 10;
pub const DEFAULT_REGION: &str = "us";
pub const DEFAULT_LANGUAGE: &str = "";
pub const DEFAULT_SHOW_REGION_OPTIONS: bool = false;
pub const DEFAULT_SEARCH_API: SteamSearchApi = SteamSearchApi::SearchSuggestions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamSearchApi {
    SearchSuggestions,
    StoreSearch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub region: String,
    pub region_options: Vec<String>,
    pub show_region_options: bool,
    pub max_results: u8,
    pub language: String,
    pub search_api: SteamSearchApi,
}

impl RuntimeConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_pairs(std::env::vars())
    }

    pub(crate) fn from_pairs<I, K, V>(pairs: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let env_map: HashMap<String, String> = pairs
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();

        let region = parse_region(env_map.get(REGION_ENV).map(String::as_str))?;
        let region_options =
            parse_region_options(env_map.get(REGION_OPTIONS_ENV).map(String::as_str), &region)?;
        let show_region_options =
            parse_show_region_options(env_map.get(SHOW_REGION_OPTIONS_ENV).map(String::as_str))?;
        let max_results = parse_max_results(env_map.get(MAX_RESULTS_ENV).map(String::as_str))?;
        let language = parse_language(env_map.get(LANGUAGE_ENV).map(String::as_str))?;
        let search_api = parse_search_api(env_map.get(SEARCH_API_ENV).map(String::as_str))?;

        Ok(Self {
            region,
            region_options,
            show_region_options,
            max_results,
            language,
            search_api,
        })
    }
}

fn parse_region(raw: Option<&str>) -> Result<String, ConfigError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_REGION.to_string());
    };

    parse_region_code(value).ok_or_else(|| ConfigError::InvalidRegion(value.to_string()))
}

fn parse_region_options(
    raw: Option<&str>,
    default_region: &str,
) -> Result<Vec<String>, ConfigError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(vec![default_region.to_string()]);
    };

    let mut seen = HashSet::new();
    let options = parse_ordered_list_with(value, |token| {
        let normalized = parse_region_code(token)
            .ok_or_else(|| ConfigError::InvalidRegionOptions(token.to_string()))?;

        if !seen.insert(normalized.clone()) {
            return Ok(None);
        }

        Ok(Some(normalized))
    })?;

    if options.is_empty() {
        return Err(ConfigError::InvalidRegionOptions(value.to_string()));
    }

    Ok(options)
}

fn parse_region_code(raw: &str) -> Option<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    let is_valid = normalized.len() == 2
        && normalized
            .chars()
            .all(|ch| ch.is_ascii_alphabetic() && ch.is_ascii_lowercase());

    if !is_valid {
        return None;
    }

    Some(normalized)
}

fn parse_max_results(raw: Option<&str>) -> Result<u8, ConfigError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_MAX_RESULTS);
    };

    let parsed = value
        .parse::<i32>()
        .map_err(|_| ConfigError::InvalidMaxResults(value.to_string()))?;

    Ok(parsed.clamp(MIN_RESULTS, MAX_RESULTS) as u8)
}

fn parse_show_region_options(raw: Option<&str>) -> Result<bool, ConfigError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_SHOW_REGION_OPTIONS);
    };

    parse_bool(value).ok_or_else(|| ConfigError::InvalidShowRegionOptions(value.to_string()))
}

fn parse_language(raw: Option<&str>) -> Result<String, ConfigError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_LANGUAGE.to_string());
    };

    parse_language_code(value).ok_or_else(|| ConfigError::InvalidLanguage(value.to_string()))
}

fn parse_language_code(raw: &str) -> Option<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    let is_valid = (2..=24).contains(&normalized.len())
        && normalized
            .chars()
            .all(|ch| ch.is_ascii_alphabetic() || ch == '-');

    if !is_valid {
        return None;
    }

    Some(normalized)
}

fn parse_bool(raw: &str) -> Option<bool> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "t" | "yes" | "y" | "on" => Some(true),
        "0" | "false" | "f" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

fn parse_search_api(raw: Option<&str>) -> Result<SteamSearchApi, ConfigError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_SEARCH_API);
    };

    let normalized = value.to_ascii_lowercase();
    match normalized.as_str() {
        "search-suggestions" | "searchsuggestions" => Ok(SteamSearchApi::SearchSuggestions),
        "storesearch" | "store-search" => Ok(SteamSearchApi::StoreSearch),
        _ => Err(ConfigError::InvalidSearchApi(value.to_string())),
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("invalid STEAM_REGION: {0} (expected 2-letter country code)")]
    InvalidRegion(String),
    #[error(
        "invalid STEAM_REGION_OPTIONS token: {0} (expected comma/newline list of 2-letter country codes)"
    )]
    InvalidRegionOptions(String),
    #[error(
        "invalid STEAM_SHOW_REGION_OPTIONS: {0} (expected one of: true/false, yes/no, on/off, 1/0)"
    )]
    InvalidShowRegionOptions(String),
    #[error("invalid STEAM_MAX_RESULTS: {0}")]
    InvalidMaxResults(String),
    #[error("invalid STEAM_LANGUAGE: {0} (expected lowercase letters/hyphen, length 2..24)")]
    InvalidLanguage(String),
    #[error("invalid STEAM_SEARCH_API: {0} (expected one of: search-suggestions, storesearch)")]
    InvalidSearchApi(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_uses_defaults_when_optional_values_are_missing() {
        let config = RuntimeConfig::from_pairs(Vec::<(String, String)>::new())
            .expect("config should parse with defaults");

        assert_eq!(config.region, DEFAULT_REGION);
        assert_eq!(config.region_options, vec![DEFAULT_REGION.to_string()]);
        assert_eq!(config.show_region_options, DEFAULT_SHOW_REGION_OPTIONS);
        assert_eq!(config.max_results, DEFAULT_MAX_RESULTS);
        assert_eq!(config.language, DEFAULT_LANGUAGE);
        assert_eq!(config.search_api, DEFAULT_SEARCH_API);
    }

    #[test]
    fn config_normalizes_region_to_lowercase() {
        let config = RuntimeConfig::from_pairs(vec![(REGION_ENV, " US ")])
            .expect("region should parse and normalize");

        assert_eq!(config.region, "us");
    }

    #[test]
    fn config_parses_region_options_with_order_and_dedup() {
        let config = RuntimeConfig::from_pairs(vec![
            (REGION_ENV, "us"),
            (REGION_OPTIONS_ENV, "jp,us,JP,kr"),
        ])
        .expect("region options should parse");

        assert_eq!(config.region, "us");
        assert_eq!(config.region_options, vec!["jp", "us", "kr"]);
    }

    #[test]
    fn config_rejects_invalid_region_options_token() {
        let err = RuntimeConfig::from_pairs(vec![(REGION_OPTIONS_ENV, "us,usa")])
            .expect_err("invalid region option should fail");

        assert_eq!(err, ConfigError::InvalidRegionOptions("usa".to_string()));
    }

    #[test]
    fn config_rejects_delimiters_only_region_options_input() {
        let err = RuntimeConfig::from_pairs(vec![(REGION_OPTIONS_ENV, ", \n ,,")])
            .expect_err("delimiter-only options should fail");

        assert_eq!(
            err,
            ConfigError::InvalidRegionOptions(", \n ,,".to_string())
        );
    }

    #[test]
    fn config_rejects_invalid_region_format() {
        let err = RuntimeConfig::from_pairs(vec![(REGION_ENV, "USA")])
            .expect_err("invalid region should fail");

        assert_eq!(err, ConfigError::InvalidRegion("USA".to_string()));
    }

    #[test]
    fn config_clamps_max_results_into_supported_range() {
        let lower = RuntimeConfig::from_pairs(vec![(MAX_RESULTS_ENV, "-8")])
            .expect("lower bound config should parse");
        assert_eq!(lower.max_results, 1);

        let upper = RuntimeConfig::from_pairs(vec![(MAX_RESULTS_ENV, "999")])
            .expect("upper bound config should parse");
        assert_eq!(upper.max_results, 50);
    }

    #[test]
    fn config_rejects_non_numeric_max_results() {
        let err = RuntimeConfig::from_pairs(vec![(MAX_RESULTS_ENV, "ten")])
            .expect_err("invalid max results should fail");

        assert_eq!(err, ConfigError::InvalidMaxResults("ten".to_string()));
    }

    #[test]
    fn config_parses_show_region_options_bool_values() {
        for raw in ["1", "true", "YES", "On", "t"] {
            let config = RuntimeConfig::from_pairs(vec![(SHOW_REGION_OPTIONS_ENV, raw)])
                .expect("truthy show switch should parse");
            assert!(config.show_region_options, "{raw} should parse as true");
        }

        for raw in ["0", "false", "NO", "off", "F"] {
            let config = RuntimeConfig::from_pairs(vec![(SHOW_REGION_OPTIONS_ENV, raw)])
                .expect("falsey show switch should parse");
            assert!(!config.show_region_options, "{raw} should parse as false");
        }
    }

    #[test]
    fn config_rejects_invalid_show_region_options_value() {
        let err = RuntimeConfig::from_pairs(vec![(SHOW_REGION_OPTIONS_ENV, "maybe")])
            .expect_err("invalid bool should fail");

        assert_eq!(
            err,
            ConfigError::InvalidShowRegionOptions("maybe".to_string())
        );
    }

    #[test]
    fn config_normalizes_language_to_lowercase() {
        let config = RuntimeConfig::from_pairs(vec![(LANGUAGE_ENV, " ENGLISH ")])
            .expect("language should parse");

        assert_eq!(config.language, "english");
    }

    #[test]
    fn config_allows_empty_language_to_skip_l_param() {
        let config = RuntimeConfig::from_pairs(vec![(LANGUAGE_ENV, "   ")])
            .expect("empty language should parse");

        assert_eq!(config.language, "");
    }

    #[test]
    fn config_rejects_invalid_language_format() {
        let err = RuntimeConfig::from_pairs(vec![(LANGUAGE_ENV, "en_US")])
            .expect_err("invalid language should fail");

        assert_eq!(err, ConfigError::InvalidLanguage("en_US".to_string()));
    }

    #[test]
    fn config_defaults_search_api_to_new_endpoint() {
        let config = RuntimeConfig::from_pairs(vec![(REGION_ENV, "us")])
            .expect("search api should use default");
        assert_eq!(config.search_api, SteamSearchApi::SearchSuggestions);
    }

    #[test]
    fn config_parses_search_api_aliases() {
        let new_aliases = ["search-suggestions", "searchsuggestions"];
        for alias in new_aliases {
            let config = RuntimeConfig::from_pairs(vec![(SEARCH_API_ENV, alias)])
                .expect("new alias should parse");
            assert_eq!(config.search_api, SteamSearchApi::SearchSuggestions);
        }

        let legacy_aliases = ["storesearch", "store-search"];
        for alias in legacy_aliases {
            let config = RuntimeConfig::from_pairs(vec![(SEARCH_API_ENV, alias)])
                .expect("legacy alias should parse");
            assert_eq!(config.search_api, SteamSearchApi::StoreSearch);
        }
    }

    #[test]
    fn config_rejects_invalid_search_api() {
        let err = RuntimeConfig::from_pairs(vec![(SEARCH_API_ENV, "beta")])
            .expect_err("invalid search api should fail");
        assert_eq!(err, ConfigError::InvalidSearchApi("beta".to_string()));
    }
}
