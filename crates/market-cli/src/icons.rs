use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;

use crate::config::{
    ICON_GENERIC_BASENAME, ICON_PNG_VARIANT_DIR, ICON_SOURCE_CDN_BASE_URL, RuntimeConfig,
};
use crate::icon_asset_filename;

const ICON_FETCH_TIMEOUT_SECS: u64 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
enum FetchOutcome {
    Bytes(Vec<u8>),
    Missing,
}

pub fn resolve_icon_path(config: &RuntimeConfig, symbol: &str) -> Option<PathBuf> {
    let client = Client::builder()
        .timeout(Duration::from_secs(ICON_FETCH_TIMEOUT_SECS))
        .build()
        .ok()?;

    resolve_icon_path_with(config, symbol, &mut |url| fetch_icon_bytes(&client, url))
}

fn resolve_icon_path_with<F>(config: &RuntimeConfig, symbol: &str, fetch: &mut F) -> Option<PathBuf>
where
    F: FnMut(&str) -> io::Result<FetchOutcome>,
{
    let Ok(file_name) = icon_asset_filename(symbol) else {
        return resolve_generic_icon_path(config, fetch);
    };

    let symbol_path = config.icon_cache_dir().join(&file_name);
    if has_usable_cache_file(&symbol_path) {
        return Some(symbol_path);
    }

    let symbol_url = icon_url(&file_name);
    match fetch(&symbol_url) {
        Ok(FetchOutcome::Bytes(bytes)) if !bytes.is_empty() => {
            if write_atomic(&symbol_path, &bytes).is_ok() {
                return Some(symbol_path);
            }
        }
        Ok(FetchOutcome::Missing) | Ok(FetchOutcome::Bytes(_)) | Err(_) => {}
    }

    resolve_generic_icon_path(config, fetch)
}

fn resolve_generic_icon_path<F>(config: &RuntimeConfig, fetch: &mut F) -> Option<PathBuf>
where
    F: FnMut(&str) -> io::Result<FetchOutcome>,
{
    let generic_path = config.icon_cache_dir().join(ICON_GENERIC_BASENAME);
    if has_usable_cache_file(&generic_path) {
        return Some(generic_path);
    }

    let generic_url = icon_url(ICON_GENERIC_BASENAME);
    match fetch(&generic_url) {
        Ok(FetchOutcome::Bytes(bytes)) if !bytes.is_empty() => {
            write_atomic(&generic_path, &bytes).ok()?;
            Some(generic_path)
        }
        Ok(FetchOutcome::Missing) | Ok(FetchOutcome::Bytes(_)) | Err(_) => None,
    }
}

fn icon_url(file_name: &str) -> String {
    format!("{ICON_SOURCE_CDN_BASE_URL}/{ICON_PNG_VARIANT_DIR}/{file_name}")
}

fn has_usable_cache_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };

    if !metadata.is_file() {
        return false;
    }

    if metadata.len() > 0 {
        return true;
    }

    let _ = fs::remove_file(path);
    false
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "icon cache path must have a parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;

    let tmp_path = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&tmp_path, bytes)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn fetch_icon_bytes(client: &Client, url: &str) -> io::Result<FetchOutcome> {
    let response = client
        .get(url)
        .send()
        .map_err(|error| io::Error::other(error.to_string()))?;
    let status = response.status().as_u16();

    if status == 404 {
        return Ok(FetchOutcome::Missing);
    }

    if !(200..=299).contains(&status) {
        return Err(io::Error::other(format!(
            "icon fetch returned HTTP {status}"
        )));
    }

    let bytes = response
        .bytes()
        .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(FetchOutcome::Bytes(bytes.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        CRYPTO_TTL_SECS, FX_TTL_SECS, ICON_GENERIC_BASENAME, ICON_SOURCE_CDN_BASE_URL,
        RuntimeConfig,
    };

    fn config_in_tempdir() -> RuntimeConfig {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().to_path_buf();
        std::mem::forget(dir);

        RuntimeConfig {
            cache_dir,
            fx_cache_ttl_secs: FX_TTL_SECS,
            crypto_cache_ttl_secs: CRYPTO_TTL_SECS,
        }
    }

    #[test]
    fn icons_use_cached_symbol_without_fetching() {
        let config = config_in_tempdir();
        let path = config.icon_cache_dir().join("btc.png");
        write_atomic(&path, b"cached").expect("write cached icon");

        let mut fetch_calls = 0usize;
        let resolved = resolve_icon_path_with(&config, "BTC", &mut |_| {
            fetch_calls += 1;
            Ok(FetchOutcome::Missing)
        });

        assert_eq!(resolved, Some(path));
        assert_eq!(fetch_calls, 0);
    }

    #[test]
    fn icons_download_symbol_and_persist_it() {
        let config = config_in_tempdir();
        let mut fetched_urls = Vec::new();

        let resolved = resolve_icon_path_with(&config, "BTC", &mut |url| {
            fetched_urls.push(url.to_string());
            Ok(FetchOutcome::Bytes(b"btc-bytes".to_vec()))
        });

        let expected_path = config.icon_cache_dir().join("btc.png");
        assert_eq!(resolved, Some(expected_path.clone()));
        assert_eq!(fs::read(&expected_path).expect("read icon"), b"btc-bytes");
        assert_eq!(
            fetched_urls,
            vec![format!(
                "{ICON_SOURCE_CDN_BASE_URL}/{ICON_PNG_VARIANT_DIR}/btc.png"
            )]
        );
    }

    #[test]
    fn icons_fall_back_to_generic_for_missing_symbol() {
        let config = config_in_tempdir();
        let mut fetched_urls = Vec::new();

        let resolved = resolve_icon_path_with(&config, "DOGE", &mut |url| {
            fetched_urls.push(url.to_string());
            if url.ends_with("/doge.png") {
                Ok(FetchOutcome::Missing)
            } else if url.ends_with(&format!("/{ICON_GENERIC_BASENAME}")) {
                Ok(FetchOutcome::Bytes(b"generic-bytes".to_vec()))
            } else {
                Err(io::Error::other("unexpected url"))
            }
        });

        let expected_path = config.icon_cache_dir().join(ICON_GENERIC_BASENAME);
        assert_eq!(resolved, Some(expected_path.clone()));
        assert_eq!(
            fs::read(&expected_path).expect("read generic icon"),
            b"generic-bytes"
        );
        assert_eq!(
            fetched_urls,
            vec![
                format!("{ICON_SOURCE_CDN_BASE_URL}/{ICON_PNG_VARIANT_DIR}/doge.png"),
                format!(
                    "{ICON_SOURCE_CDN_BASE_URL}/{ICON_PNG_VARIANT_DIR}/{ICON_GENERIC_BASENAME}"
                ),
            ]
        );
    }

    #[test]
    fn icons_repair_empty_cache_file_before_refetching() {
        let config = config_in_tempdir();
        let path = config.icon_cache_dir().join("eth.png");
        write_atomic(&path, b"").expect("write empty icon");

        let resolved = resolve_icon_path_with(&config, "ETH", &mut |_| {
            Ok(FetchOutcome::Bytes(b"eth-bytes".to_vec()))
        });

        assert_eq!(resolved, Some(path.clone()));
        assert_eq!(fs::read(&path).expect("read repaired icon"), b"eth-bytes");
    }

    #[test]
    fn icons_return_none_when_symbol_and_generic_download_fail() {
        let config = config_in_tempdir();

        let resolved =
            resolve_icon_path_with(&config, "BTC", &mut |_| Err(io::Error::other("offline")));

        assert_eq!(resolved, None);
    }
}
