use std::collections::HashMap;
use std::path::PathBuf;

use crate::icon_asset_filename;
use crate::model::{MarketKind, ValidationError};

pub const FX_TTL_SECS: u64 = 24 * 60 * 60;
pub const CRYPTO_TTL_SECS: u64 = 5 * 60;
pub const ICON_SOURCE_PACKAGE: &str = "cryptocurrency-icons";
pub const ICON_SOURCE_VERSION: &str = "0.18.1";
pub const ICON_SOURCE_CDN_BASE_URL: &str =
    "https://cdn.jsdelivr.net/npm/cryptocurrency-icons@0.18.1";
pub const ICON_PNG_VARIANT_DIR: &str = "32/color";
pub const ICON_GENERIC_BASENAME: &str = "generic.png";

pub const MARKET_CACHE_DIR_ENV: &str = "MARKET_CACHE_DIR";
pub const MARKET_FX_CACHE_TTL_ENV: &str = "MARKET_FX_CACHE_TTL";
pub const MARKET_CRYPTO_CACHE_TTL_ENV: &str = "MARKET_CRYPTO_CACHE_TTL";
const ALFRED_WORKFLOW_CACHE_ENV: &str = "ALFRED_WORKFLOW_CACHE";
const ALFRED_WORKFLOW_DATA_ENV: &str = "ALFRED_WORKFLOW_DATA";
const HOME_ENV: &str = "HOME";

pub const PROVIDER_TIMEOUT_SECS: u64 = 6;
pub const PROVIDER_RETRY_MAX_ATTEMPTS: usize = 3;
pub const PROVIDER_RETRY_BASE_BACKOFF_MS: u64 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub cache_dir: PathBuf,
    pub fx_cache_ttl_secs: u64,
    pub crypto_cache_ttl_secs: u64,
}

impl RuntimeConfig {
    pub fn from_env() -> Self {
        Self::from_pairs(std::env::vars())
    }

    pub(crate) fn from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let map: HashMap<String, String> = pairs
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        Self {
            cache_dir: resolve_cache_dir(&map),
            fx_cache_ttl_secs: resolve_cache_ttl_secs(&map, MARKET_FX_CACHE_TTL_ENV, FX_TTL_SECS),
            crypto_cache_ttl_secs: resolve_cache_ttl_secs(
                &map,
                MARKET_CRYPTO_CACHE_TTL_ENV,
                CRYPTO_TTL_SECS,
            ),
        }
    }

    pub fn cache_ttl_secs_for_kind(&self, kind: MarketKind) -> u64 {
        match kind {
            MarketKind::Fx => self.fx_cache_ttl_secs,
            MarketKind::Crypto => self.crypto_cache_ttl_secs,
        }
    }

    pub fn market_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("market-cli")
    }

    pub fn icon_cache_dir(&self) -> PathBuf {
        self.market_cache_dir()
            .join("icons")
            .join(ICON_SOURCE_PACKAGE)
            .join(ICON_SOURCE_VERSION)
            .join(ICON_PNG_VARIANT_DIR)
    }

    pub fn icon_cache_path(&self, asset_filename: &str) -> PathBuf {
        self.icon_cache_dir().join(asset_filename)
    }

    pub fn icon_cache_path_for_symbol(&self, symbol: &str) -> Result<PathBuf, ValidationError> {
        Ok(self.icon_cache_path(&icon_asset_filename(symbol)?))
    }

    pub fn generic_icon_cache_path(&self) -> PathBuf {
        self.icon_cache_path(ICON_GENERIC_BASENAME)
    }
}

fn resolve_cache_dir(env_map: &HashMap<String, String>) -> PathBuf {
    let home = env_map.get(HOME_ENV).map(String::as_str);
    env_map
        .get(MARKET_CACHE_DIR_ENV)
        .or_else(|| env_map.get(ALFRED_WORKFLOW_CACHE_ENV))
        .or_else(|| env_map.get(ALFRED_WORKFLOW_DATA_ENV))
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| expand_home_path(value, home))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("nils-market-cli"))
}

fn expand_home_path(raw: &str, home: Option<&str>) -> String {
    let trimmed = raw.trim();
    let Some(home) = home.map(str::trim).filter(|value| !value.is_empty()) else {
        return trimmed.to_string();
    };

    let home = home.trim_end_matches('/');
    let mut expanded = trimmed.replace("$HOME", home);

    if expanded == "~" {
        expanded = home.to_string();
    } else if let Some(rest) = expanded.strip_prefix("~/") {
        expanded = format!("{home}/{rest}");
    }

    expanded
}

fn resolve_cache_ttl_secs(
    env_map: &HashMap<String, String>,
    env_name: &str,
    default_secs: u64,
) -> u64 {
    env_map
        .get(env_name)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(parse_duration_to_secs)
        .unwrap_or(default_secs)
}

fn parse_duration_to_secs(raw: &str) -> Option<u64> {
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    if normalized.chars().all(|ch| ch.is_ascii_digit()) {
        return normalized.parse::<u64>().ok().filter(|value| *value > 0);
    }

    if normalized.len() < 2 {
        return None;
    }

    let (digits, unit) = normalized.split_at(normalized.len() - 1);
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let amount = digits.parse::<u64>().ok().filter(|value| *value > 0)?;
    let multiplier = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        "d" => 24 * 60 * 60,
        _ => return None,
    };

    amount.checked_mul(multiplier)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub base_backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: PROVIDER_RETRY_MAX_ATTEMPTS,
            base_backoff_ms: PROVIDER_RETRY_BASE_BACKOFF_MS,
        }
    }
}

impl RetryPolicy {
    pub fn backoff_for_attempt(self, attempt: usize) -> u64 {
        if attempt <= 1 {
            return 0;
        }

        let shift = (attempt - 2).min(8);
        self.base_backoff_ms.saturating_mul(1_u64 << shift)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_use_temp_market_cache_dir() {
        let config = RuntimeConfig::from_pairs(Vec::<(String, String)>::new());
        assert!(config.cache_dir.ends_with("nils-market-cli"));
        assert_eq!(config.fx_cache_ttl_secs, FX_TTL_SECS);
        assert_eq!(config.crypto_cache_ttl_secs, CRYPTO_TTL_SECS);
        assert_eq!(config.cache_ttl_secs_for_kind(MarketKind::Fx), FX_TTL_SECS);
        assert_eq!(
            config.cache_ttl_secs_for_kind(MarketKind::Crypto),
            CRYPTO_TTL_SECS
        );
    }

    #[test]
    fn config_prefers_market_cache_dir_over_alfred_paths() {
        let config = RuntimeConfig::from_pairs(vec![
            (ALFRED_WORKFLOW_DATA_ENV, "/tmp/alfred-data"),
            (ALFRED_WORKFLOW_CACHE_ENV, "/tmp/alfred-cache"),
            (MARKET_CACHE_DIR_ENV, "/tmp/market-cache"),
        ]);

        assert_eq!(config.cache_dir, PathBuf::from("/tmp/market-cache"));
    }

    #[test]
    fn config_expands_home_prefix_for_market_cache_dir() {
        let config = RuntimeConfig::from_pairs(vec![
            (HOME_ENV, "/tmp/home"),
            (MARKET_CACHE_DIR_ENV, "~/.cache/market"),
        ]);

        assert_eq!(config.cache_dir, PathBuf::from("/tmp/home/.cache/market"));
    }

    #[test]
    fn config_icon_cache_dir_is_versioned() {
        let config = RuntimeConfig::from_pairs(vec![(MARKET_CACHE_DIR_ENV, "/tmp/market-cache")]);

        assert_eq!(
            config.icon_cache_dir(),
            PathBuf::from("/tmp/market-cache")
                .join("market-cli")
                .join("icons")
                .join(ICON_SOURCE_PACKAGE)
                .join(ICON_SOURCE_VERSION)
                .join(ICON_PNG_VARIANT_DIR)
        );
    }

    #[test]
    fn config_icon_cache_paths_are_deterministic() {
        let config = RuntimeConfig::from_pairs(vec![(MARKET_CACHE_DIR_ENV, "/tmp/market-cache")]);
        let expected_icon_dir = PathBuf::from("/tmp/market-cache")
            .join("market-cli")
            .join("icons")
            .join(ICON_SOURCE_PACKAGE)
            .join(ICON_SOURCE_VERSION)
            .join(ICON_PNG_VARIANT_DIR);

        assert_eq!(
            config.icon_cache_path_for_symbol(" btc "),
            Ok(expected_icon_dir.join("btc.png"))
        );
        assert_eq!(
            config.generic_icon_cache_path(),
            expected_icon_dir.join(ICON_GENERIC_BASENAME)
        );
    }

    #[test]
    fn config_uses_alfred_workflow_cache_when_market_cache_dir_missing() {
        let config =
            RuntimeConfig::from_pairs(vec![(ALFRED_WORKFLOW_CACHE_ENV, "/tmp/alfred-cache")]);
        assert_eq!(config.cache_dir, PathBuf::from("/tmp/alfred-cache"));
    }

    #[test]
    fn config_uses_alfred_workflow_data_when_cache_env_missing() {
        let config =
            RuntimeConfig::from_pairs(vec![(ALFRED_WORKFLOW_DATA_ENV, "/tmp/alfred-data")]);
        assert_eq!(config.cache_dir, PathBuf::from("/tmp/alfred-data"));
    }

    #[test]
    fn duration_parser_supports_s_m_h_d_suffixes() {
        assert_eq!(parse_duration_to_secs("1s"), Some(1));
        assert_eq!(parse_duration_to_secs("1m"), Some(60));
        assert_eq!(parse_duration_to_secs("1h"), Some(3600));
        assert_eq!(parse_duration_to_secs("1d"), Some(86_400));
    }

    #[test]
    fn config_supports_fx_cache_ttl_duration_override() {
        let config = RuntimeConfig::from_pairs(vec![(MARKET_FX_CACHE_TTL_ENV, "2d")]);
        assert_eq!(config.fx_cache_ttl_secs, 172_800);
        assert_eq!(config.crypto_cache_ttl_secs, CRYPTO_TTL_SECS);
    }

    #[test]
    fn config_supports_crypto_cache_ttl_duration_override() {
        let config = RuntimeConfig::from_pairs(vec![
            (MARKET_FX_CACHE_TTL_ENV, "15m"),
            (MARKET_CRYPTO_CACHE_TTL_ENV, "1h"),
        ]);
        assert_eq!(config.fx_cache_ttl_secs, 900);
        assert_eq!(config.crypto_cache_ttl_secs, 3600);
    }

    #[test]
    fn config_supports_seconds_and_uppercase_duration_suffixes() {
        let config = RuntimeConfig::from_pairs(vec![
            (MARKET_FX_CACHE_TTL_ENV, "30"),
            (MARKET_CRYPTO_CACHE_TTL_ENV, "2H"),
        ]);
        assert_eq!(config.fx_cache_ttl_secs, 30);
        assert_eq!(config.crypto_cache_ttl_secs, 7200);
    }

    #[test]
    fn config_falls_back_when_cache_ttl_override_invalid() {
        let invalid = RuntimeConfig::from_pairs(vec![
            (MARKET_FX_CACHE_TTL_ENV, "abc"),
            (MARKET_CRYPTO_CACHE_TTL_ENV, "1x"),
        ]);
        assert_eq!(invalid.fx_cache_ttl_secs, FX_TTL_SECS);
        assert_eq!(invalid.crypto_cache_ttl_secs, CRYPTO_TTL_SECS);

        let zero = RuntimeConfig::from_pairs(vec![
            (MARKET_FX_CACHE_TTL_ENV, "0s"),
            (MARKET_CRYPTO_CACHE_TTL_ENV, "0"),
        ]);
        assert_eq!(zero.fx_cache_ttl_secs, FX_TTL_SECS);
        assert_eq!(zero.crypto_cache_ttl_secs, CRYPTO_TTL_SECS);
    }

    #[test]
    fn config_retry_policy_backoff_is_deterministic() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.backoff_for_attempt(1), 0);
        assert_eq!(policy.backoff_for_attempt(2), 200);
        assert_eq!(policy.backoff_for_attempt(3), 400);
    }
}
