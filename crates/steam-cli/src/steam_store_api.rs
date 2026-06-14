use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use base64::Engine as _;
use prost::Message;
use serde::Deserialize;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

use crate::config::{RuntimeConfig, SteamSearchApi};

pub const SEARCH_SUGGESTIONS_ENDPOINT: &str =
    "https://api.steampowered.com/IStoreQueryService/SearchSuggestions/v1";
pub const STORE_SEARCH_ENDPOINT: &str = "https://store.steampowered.com/api/storesearch";
pub const FEATURED_CATEGORIES_ENDPOINT: &str =
    "https://store.steampowered.com/api/featuredcategories";
const SEARCH_SUGGESTIONS_ENDPOINT_ENV: &str = "STEAM_SEARCH_SUGGESTIONS_ENDPOINT";
const STORE_SEARCH_ENDPOINT_ENV: &str = "STEAM_STORE_SEARCH_ENDPOINT";
const FEATURED_CATEGORIES_ENDPOINT_ENV: &str = "STEAM_FEATURED_CATEGORIES_ENDPOINT";
const SEARCH_ORIGIN: &str = "https://store.steampowered.com";
const USER_AGENT: &str = "nils-alfredworkflow-steam-search/0.1.0";

const COVER_CACHE_SUBDIR: &str = "steam-covers";
const COVER_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const COVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(4);
const COVER_MAX_WORKERS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamSearchResult {
    pub app_id: u32,
    pub name: String,
    pub price: Option<SteamPrice>,
    pub item_type: SteamItemType,
    pub platforms: SteamPlatforms,
    /// Remote cover-art URL used to populate the local cache.
    pub image_url: Option<String>,
    /// Local cached cover path (set after a successful cache hit/download).
    pub cover_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamItemType {
    Game,
    Demo,
    Dlc,
    Tool,
    Soundtrack,
    Application,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SteamPrice {
    pub final_price_cents: Option<u32>,
    pub final_formatted: Option<String>,
    pub original_price_cents: Option<u32>,
    pub original_formatted: Option<String>,
    pub discount_percent: Option<u32>,
}

impl SteamPrice {
    pub fn compute_discount_percent(
        original_cents: Option<u32>,
        final_cents: Option<u32>,
    ) -> Option<u32> {
        let original = original_cents?;
        let final_value = final_cents?;
        if original == 0 || final_value >= original {
            return None;
        }

        let saved = original - final_value;
        let percent = (u64::from(saved) * 100 + u64::from(original) / 2) / u64::from(original);
        if percent == 0 {
            None
        } else {
            Some(u32::try_from(percent).unwrap_or(u32::MAX))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SteamPlatforms {
    pub windows: bool,
    pub mac: bool,
    pub linux: bool,
}

impl SteamItemType {
    pub fn from_search_suggestions_code(code: u32) -> Self {
        match code {
            0 => Self::Game,
            1 => Self::Demo,
            4 => Self::Dlc,
            6 => Self::Tool,
            11 => Self::Soundtrack,
            _ => Self::Unknown,
        }
    }

    pub fn from_featured_type(code: Option<u32>) -> Self {
        match code.unwrap_or(0) {
            0 => Self::Game,
            4 => Self::Dlc,
            _ => Self::Unknown,
        }
    }

    pub fn from_storesearch_type(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "app" => Self::Application,
            "demo" => Self::Demo,
            "dlc" => Self::Dlc,
            "tool" => Self::Tool,
            "soundtrack" => Self::Soundtrack,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Game => "Game",
            Self::Demo => "Demo",
            Self::Dlc => "DLC",
            Self::Tool => "Tool",
            Self::Soundtrack => "Soundtrack",
            Self::Application => "App",
            Self::Unknown => "Unknown",
        }
    }
}

pub fn search_apps(
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    let client = reqwest::blocking::Client::new();

    let mut results = match config.search_api {
        SteamSearchApi::SearchSuggestions => {
            search_apps_with_search_suggestions(&client, config, query)?
        }
        SteamSearchApi::StoreSearch => search_apps_with_store_search(&client, config, query)?,
    };

    cache_covers_if_enabled(config, &mut results);
    Ok(results)
}

/// Steam's hashless capsule asset URL, derivable from the app id alone so it
/// works for every search/specials row regardless of backend.
fn capsule_image_url(app_id: u32) -> String {
    format!(
        "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{app_id}/capsule_184x69.jpg"
    )
}

fn cache_covers_if_enabled(config: &RuntimeConfig, results: &mut [SteamSearchResult]) {
    if let Some(base) = config
        .cover_cache_dir
        .as_deref()
        .filter(|dir| config.show_covers && !dir.is_empty())
    {
        cache_covers(results, base);
    }
}

pub fn fetch_specials(
    config: &RuntimeConfig,
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    let client = reqwest::blocking::Client::new();
    let endpoint = resolve_endpoint(
        FEATURED_CATEGORIES_ENDPOINT_ENV,
        FEATURED_CATEGORIES_ENDPOINT,
    );
    let params = build_featured_categories_query_params(config);

    let response = client
        .get(endpoint)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .query(&params)
        .send()
        .map_err(|source| SteamStoreApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .bytes()
        .map_err(|source| SteamStoreApiError::Transport { source })?
        .to_vec();

    let mut results = parse_featured_categories_response(status_code, &body)?;
    results.truncate(usize::from(config.specials_max_results));

    cache_covers_if_enabled(config, &mut results);
    Ok(results)
}

fn cover_cache_root(base: &str) -> PathBuf {
    Path::new(base).join(COVER_CACHE_SUBDIR)
}

/// Download missing/stale covers into the local cache in parallel, then record
/// the local path on every row whose cover is cached.
///
/// Best-effort: any failed download simply leaves that row without a cached
/// cover, so the rendered feedback degrades to no icon rather than erroring.
fn cache_covers(results: &mut [SteamSearchResult], base_cache_dir: &str) {
    let dir = cover_cache_root(base_cache_dir);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }

    let jobs: Vec<(String, PathBuf)> = results
        .iter()
        .filter_map(|result| {
            let url = result.image_url.as_deref().filter(|url| !url.is_empty())?;
            let path = cover_cache_path(&dir, result.app_id);
            if is_cover_fresh(&path) {
                return None;
            }
            Some((url.to_string(), path))
        })
        .collect();

    if !jobs.is_empty()
        && let Ok(client) = build_blocking_client(Some(USER_AGENT), Some(COVER_REQUEST_TIMEOUT))
    {
        let worker_count = jobs.len().min(COVER_MAX_WORKERS);
        let next = AtomicUsize::new(0);

        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                scope.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some((url, path)) = jobs.get(index) else {
                            break;
                        };
                        download_cover(&client, url, path);
                    }
                });
            }
        });
    }

    for result in results.iter_mut() {
        if result.image_url.is_none() {
            continue;
        }
        let path = cover_cache_path(&dir, result.app_id);
        if path.is_file() {
            result.cover_path = Some(path.to_string_lossy().into_owned());
        }
    }
}

fn cover_cache_path(dir: &Path, app_id: u32) -> PathBuf {
    dir.join(format!("{app_id}.jpg"))
}

fn is_cover_fresh(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.len() == 0 {
        return false;
    }
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    match SystemTime::now().duration_since(modified) {
        Ok(age) => age < COVER_CACHE_TTL,
        // A modification time in the future is unexpected; treat as fresh.
        Err(_) => true,
    }
}

fn download_cover(client: &reqwest::blocking::Client, url: &str, path: &Path) {
    let Ok(response) = client.get(url).send() else {
        return;
    };
    if !response.status().is_success() {
        return;
    }
    let Ok(bytes) = response.bytes() else {
        return;
    };
    if bytes.is_empty() {
        return;
    }

    // Write to a temp sibling then rename so feedback never sees a partial file.
    let temp_path = path.with_extension("jpg.part");
    if std::fs::write(&temp_path, &bytes).is_ok() {
        let _ = std::fs::rename(&temp_path, path);
    }
}

pub fn build_query_params(config: &RuntimeConfig, query: &str) -> Vec<(String, String)> {
    build_search_suggestions_query_params(config, query)
}

fn build_featured_categories_query_params(config: &RuntimeConfig) -> Vec<(String, String)> {
    let mut params = vec![("cc".to_string(), config.region.clone())];
    if !config.language.is_empty() {
        params.push(("l".to_string(), config.language.clone()));
    }
    params
}

pub fn parse_search_response(
    status_code: u16,
    body: &[u8],
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    parse_search_suggestions_response(status_code, body)
}

fn search_apps_with_search_suggestions(
    client: &reqwest::blocking::Client,
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    let endpoint = resolve_endpoint(SEARCH_SUGGESTIONS_ENDPOINT_ENV, SEARCH_SUGGESTIONS_ENDPOINT);
    let params = build_search_suggestions_query_params(config, query);

    let response = client
        .get(endpoint)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::ORIGIN, SEARCH_ORIGIN)
        .query(&params)
        .send()
        .map_err(|source| SteamStoreApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .bytes()
        .map_err(|source| SteamStoreApiError::Transport { source })?
        .to_vec();

    parse_search_suggestions_response(status_code, &body)
}

fn search_apps_with_store_search(
    client: &reqwest::blocking::Client,
    config: &RuntimeConfig,
    query: &str,
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    let endpoint = resolve_endpoint(STORE_SEARCH_ENDPOINT_ENV, STORE_SEARCH_ENDPOINT);
    let params = build_store_search_query_params(config, query);

    let response = client
        .get(endpoint)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .query(&params)
        .send()
        .map_err(|source| SteamStoreApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .bytes()
        .map_err(|source| SteamStoreApiError::Transport { source })?
        .to_vec();

    parse_store_search_response(status_code, &body)
}

fn build_search_suggestions_query_params(
    config: &RuntimeConfig,
    query: &str,
) -> Vec<(String, String)> {
    let request_payload = SearchSuggestionsRequest {
        context: Some(SearchBrowseContext {
            language: config.language.clone(),
            country_code: config.region.to_ascii_uppercase(),
        }),
        query: query.to_string(),
        max_results: config.max_results.into(),
        scope: String::new(),
        options: Some(SearchSuggestionsOptions {
            include_apps: true,
            include_associated_packages: true,
        }),
    };

    let encoded_payload =
        base64::engine::general_purpose::STANDARD.encode(request_payload.encode_to_vec());

    vec![
        ("origin".to_string(), SEARCH_ORIGIN.to_string()),
        ("input_protobuf_encoded".to_string(), encoded_payload),
    ]
}

fn build_store_search_query_params(config: &RuntimeConfig, query: &str) -> Vec<(String, String)> {
    let mut params = vec![
        ("term".to_string(), query.to_string()),
        ("cc".to_string(), config.region.clone()),
        ("json".to_string(), "1".to_string()),
        ("max_results".to_string(), config.max_results.to_string()),
    ];

    if !config.language.is_empty() {
        params.push(("l".to_string(), config.language.clone()));
    }

    params
}

fn parse_search_suggestions_response(
    status_code: u16,
    body: &[u8],
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(SteamStoreApiError::Http {
            status: status_code,
            message,
        });
    }

    let payload = SearchSuggestionsResponse::decode(body).map_err(|source| {
        SteamStoreApiError::InvalidResponse(ResponseDecodeError::Protobuf(source))
    })?;

    let results = payload
        .results
        .into_iter()
        .filter_map(|item| {
            let app_id = item.app_id?;
            if app_id == 0 {
                return None;
            }

            let name = item.name.trim().to_string();
            if name.is_empty() {
                return None;
            }

            let price = item.prices.into_iter().find_map(|candidate| {
                let final_formatted = candidate
                    .final_formatted
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                let original_formatted = candidate
                    .original_formatted
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());

                if candidate.final_price_cents.is_none() && final_formatted.is_none() {
                    return None;
                }

                let discount_percent = SteamPrice::compute_discount_percent(
                    candidate.original_price_cents,
                    candidate.final_price_cents,
                );

                Some(SteamPrice {
                    final_price_cents: candidate.final_price_cents,
                    final_formatted,
                    original_price_cents: candidate.original_price_cents,
                    original_formatted,
                    discount_percent,
                })
            });

            let item_type =
                SteamItemType::from_search_suggestions_code(item.item_type_code.unwrap_or(0));

            // SearchSuggestions does not publish a stable platform contract.
            let platforms = SteamPlatforms::default();

            Some(SteamSearchResult {
                app_id,
                name,
                price,
                item_type,
                platforms,
                image_url: Some(capsule_image_url(app_id)),
                cover_path: None,
            })
        })
        .collect();

    Ok(results)
}

fn parse_store_search_response(
    status_code: u16,
    body: &[u8],
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(SteamStoreApiError::Http {
            status: status_code,
            message,
        });
    }

    let body_text = std::str::from_utf8(body)
        .map_err(|source| SteamStoreApiError::InvalidResponse(ResponseDecodeError::Utf8(source)))?;
    let payload: StoreSearchResponse = serde_json::from_str(body_text)
        .map_err(|source| SteamStoreApiError::InvalidResponse(ResponseDecodeError::Json(source)))?;

    let results = payload
        .items
        .into_iter()
        .filter_map(|item| {
            let app_id = item.id?;
            if app_id == 0 {
                return None;
            }

            let name = item.name.trim().to_string();
            if name.is_empty() {
                return None;
            }

            let price = item.price.map(|price| {
                let final_formatted = price
                    .final_formatted
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                let original_formatted = price
                    .initial_formatted
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                let discount_percent = SteamPrice::compute_discount_percent(
                    price.original_price_cents,
                    price.final_price_cents,
                );

                SteamPrice {
                    final_price_cents: price.final_price_cents,
                    final_formatted,
                    original_price_cents: price.original_price_cents,
                    original_formatted,
                    discount_percent,
                }
            });

            let platforms = SteamPlatforms {
                windows: item.platforms.windows,
                mac: item.platforms.mac,
                linux: item.platforms.linux,
            };
            let item_type = SteamItemType::from_storesearch_type(&item.item_type);

            Some(SteamSearchResult {
                app_id,
                name,
                price,
                item_type,
                platforms,
                image_url: Some(capsule_image_url(app_id)),
                cover_path: None,
            })
        })
        .collect();

    Ok(results)
}

pub fn parse_featured_categories_response(
    status_code: u16,
    body: &[u8],
) -> Result<Vec<SteamSearchResult>, SteamStoreApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(SteamStoreApiError::Http {
            status: status_code,
            message,
        });
    }

    let body_text = std::str::from_utf8(body)
        .map_err(|source| SteamStoreApiError::InvalidResponse(ResponseDecodeError::Utf8(source)))?;
    let payload: FeaturedCategoriesResponse = serde_json::from_str(body_text)
        .map_err(|source| SteamStoreApiError::InvalidResponse(ResponseDecodeError::Json(source)))?;

    // The specials carousel alone is capped at ~10. Merge the discounted titles
    // across every featured section so the empty-query view can show a larger
    // discount ranking from a single official request.
    let sections = [
        payload.specials,
        payload.top_sellers,
        payload.new_releases,
        payload.coming_soon,
    ];

    let mut seen = HashSet::new();
    let mut results: Vec<SteamSearchResult> = Vec::new();
    for section in sections.into_iter().flatten() {
        for item in section.items {
            let Some(result) = featured_item_to_discounted_result(item) else {
                continue;
            };
            if seen.insert(result.app_id) {
                results.push(result);
            }
        }
    }

    results.sort_by(|a, b| {
        let a_discount = a.price.as_ref().and_then(|price| price.discount_percent);
        let b_discount = b.price.as_ref().and_then(|price| price.discount_percent);
        b_discount.unwrap_or(0).cmp(&a_discount.unwrap_or(0))
    });

    Ok(results)
}

fn featured_item_to_discounted_result(item: FeaturedItem) -> Option<SteamSearchResult> {
    let app_id = item.id?;
    if app_id == 0 {
        return None;
    }

    let name = item.name.trim().to_string();
    if name.is_empty() {
        return None;
    }

    // Sections other than `specials` mix in full-price titles; keep only the
    // ones that actually carry a discount.
    let discount_percent = item
        .discount_percent
        .filter(|percent| *percent > 0)
        .or_else(|| SteamPrice::compute_discount_percent(item.original_price, item.final_price))?;

    let currency = item.currency.as_deref();
    let final_formatted = item
        .final_price
        .and_then(|cents| format_price(cents, currency));
    let original_formatted = item
        .original_price
        .and_then(|cents| format_price(cents, currency));

    let price = Some(SteamPrice {
        final_price_cents: item.final_price,
        final_formatted,
        original_price_cents: item.original_price,
        original_formatted,
        discount_percent: Some(discount_percent),
    });

    let platforms = SteamPlatforms {
        windows: item.windows_available,
        mac: item.mac_available,
        linux: item.linux_available,
    };
    let item_type = SteamItemType::from_featured_type(item.item_type_code);

    let image_url = item
        .small_capsule_image
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Some(SteamSearchResult {
        app_id,
        name,
        price,
        item_type,
        platforms,
        image_url,
        cover_path: None,
    })
}

// featuredcategories returns integer minor units (price * 100) plus a currency
// code, but no pre-formatted price string, so we render one here.
fn format_price(cents: u32, currency: Option<&str>) -> Option<String> {
    let code = currency
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_ascii_uppercase();

    let (symbol, decimals) = currency_format_spec(&code);
    let whole = cents / 100;
    let grouped = group_thousands(u64::from(whole));

    let amount = if decimals == 0 {
        grouped
    } else {
        format!("{grouped}.{minor:02}", minor = cents % 100)
    };

    Some(match symbol {
        Some(symbol) => format!("{symbol}{amount}"),
        None => format!("{amount} {code}"),
    })
}

fn currency_format_spec(code: &str) -> (Option<&'static str>, u32) {
    match code {
        "USD" => (Some("$"), 2),
        "EUR" => (Some("€"), 2),
        "GBP" => (Some("£"), 2),
        "JPY" => (Some("¥"), 0),
        "KRW" => (Some("₩"), 0),
        "TWD" => (Some("NT$ "), 0),
        "HKD" => (Some("HK$ "), 2),
        "CNY" => (Some("¥ "), 2),
        _ => (None, 2),
    }
}

fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (len - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn resolve_endpoint(env_key: &str, default_endpoint: &str) -> String {
    std::env::var(env_key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_endpoint.to_string())
}

fn extract_error_message(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?.trim();
    if text.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return first_non_empty_string(&[
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(serde_json::Value::as_str),
            value
                .get("error")
                .and_then(|error| error.get("detail"))
                .and_then(serde_json::Value::as_str),
            value.get("message").and_then(serde_json::Value::as_str),
            value.get("detail").and_then(serde_json::Value::as_str),
            value.get("error").and_then(serde_json::Value::as_str),
        ]);
    }

    Some(text.to_string())
}

fn first_non_empty_string(candidates: &[Option<&str>]) -> Option<String> {
    candidates
        .iter()
        .flatten()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[derive(Debug, Error)]
pub enum SteamStoreApiError {
    #[error("steam store request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("steam store api error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid steam store response")]
    InvalidResponse(#[source] ResponseDecodeError),
}

#[derive(Debug, Error)]
pub enum ResponseDecodeError {
    #[error(transparent)]
    Protobuf(#[from] prost::DecodeError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionsRequest {
    #[prost(message, optional, tag = "2")]
    context: Option<SearchBrowseContext>,
    #[prost(string, tag = "3")]
    query: String,
    #[prost(uint32, tag = "4")]
    max_results: u32,
    #[prost(string, tag = "5")]
    scope: String,
    #[prost(message, optional, tag = "6")]
    options: Option<SearchSuggestionsOptions>,
}

#[derive(Clone, PartialEq, Message)]
struct SearchBrowseContext {
    #[prost(string, tag = "1")]
    language: String,
    #[prost(string, tag = "3")]
    country_code: String,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionsOptions {
    #[prost(bool, tag = "1")]
    include_apps: bool,
    #[prost(bool, tag = "16")]
    include_associated_packages: bool,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionsResponse {
    #[prost(message, repeated, tag = "3")]
    results: Vec<SearchSuggestionResult>,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionResult {
    #[prost(optional, uint32, tag = "2")]
    app_id: Option<u32>,
    #[prost(string, tag = "6")]
    name: String,
    #[prost(optional, uint32, tag = "10")]
    item_type_code: Option<u32>,
    #[prost(message, repeated, tag = "40")]
    prices: Vec<SearchSuggestionPrice>,
}

#[derive(Clone, PartialEq, Message)]
struct SearchSuggestionPrice {
    #[prost(optional, uint32, tag = "5")]
    final_price_cents: Option<u32>,
    #[prost(optional, uint32, tag = "6")]
    original_price_cents: Option<u32>,
    #[prost(optional, string, tag = "8")]
    final_formatted: Option<String>,
    #[prost(optional, string, tag = "9")]
    original_formatted: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct StoreSearchResponse {
    #[serde(default)]
    items: Vec<StoreSearchItem>,
}

#[derive(Debug, Default, Deserialize)]
struct StoreSearchItem {
    #[serde(default)]
    id: Option<u32>,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    item_type: String,
    #[serde(default)]
    price: Option<StoreSearchPrice>,
    #[serde(default)]
    platforms: StoreSearchPlatformPayload,
}

#[derive(Debug, Default, Deserialize)]
struct StoreSearchPrice {
    #[serde(default, rename = "final")]
    final_price_cents: Option<u32>,
    #[serde(default, rename = "initial")]
    original_price_cents: Option<u32>,
    #[serde(default)]
    final_formatted: Option<String>,
    #[serde(default)]
    initial_formatted: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct StoreSearchPlatformPayload {
    #[serde(default)]
    windows: bool,
    #[serde(default)]
    mac: bool,
    #[serde(default)]
    linux: bool,
}

#[derive(Debug, Default, Deserialize)]
struct FeaturedCategoriesResponse {
    #[serde(default)]
    specials: Option<FeaturedSection>,
    #[serde(default)]
    top_sellers: Option<FeaturedSection>,
    #[serde(default)]
    new_releases: Option<FeaturedSection>,
    #[serde(default)]
    coming_soon: Option<FeaturedSection>,
}

#[derive(Debug, Default, Deserialize)]
struct FeaturedSection {
    #[serde(default)]
    items: Vec<FeaturedItem>,
}

#[derive(Debug, Default, Deserialize)]
struct FeaturedItem {
    #[serde(default)]
    id: Option<u32>,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    item_type_code: Option<u32>,
    #[serde(default)]
    discount_percent: Option<u32>,
    #[serde(default)]
    original_price: Option<u32>,
    #[serde(default)]
    final_price: Option<u32>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    windows_available: bool,
    #[serde(default)]
    mac_available: bool,
    #[serde(default)]
    linux_available: bool,
    #[serde(default)]
    small_capsule_image: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_config(region: &str, language: &str, max_results: u8) -> RuntimeConfig {
        RuntimeConfig {
            region: region.to_string(),
            region_options: vec![region.to_string()],
            show_region_options: false,
            max_results,
            specials_max_results: 30,
            language: language.to_string(),
            search_api: SteamSearchApi::SearchSuggestions,
            show_covers: false,
            cover_cache_dir: None,
        }
    }

    #[test]
    fn steam_store_api_build_query_params_includes_query_region_and_language_when_configured() {
        let params = build_query_params(&fixture_config("jp", "schinese", 7), "persona");
        let encoded_payload = params
            .iter()
            .find(|(key, _)| key == "input_protobuf_encoded")
            .map(|(_, value)| value)
            .expect("input_protobuf_encoded must exist");

        let payload_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded_payload)
            .expect("base64 payload should decode");
        let payload = SearchSuggestionsRequest::decode(payload_bytes.as_slice())
            .expect("protobuf should decode");
        let context = payload.context.expect("context must exist");

        assert!(params.contains(&("origin".to_string(), SEARCH_ORIGIN.to_string())));
        assert_eq!(context.language, "schinese");
        assert_eq!(context.country_code, "JP");
        assert_eq!(payload.query, "persona");
        assert_eq!(payload.max_results, 7);
        assert_eq!(payload.scope, "");

        let options = payload.options.expect("options must exist");
        assert!(options.include_apps);
        assert!(options.include_associated_packages);
    }

    #[test]
    fn steam_store_api_build_query_params_uses_empty_language_when_not_configured() {
        let params = build_query_params(&fixture_config("jp", "", 7), "persona");
        let encoded_payload = params
            .iter()
            .find(|(key, _)| key == "input_protobuf_encoded")
            .map(|(_, value)| value)
            .expect("input_protobuf_encoded must exist");

        let payload_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded_payload)
            .expect("base64 payload should decode");
        let payload = SearchSuggestionsRequest::decode(payload_bytes.as_slice())
            .expect("protobuf should decode");
        let context = payload.context.expect("context must exist");

        assert_eq!(context.language, "");
        assert_eq!(context.country_code, "JP");
    }

    #[test]
    fn steam_store_api_build_store_search_query_params_omits_language_when_empty() {
        let config = RuntimeConfig {
            language: "".to_string(),
            search_api: SteamSearchApi::StoreSearch,
            ..fixture_config("us", "english", 8)
        };
        let params = build_store_search_query_params(&config, "dota");

        assert!(params.contains(&("term".to_string(), "dota".to_string())));
        assert!(params.contains(&("cc".to_string(), "us".to_string())));
        assert!(params.contains(&("max_results".to_string(), "8".to_string())));
        assert!(!params.iter().any(|(key, _)| key == "l"));
    }

    #[test]
    fn steam_store_api_parse_search_response_extracts_expected_fields() {
        let body = SearchSuggestionsResponse {
            results: vec![SearchSuggestionResult {
                app_id: Some(730),
                name: "Counter-Strike 2".to_string(),
                item_type_code: Some(0),
                prices: vec![SearchSuggestionPrice {
                    final_price_cents: Some(0),
                    final_formatted: Some("Free".to_string()),
                    original_price_cents: None,
                    original_formatted: None,
                }],
            }],
        }
        .encode_to_vec();

        let results = parse_search_response(200, body.as_slice()).expect("response should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, 730);
        assert_eq!(results[0].name, "Counter-Strike 2");
        assert_eq!(
            results[0].price,
            Some(SteamPrice {
                final_price_cents: Some(0),
                final_formatted: Some("Free".to_string()),
                ..SteamPrice::default()
            })
        );
        assert_eq!(results[0].item_type, SteamItemType::Game);
        assert!(!results[0].platforms.windows);
        assert!(!results[0].platforms.mac);
        assert!(!results[0].platforms.linux);
        assert_eq!(
            results[0].image_url.as_deref(),
            Some(
                "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/730/capsule_184x69.jpg"
            )
        );
    }

    #[test]
    fn steam_store_api_parse_search_response_carries_discount_fields() {
        let body = SearchSuggestionsResponse {
            results: vec![SearchSuggestionResult {
                app_id: Some(35704),
                name: "Hero Siege".to_string(),
                item_type_code: Some(0),
                prices: vec![SearchSuggestionPrice {
                    final_price_cents: Some(5000),
                    final_formatted: Some("NT$ 50.00".to_string()),
                    original_price_cents: Some(15200),
                    original_formatted: Some("NT$ 152.00".to_string()),
                }],
            }],
        }
        .encode_to_vec();

        let results = parse_search_response(200, body.as_slice()).expect("response should parse");

        let price = results[0].price.as_ref().expect("price should exist");
        assert_eq!(price.final_price_cents, Some(5000));
        assert_eq!(price.original_price_cents, Some(15200));
        assert_eq!(price.original_formatted.as_deref(), Some("NT$ 152.00"));
        assert_eq!(price.discount_percent, Some(67));
    }

    #[test]
    fn steam_store_api_parse_store_search_response_extracts_expected_fields() {
        let body = br#"{
            "items": [
                {
                    "id": 730,
                    "name": "Counter-Strike 2",
                    "type": "app",
                    "price": {"final": 0, "final_formatted": "Free"},
                    "platforms": {"windows": true, "mac": false, "linux": true}
                }
            ]
        }"#;

        let results = parse_store_search_response(200, body).expect("response should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, 730);
        assert_eq!(results[0].name, "Counter-Strike 2");
        assert_eq!(
            results[0].price,
            Some(SteamPrice {
                final_price_cents: Some(0),
                final_formatted: Some("Free".to_string()),
                ..SteamPrice::default()
            })
        );
        assert_eq!(results[0].item_type, SteamItemType::Application);
        assert!(results[0].platforms.windows);
        assert!(!results[0].platforms.mac);
        assert!(results[0].platforms.linux);
    }

    #[test]
    fn steam_store_api_parse_store_search_response_carries_discount_fields() {
        let body = br#"{
            "items": [
                {
                    "id": 1091500,
                    "name": "Cyberpunk 2077",
                    "type": "app",
                    "price": {"initial": 159900, "final": 79900, "currency": "TWD"},
                    "platforms": {"windows": true, "mac": false, "linux": false}
                }
            ]
        }"#;

        let results = parse_store_search_response(200, body).expect("response should parse");
        let price = results[0].price.as_ref().expect("price should exist");
        assert_eq!(price.final_price_cents, Some(79900));
        assert_eq!(price.original_price_cents, Some(159900));
        assert_eq!(price.discount_percent, Some(50));
    }

    #[test]
    fn steam_store_api_compute_discount_percent_handles_edges() {
        assert_eq!(
            SteamPrice::compute_discount_percent(Some(15200), Some(5000)),
            Some(67),
        );
        assert_eq!(
            SteamPrice::compute_discount_percent(Some(1000), Some(1000)),
            None,
        );
        assert_eq!(
            SteamPrice::compute_discount_percent(Some(1000), Some(1500)),
            None,
        );
        assert_eq!(SteamPrice::compute_discount_percent(Some(0), Some(0)), None);
        assert_eq!(SteamPrice::compute_discount_percent(None, Some(500)), None);
        assert_eq!(SteamPrice::compute_discount_percent(Some(500), None), None);
    }

    #[test]
    fn steam_store_api_parse_search_response_ignores_partial_items() {
        let body = SearchSuggestionsResponse {
            results: vec![
                SearchSuggestionResult {
                    app_id: Some(0),
                    name: "skip-id".to_string(),
                    item_type_code: Some(0),
                    prices: vec![],
                },
                SearchSuggestionResult {
                    app_id: Some(10),
                    name: "".to_string(),
                    item_type_code: Some(0),
                    prices: vec![],
                },
                SearchSuggestionResult {
                    app_id: None,
                    name: "missing-id".to_string(),
                    item_type_code: Some(0),
                    prices: vec![],
                },
                SearchSuggestionResult {
                    app_id: Some(570),
                    name: "Dota 2".to_string(),
                    item_type_code: Some(0),
                    prices: vec![],
                },
            ],
        }
        .encode_to_vec();

        let results = parse_search_response(200, body.as_slice()).expect("response should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, 570);
        assert_eq!(results[0].name, "Dota 2");
    }

    #[test]
    fn steam_store_api_parse_search_response_supports_empty_items() {
        let body = SearchSuggestionsResponse { results: vec![] }.encode_to_vec();
        let results =
            parse_search_response(200, body.as_slice()).expect("empty payload should parse");

        assert!(results.is_empty());
    }

    #[test]
    fn steam_store_api_parse_search_response_surfaces_api_error_message() {
        let body = br#"{"message":"upstream unavailable"}"#;
        let err = parse_search_response(503, body).expect_err("non-2xx should fail");

        match err {
            SteamStoreApiError::Http { status, message } => {
                assert_eq!(status, 503);
                assert_eq!(message, "upstream unavailable");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn steam_store_api_maps_search_suggestions_type_codes() {
        assert_eq!(
            SteamItemType::from_search_suggestions_code(0),
            SteamItemType::Game
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(1),
            SteamItemType::Demo
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(4),
            SteamItemType::Dlc
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(6),
            SteamItemType::Tool
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(11),
            SteamItemType::Soundtrack
        );
        assert_eq!(
            SteamItemType::from_search_suggestions_code(999),
            SteamItemType::Unknown
        );
    }

    #[test]
    fn steam_store_api_maps_storesearch_type_values() {
        assert_eq!(
            SteamItemType::from_storesearch_type("app"),
            SteamItemType::Application
        );
        assert_eq!(
            SteamItemType::from_storesearch_type("demo"),
            SteamItemType::Demo
        );
        assert_eq!(
            SteamItemType::from_storesearch_type("dlc"),
            SteamItemType::Dlc
        );
        assert_eq!(
            SteamItemType::from_storesearch_type("unknown-type"),
            SteamItemType::Unknown
        );
    }

    #[test]
    fn steam_store_api_parse_search_response_rejects_malformed_success_payload() {
        let err =
            parse_search_response(200, b"not-protobuf").expect_err("invalid payload should fail");

        assert!(matches!(err, SteamStoreApiError::InvalidResponse(_)));
    }

    #[test]
    fn steam_store_api_parse_featured_categories_maps_specials_with_formatted_prices() {
        let body = br#"{
            "specials": {
                "id": 0,
                "name": "Specials",
                "items": [
                    {
                        "id": 1118520,
                        "type": 0,
                        "name": "Paralives",
                        "discounted": true,
                        "discount_percent": 75,
                        "original_price": 498000,
                        "final_price": 124500,
                        "currency": "JPY",
                        "small_capsule_image": "https://cdn.example/apps/1118520/capsule_184x69.jpg?t=1",
                        "windows_available": true,
                        "mac_available": true,
                        "linux_available": false
                    }
                ]
            }
        }"#;

        let results = parse_featured_categories_response(200, body).expect("response should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, 1118520);
        assert_eq!(results[0].name, "Paralives");
        assert_eq!(results[0].item_type, SteamItemType::Game);
        assert!(results[0].platforms.windows);
        assert!(results[0].platforms.mac);
        assert!(!results[0].platforms.linux);
        assert_eq!(
            results[0].image_url.as_deref(),
            Some("https://cdn.example/apps/1118520/capsule_184x69.jpg?t=1")
        );

        let price = results[0].price.as_ref().expect("price should exist");
        assert_eq!(price.final_price_cents, Some(124500));
        assert_eq!(price.original_price_cents, Some(498000));
        assert_eq!(price.final_formatted.as_deref(), Some("¥1,245"));
        assert_eq!(price.original_formatted.as_deref(), Some("¥4,980"));
        assert_eq!(price.discount_percent, Some(75));
    }

    #[test]
    fn steam_store_api_parse_featured_categories_skips_partial_and_undiscounted_items() {
        let body = br#"{
            "specials": {
                "items": [
                    {"id": 0, "name": "skip-zero-id", "discount_percent": 50, "original_price": 200, "final_price": 100, "currency": "USD"},
                    {"id": 730, "name": "", "discount_percent": 50, "original_price": 200, "final_price": 100, "currency": "USD"},
                    {"id": 440, "name": "Full Price", "original_price": 1000, "final_price": 1000, "currency": "USD"},
                    {"id": 570, "name": "Dota 2", "discount_percent": 60, "original_price": 1000, "final_price": 400, "currency": "USD"}
                ]
            }
        }"#;
        let results = parse_featured_categories_response(200, body).expect("response should parse");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, 570);

        let empty = parse_featured_categories_response(200, b"{}").expect("missing block parses");
        assert!(empty.is_empty());
    }

    #[test]
    fn steam_store_api_parse_featured_categories_merges_sections_dedupes_and_sorts() {
        let body = br#"{
            "specials": {
                "items": [
                    {"id": 10, "name": "Specials 30%", "discount_percent": 30, "original_price": 1000, "final_price": 700, "currency": "USD"},
                    {"id": 20, "name": "Shared", "discount_percent": 40, "original_price": 1000, "final_price": 600, "currency": "USD"}
                ]
            },
            "top_sellers": {
                "items": [
                    {"id": 20, "name": "Shared Duplicate", "discount_percent": 40, "original_price": 1000, "final_price": 600, "currency": "USD"},
                    {"id": 30, "name": "Top Full Price", "original_price": 1000, "final_price": 1000, "currency": "USD"}
                ]
            },
            "new_releases": {
                "items": [
                    {"id": 40, "name": "New 80%", "discount_percent": 80, "original_price": 1000, "final_price": 200, "currency": "USD"}
                ]
            },
            "coming_soon": {
                "items": [
                    {"id": 50, "name": "Soon 10%", "discount_percent": 10, "original_price": 1000, "final_price": 900, "currency": "USD"}
                ]
            }
        }"#;

        let results = parse_featured_categories_response(200, body).expect("response should parse");

        let ids: Vec<u32> = results.iter().map(|result| result.app_id).collect();
        // 80% (new), 40% (shared, deduped), 30% (specials), 10% (coming soon);
        // the full-price top seller is dropped.
        assert_eq!(ids, vec![40, 20, 10, 50]);
    }

    #[test]
    fn steam_store_api_parse_featured_categories_surfaces_http_errors() {
        let err = parse_featured_categories_response(503, br#"{"message":"down"}"#)
            .expect_err("non-2xx should fail");
        match err {
            SteamStoreApiError::Http { status, message } => {
                assert_eq!(status, 503);
                assert_eq!(message, "down");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn steam_store_api_format_price_renders_known_and_unknown_currencies() {
        assert_eq!(format_price(3999, Some("USD")).as_deref(), Some("$39.99"));
        assert_eq!(format_price(59900, Some("TWD")).as_deref(), Some("NT$ 599"));
        assert_eq!(format_price(498000, Some("JPY")).as_deref(), Some("¥4,980"));
        assert_eq!(
            format_price(123456, Some("CHF")).as_deref(),
            Some("1,234.56 CHF")
        );
        assert_eq!(format_price(1000, None), None);
        assert_eq!(format_price(1000, Some("   ")), None);
    }
}
