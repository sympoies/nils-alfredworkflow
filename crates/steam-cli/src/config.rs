use std::collections::{HashMap, HashSet};

use thiserror::Error;
use workflow_common::parse_ordered_list_with;

const REGION_ENV: &str = "STEAM_REGION";
const REGION_OPTIONS_ENV: &str = "STEAM_REGION_OPTIONS";
const MAX_RESULTS_ENV: &str = "STEAM_MAX_RESULTS";
const LANGUAGE_ENV: &str = "STEAM_LANGUAGE";

const MIN_RESULTS: i32 = 1;
const MAX_RESULTS: i32 = 50;
pub const DEFAULT_MAX_RESULTS: u8 = 10;
pub const DEFAULT_REGION: &str = "us";
pub const DEFAULT_LANGUAGE: &str = "english";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub region: String,
    pub region_options: Vec<String>,
    pub max_results: u8,
    pub language: String,
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
        let max_results = parse_max_results(env_map.get(MAX_RESULTS_ENV).map(String::as_str))?;
        let language = parse_language(env_map.get(LANGUAGE_ENV).map(String::as_str))?;

        Ok(Self {
            region,
            region_options,
            max_results,
            language,
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("invalid STEAM_REGION: {0} (expected 2-letter country code)")]
    InvalidRegion(String),
    #[error(
        "invalid STEAM_REGION_OPTIONS token: {0} (expected comma/newline list of 2-letter country codes)"
    )]
    InvalidRegionOptions(String),
    #[error("invalid STEAM_MAX_RESULTS: {0}")]
    InvalidMaxResults(String),
    #[error("invalid STEAM_LANGUAGE: {0} (expected lowercase letters/hyphen, length 2..24)")]
    InvalidLanguage(String),
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
        assert_eq!(config.max_results, DEFAULT_MAX_RESULTS);
        assert_eq!(config.language, DEFAULT_LANGUAGE);
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
    fn config_normalizes_language_to_lowercase() {
        let config = RuntimeConfig::from_pairs(vec![(LANGUAGE_ENV, " ENGLISH ")])
            .expect("language should parse");

        assert_eq!(config.language, "english");
    }

    #[test]
    fn config_rejects_invalid_language_format() {
        let err = RuntimeConfig::from_pairs(vec![(LANGUAGE_ENV, "en_US")])
            .expect_err("invalid language should fail");

        assert_eq!(err, ConfigError::InvalidLanguage("en_US".to_string()));
    }
}
