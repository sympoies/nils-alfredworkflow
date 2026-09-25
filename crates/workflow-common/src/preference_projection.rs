//! Loader for an external preference projection file.
//!
//! An external preference owner may publish resolved market and weather
//! preferences as one bounded JSON document. Workflows consume it read-only:
//! this module reads the file, validates it strictly against
//! [`PREFERENCE_PROJECTION_SCHEMA`], checks freshness, and returns a typed
//! projection or a privacy-safe [`ProjectionError`]. Error messages never
//! include the file path or any preference value.
//!
//! The canonical contract lives in
//! `crates/workflow-common/docs/preference-projection-contract.md`.

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use alfred_core::Item;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{Map, Value};

/// Schema identifier accepted by this consumer.
pub const PREFERENCE_PROJECTION_SCHEMA: &str = "sympoies.alfred-preference-projection/v1";
/// Maximum accepted file size in bytes.
pub const PREFERENCE_PROJECTION_MAX_BYTES: u64 = 16_384;
/// A projection older than this many seconds is stale.
pub const PREFERENCE_PROJECTION_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60;
/// Tolerated clock skew for `generatedAt` values slightly in the future.
pub const PREFERENCE_PROJECTION_MAX_FUTURE_SKEW_SECS: i64 = 5 * 60;
/// Maximum number of entries in each ordered list.
pub const PREFERENCE_PROJECTION_MAX_LIST_ITEMS: usize = 32;
/// Maximum characters in one watchlist entry.
pub const PREFERENCE_PROJECTION_MAX_WATCHLIST_CHARS: usize = 16;
/// Maximum characters in one location label.
pub const PREFERENCE_PROJECTION_MAX_LOCATION_CHARS: usize = 128;

const TOP_LEVEL_FIELDS: [&str; 7] = [
    "schema",
    "generatedAt",
    "revision",
    "digest",
    "market",
    "weather",
    "sources",
];
const MARKET_FIELDS: [&str; 2] = ["default_quote_currency", "watchlist"];
const WEATHER_FIELDS: [&str; 2] = ["default_location", "saved_locations"];
const SOURCE_FIELDS: [&str; 4] = [
    "market.default_quote_currency",
    "market.watchlist",
    "weather.default_location",
    "weather.saved_locations",
];
const DIGEST_PREFIX: &str = "sha256:";
const DIGEST_HEX_LEN: usize = 64;

/// Provenance of one resolved preference field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferenceSource {
    Profile,
    OwnerOverride,
}

impl PreferenceSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::OwnerOverride => "owner_override",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketPreferences {
    /// Quote currency for fiat watchlist entries. Empty means unset.
    pub default_quote_currency: String,
    pub watchlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherPreferences {
    /// Primary location label. Empty means unset.
    pub default_location: String,
    pub saved_locations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferenceSources {
    pub market_default_quote_currency: PreferenceSource,
    pub market_watchlist: PreferenceSource,
    pub weather_default_location: PreferenceSource,
    pub weather_saved_locations: PreferenceSource,
}

/// A validated projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceProjection {
    pub generated_at: DateTime<Utc>,
    pub revision: Option<u64>,
    pub digest: Option<String>,
    pub market: MarketPreferences,
    pub weather: WeatherPreferences,
    pub sources: PreferenceSources,
}

impl PreferenceProjection {
    /// Weather defaults: `default_location` followed by `saved_locations`,
    /// deduplicated case-insensitively while preserving first occurrence.
    /// Labels are never split on commas.
    pub fn weather_default_locations(&self) -> Vec<String> {
        let mut seen = Vec::<String>::new();
        let mut locations = Vec::new();
        let candidates = std::iter::once(&self.weather.default_location)
            .chain(self.weather.saved_locations.iter());

        for location in candidates {
            if location.is_empty() {
                continue;
            }
            let key = location.to_lowercase();
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            locations.push(location.clone());
        }

        locations
    }
}

/// Privacy-safe failure kinds. Messages name fields, never values or paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    /// The file is missing or unreadable.
    Unavailable,
    /// The file exceeds [`PREFERENCE_PROJECTION_MAX_BYTES`].
    TooLarge,
    /// The file is not UTF-8 JSON.
    Malformed,
    /// The document violates the contract at the named field.
    Invalid { field: &'static str },
    /// `generatedAt` is further in the future than the tolerated skew.
    FutureTimestamp,
    /// `generatedAt` is older than [`PREFERENCE_PROJECTION_MAX_AGE_SECS`].
    Stale { age_secs: i64 },
}

/// Coarse state used for user-visible status wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionState {
    Unavailable,
    Stale,
    Invalid,
}

impl ProjectionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Stale => "stale",
            Self::Invalid => "invalid",
        }
    }
}

impl ProjectionError {
    pub fn state(&self) -> ProjectionState {
        match self {
            Self::Unavailable => ProjectionState::Unavailable,
            Self::Stale { .. } => ProjectionState::Stale,
            Self::TooLarge | Self::Malformed | Self::Invalid { .. } | Self::FutureTimestamp => {
                ProjectionState::Invalid
            }
        }
    }
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(f, "projection file is missing or unreadable"),
            Self::TooLarge => write!(
                f,
                "projection file exceeds {PREFERENCE_PROJECTION_MAX_BYTES} bytes"
            ),
            Self::Malformed => write!(f, "projection file is not valid UTF-8 JSON"),
            Self::Invalid { field } => write!(f, "projection field failed validation: {field}"),
            Self::FutureTimestamp => write!(f, "projection generatedAt is in the future"),
            Self::Stale { age_secs } => write!(
                f,
                "projection is older than 7 days (synced {})",
                format_age(*age_secs)
            ),
        }
    }
}

impl std::error::Error for ProjectionError {}

/// Read, validate, and freshness-check a projection file.
pub fn load_preference_projection(
    path: &Path,
    now: DateTime<Utc>,
) -> Result<PreferenceProjection, ProjectionError> {
    let bytes = read_bounded(path)?;
    let projection = parse_preference_projection(&bytes)?;
    check_freshness(&projection, now)?;
    Ok(projection)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, ProjectionError> {
    let file = File::open(path).map_err(|_| ProjectionError::Unavailable)?;
    let metadata = file.metadata().map_err(|_| ProjectionError::Unavailable)?;
    if !metadata.is_file() {
        return Err(ProjectionError::Unavailable);
    }
    if metadata.len() > PREFERENCE_PROJECTION_MAX_BYTES {
        return Err(ProjectionError::TooLarge);
    }

    let mut bytes = Vec::new();
    file.take(PREFERENCE_PROJECTION_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ProjectionError::Unavailable)?;
    if bytes.len() as u64 > PREFERENCE_PROJECTION_MAX_BYTES {
        return Err(ProjectionError::TooLarge);
    }

    Ok(bytes)
}

/// Validate projection bytes against the contract without a freshness check.
pub fn parse_preference_projection(bytes: &[u8]) -> Result<PreferenceProjection, ProjectionError> {
    if bytes.len() as u64 > PREFERENCE_PROJECTION_MAX_BYTES {
        return Err(ProjectionError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ProjectionError::Malformed)?;
    let value: Value = serde_json::from_str(text).map_err(|_| ProjectionError::Malformed)?;
    let root = value
        .as_object()
        .ok_or(ProjectionError::Invalid { field: "(root)" })?;
    require_exact_fields(root, &TOP_LEVEL_FIELDS, "(root)")?;

    if root.get("schema").and_then(Value::as_str) != Some(PREFERENCE_PROJECTION_SCHEMA) {
        return Err(ProjectionError::Invalid { field: "schema" });
    }

    let generated_at = parse_generated_at(root.get("generatedAt"))?;
    let revision = parse_revision(root.get("revision"))?;
    let digest = parse_digest(root.get("digest"))?;
    if revision.is_some() != digest.is_some() {
        return Err(ProjectionError::Invalid { field: "digest" });
    }

    let market = object_field(root, "market")?;
    require_exact_fields(market, &MARKET_FIELDS, "market")?;
    let market = MarketPreferences {
        default_quote_currency: scalar_label(
            market.get("default_quote_currency"),
            "market.default_quote_currency",
            PREFERENCE_PROJECTION_MAX_WATCHLIST_CHARS,
        )?,
        watchlist: label_list(
            market.get("watchlist"),
            "market.watchlist",
            PREFERENCE_PROJECTION_MAX_WATCHLIST_CHARS,
        )?,
    };

    let weather = object_field(root, "weather")?;
    require_exact_fields(weather, &WEATHER_FIELDS, "weather")?;
    let weather = WeatherPreferences {
        default_location: scalar_label(
            weather.get("default_location"),
            "weather.default_location",
            PREFERENCE_PROJECTION_MAX_LOCATION_CHARS,
        )?,
        saved_locations: label_list(
            weather.get("saved_locations"),
            "weather.saved_locations",
            PREFERENCE_PROJECTION_MAX_LOCATION_CHARS,
        )?,
    };

    let sources = object_field(root, "sources")?;
    require_exact_fields(sources, &SOURCE_FIELDS, "sources")?;
    let sources = PreferenceSources {
        market_default_quote_currency: parse_source(sources, "market.default_quote_currency")?,
        market_watchlist: parse_source(sources, "market.watchlist")?,
        weather_default_location: parse_source(sources, "weather.default_location")?,
        weather_saved_locations: parse_source(sources, "weather.saved_locations")?,
    };

    Ok(PreferenceProjection {
        generated_at,
        revision,
        digest,
        market,
        weather,
        sources,
    })
}

fn check_freshness(
    projection: &PreferenceProjection,
    now: DateTime<Utc>,
) -> Result<(), ProjectionError> {
    let age_secs = (now - projection.generated_at).num_seconds();
    if age_secs < -PREFERENCE_PROJECTION_MAX_FUTURE_SKEW_SECS {
        return Err(ProjectionError::FutureTimestamp);
    }
    if age_secs > PREFERENCE_PROJECTION_MAX_AGE_SECS {
        return Err(ProjectionError::Stale { age_secs });
    }
    Ok(())
}

fn require_exact_fields(
    object: &Map<String, Value>,
    expected: &[&'static str],
    field: &'static str,
) -> Result<(), ProjectionError> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(ProjectionError::Invalid { field });
    }
    Ok(())
}

fn object_field<'a>(
    root: &'a Map<String, Value>,
    field: &'static str,
) -> Result<&'a Map<String, Value>, ProjectionError> {
    root.get(field)
        .and_then(Value::as_object)
        .ok_or(ProjectionError::Invalid { field })
}

fn parse_generated_at(value: Option<&Value>) -> Result<DateTime<Utc>, ProjectionError> {
    const FIELD: &str = "generatedAt";
    let raw = value
        .and_then(Value::as_str)
        .ok_or(ProjectionError::Invalid { field: FIELD })?;
    if raw.len() != 20 {
        return Err(ProjectionError::Invalid { field: FIELD });
    }
    NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%SZ")
        .map(|naive| naive.and_utc())
        .map_err(|_| ProjectionError::Invalid { field: FIELD })
}

fn parse_revision(value: Option<&Value>) -> Result<Option<u64>, ProjectionError> {
    match value {
        Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .filter(|revision| *revision >= 1)
            .map(Some)
            .ok_or(ProjectionError::Invalid { field: "revision" }),
        None => Err(ProjectionError::Invalid { field: "revision" }),
    }
}

fn parse_digest(value: Option<&Value>) -> Result<Option<String>, ProjectionError> {
    const FIELD: &str = "digest";
    match value {
        Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => {
            let hex = raw
                .strip_prefix(DIGEST_PREFIX)
                .ok_or(ProjectionError::Invalid { field: FIELD })?;
            if hex.len() != DIGEST_HEX_LEN || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
                return Err(ProjectionError::Invalid { field: FIELD });
            }
            Ok(Some(raw.clone()))
        }
        _ => Err(ProjectionError::Invalid { field: FIELD }),
    }
}

fn parse_source(
    sources: &Map<String, Value>,
    field: &'static str,
) -> Result<PreferenceSource, ProjectionError> {
    match sources.get(field).and_then(Value::as_str) {
        Some("profile") => Ok(PreferenceSource::Profile),
        Some("owner_override") => Ok(PreferenceSource::OwnerOverride),
        _ => Err(ProjectionError::Invalid { field }),
    }
}

/// A scalar label. Empty means "unset"; otherwise it must be a valid label.
fn scalar_label(
    value: Option<&Value>,
    field: &'static str,
    max_chars: usize,
) -> Result<String, ProjectionError> {
    let raw = value
        .and_then(Value::as_str)
        .ok_or(ProjectionError::Invalid { field })?;
    if raw.is_empty() {
        return Ok(String::new());
    }
    validate_label(raw, field, max_chars)?;
    Ok(raw.to_string())
}

fn label_list(
    value: Option<&Value>,
    field: &'static str,
    max_chars: usize,
) -> Result<Vec<String>, ProjectionError> {
    let items = value
        .and_then(Value::as_array)
        .ok_or(ProjectionError::Invalid { field })?;
    if items.len() > PREFERENCE_PROJECTION_MAX_LIST_ITEMS {
        return Err(ProjectionError::Invalid { field });
    }

    let mut labels = Vec::<String>::with_capacity(items.len());
    for item in items {
        let raw = item.as_str().ok_or(ProjectionError::Invalid { field })?;
        if raw.is_empty() {
            return Err(ProjectionError::Invalid { field });
        }
        validate_label(raw, field, max_chars)?;
        if labels.iter().any(|existing| existing == raw) {
            return Err(ProjectionError::Invalid { field });
        }
        labels.push(raw.to_string());
    }

    Ok(labels)
}

fn validate_label(raw: &str, field: &'static str, max_chars: usize) -> Result<(), ProjectionError> {
    if raw.chars().count() > max_chars
        || raw.trim() != raw
        || raw.chars().any(is_disallowed_label_char)
    {
        return Err(ProjectionError::Invalid { field });
    }
    Ok(())
}

/// Unicode Cc, Cf, Zl, and Zp characters are not allowed in labels.
fn is_disallowed_label_char(ch: char) -> bool {
    ch.is_control() || is_format_char(ch) || ch == '\u{2028}' || ch == '\u{2029}'
}

/// Unicode general category Cf (format) code points.
fn is_format_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{00AD}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061C}'
            | '\u{06DD}'
            | '\u{070F}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08E2}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{13430}'..='\u{1343F}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0001}'
            | '\u{E0020}'..='\u{E007F}'
    )
}

/// Compact relative age label, for example `12m ago`.
pub fn format_age(age_secs: i64) -> String {
    let age = age_secs.max(0);
    if age < 60 {
        "just now".to_string()
    } else if age < 60 * 60 {
        format!("{}m ago", age / 60)
    } else if age < 48 * 60 * 60 {
        format!("{}h ago", age / (60 * 60))
    } else {
        format!("{}d ago", age / (24 * 60 * 60))
    }
}

/// Outcome of consulting a configured projection, used for the status row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionStatus {
    /// The projection supplied the defaults.
    Used {
        revision: Option<u64>,
        generated_at: DateTime<Utc>,
        skipped: usize,
    },
    /// The projection was valid but supplied nothing usable for this workflow.
    Empty {
        revision: Option<u64>,
        skipped: usize,
    },
    /// The projection could not be used.
    Failed(ProjectionError),
}

impl ProjectionStatus {
    /// Status row title. Never contains a path or preference value.
    pub fn title(&self, now: DateTime<Utc>) -> String {
        match self {
            Self::Used {
                revision,
                generated_at,
                ..
            } => format!(
                "Preferences: {} · synced {}",
                projection_label(*revision),
                format_age((now - *generated_at).num_seconds())
            ),
            Self::Empty { revision, .. } => format!(
                "Preferences: {} has no usable entries — using workflow settings",
                projection_label(*revision)
            ),
            Self::Failed(error) => format!(
                "Preferences: projection {} — using workflow settings",
                error.state().as_str()
            ),
        }
    }

    /// Status row subtitle. `used_hint` describes what the projection supplied.
    pub fn subtitle(&self, used_hint: &str) -> String {
        match self {
            Self::Used { skipped, .. } => with_skipped(used_hint.to_string(), *skipped),
            Self::Empty { skipped, .. } => with_skipped(
                "The external preference projection supplied no usable entries.".to_string(),
                *skipped,
            ),
            Self::Failed(error) => format!("Reason: {error}."),
        }
    }

    /// Machine-readable state: `used`, `empty`, `unavailable`, `stale`, `invalid`.
    pub fn state(&self) -> &'static str {
        match self {
            Self::Used { .. } => "used",
            Self::Empty { .. } => "empty",
            Self::Failed(error) => error.state().as_str(),
        }
    }

    /// Non-selectable Alfred status row.
    pub fn to_item(&self, now: DateTime<Utc>, used_hint: &str) -> Item {
        Item::new(self.title(now))
            .with_subtitle(self.subtitle(used_hint))
            .with_valid(false)
    }
}

fn projection_label(revision: Option<u64>) -> String {
    match revision {
        Some(revision) => format!("projection revision {revision}"),
        None => "projection (no revision)".to_string(),
    }
}

fn with_skipped(message: String, skipped: usize) -> String {
    match skipped {
        0 => message,
        1 => format!("{message} 1 entry skipped (unsupported)."),
        n => format!("{message} {n} entries skipped (unsupported)."),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::TimeZone;
    use serde_json::json;

    use super::*;

    const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
            .single()
            .expect("time")
    }

    fn valid_document() -> Value {
        json!({
            "schema": PREFERENCE_PROJECTION_SCHEMA,
            "generatedAt": "2026-03-01T11:48:00Z",
            "revision": 2,
            "digest": DIGEST,
            "market": {
                "default_quote_currency": "TWD",
                "watchlist": ["USD", "JPY", "BTC", "ETH"]
            },
            "weather": {
                "default_location": "Taipei, Taiwan",
                "saved_locations": ["東京", "taipei, taiwan", "Osaka"]
            },
            "sources": {
                "market.default_quote_currency": "profile",
                "market.watchlist": "owner_override",
                "weather.default_location": "profile",
                "weather.saved_locations": "profile"
            }
        })
    }

    fn parse(value: &Value) -> Result<PreferenceProjection, ProjectionError> {
        parse_preference_projection(value.to_string().as_bytes())
    }

    fn invalid_field(value: &Value) -> &'static str {
        match parse(value) {
            Err(ProjectionError::Invalid { field }) => field,
            other => panic!("expected invalid field, got {other:?}"),
        }
    }

    #[test]
    fn parses_valid_projection_preserving_order() {
        let projection = parse(&valid_document()).expect("valid");
        assert_eq!(projection.revision, Some(2));
        assert_eq!(projection.digest.as_deref(), Some(DIGEST));
        assert_eq!(projection.market.default_quote_currency, "TWD");
        assert_eq!(
            projection.market.watchlist,
            vec!["USD", "JPY", "BTC", "ETH"]
        );
        assert_eq!(
            projection.sources.market_watchlist,
            PreferenceSource::OwnerOverride
        );
        assert_eq!(
            projection.weather.saved_locations,
            vec!["東京", "taipei, taiwan", "Osaka"]
        );
    }

    #[test]
    fn weather_defaults_prepend_default_and_dedup_case_insensitively_without_comma_split() {
        let projection = parse(&valid_document()).expect("valid");
        assert_eq!(
            projection.weather_default_locations(),
            vec!["Taipei, Taiwan", "東京", "Osaka"]
        );
    }

    #[test]
    fn weather_defaults_skip_empty_default_location() {
        let mut document = valid_document();
        document["weather"]["default_location"] = json!("");
        let projection = parse(&document).expect("valid");
        assert_eq!(
            projection.weather_default_locations(),
            vec!["東京", "taipei, taiwan", "Osaka"]
        );

        document["weather"]["saved_locations"] = json!([]);
        let projection = parse(&document).expect("valid");
        assert!(projection.weather_default_locations().is_empty());
    }

    #[test]
    fn accepts_null_revision_with_null_digest() {
        let mut document = valid_document();
        document["revision"] = Value::Null;
        document["digest"] = Value::Null;
        let projection = parse(&document).expect("valid");
        assert_eq!(projection.revision, None);
        assert_eq!(projection.digest, None);
    }

    #[test]
    fn rejects_revision_digest_nullness_mismatch() {
        let mut document = valid_document();
        document["digest"] = Value::Null;
        assert_eq!(invalid_field(&document), "digest");

        let mut document = valid_document();
        document["revision"] = Value::Null;
        assert_eq!(invalid_field(&document), "digest");
    }

    #[test]
    fn rejects_bad_revision_and_digest_shapes() {
        for revision in [json!(0), json!(-1), json!(1.5), json!("2")] {
            let mut document = valid_document();
            document["revision"] = revision;
            assert_eq!(invalid_field(&document), "revision");
        }
        for digest in [json!("sha256:abc"), json!("md5:00"), json!(7)] {
            let mut document = valid_document();
            document["digest"] = digest;
            assert_eq!(invalid_field(&document), "digest");
        }
    }

    #[test]
    fn rejects_wrong_schema() {
        let mut document = valid_document();
        document["schema"] = json!("sympoies.alfred-preference-projection/v2");
        assert_eq!(invalid_field(&document), "schema");
    }

    #[test]
    fn rejects_extra_and_missing_fields_at_every_level() {
        let mut document = valid_document();
        document["extra"] = json!(true);
        assert_eq!(invalid_field(&document), "(root)");

        let mut document = valid_document();
        document.as_object_mut().expect("object").remove("sources");
        assert_eq!(invalid_field(&document), "(root)");

        let mut document = valid_document();
        document["market"]["extra"] = json!([]);
        assert_eq!(invalid_field(&document), "market");

        let mut document = valid_document();
        document["weather"]["extra"] = json!("x");
        assert_eq!(invalid_field(&document), "weather");

        let mut document = valid_document();
        document["sources"]["market.extra"] = json!("profile");
        assert_eq!(invalid_field(&document), "sources");
    }

    #[test]
    fn rejects_unknown_source_value() {
        let mut document = valid_document();
        document["sources"]["weather.saved_locations"] = json!("manual");
        assert_eq!(invalid_field(&document), "weather.saved_locations");
    }

    #[test]
    fn rejects_bad_generated_at_formats() {
        for raw in [
            "2026-03-01T11:48:00+00:00",
            "2026-03-01 11:48:00Z",
            "2026-03-01T11:48:00.000Z",
            "2026-13-01T11:48:00Z",
        ] {
            let mut document = valid_document();
            document["generatedAt"] = json!(raw);
            assert_eq!(invalid_field(&document), "generatedAt");
        }
    }

    #[test]
    fn rejects_duplicate_empty_and_oversized_list_entries() {
        let mut document = valid_document();
        document["market"]["watchlist"] = json!(["BTC", "BTC"]);
        assert_eq!(invalid_field(&document), "market.watchlist");

        let mut document = valid_document();
        document["market"]["watchlist"] = json!([""]);
        assert_eq!(invalid_field(&document), "market.watchlist");

        let mut document = valid_document();
        document["market"]["watchlist"] = json!(["ABCDEFGHIJKLMNOPQ"]);
        assert_eq!(invalid_field(&document), "market.watchlist");

        let mut document = valid_document();
        let many: Vec<String> = (0..33).map(|index| format!("L{index}")).collect();
        document["weather"]["saved_locations"] = json!(many);
        assert_eq!(invalid_field(&document), "weather.saved_locations");

        let mut document = valid_document();
        document["weather"]["saved_locations"] = json!(["x".repeat(129)]);
        assert_eq!(invalid_field(&document), "weather.saved_locations");
    }

    #[test]
    fn accepts_list_boundaries_and_case_variant_entries() {
        let mut document = valid_document();
        let many: Vec<String> = (0..32).map(|index| format!("L{index}")).collect();
        document["weather"]["saved_locations"] = json!(many);
        document["weather"]["default_location"] = json!("東".repeat(128));
        document["market"]["watchlist"] = json!(["btc", "BTC", "ABCDEFGHIJKLMNOP"]);
        let projection = parse(&document).expect("valid");
        assert_eq!(projection.weather.saved_locations.len(), 32);
        assert_eq!(projection.market.watchlist.len(), 3);
    }

    #[test]
    fn rejects_whitespace_padding_and_control_or_format_characters() {
        for label in [
            " Tokyo",
            "Tokyo ",
            "To\nkyo",
            "To\u{0007}kyo",
            "To\u{200B}kyo",
            "To\u{202E}kyo",
            "To\u{2028}kyo",
            "To\u{2029}kyo",
            "To\u{FEFF}kyo",
        ] {
            let mut document = valid_document();
            document["weather"]["default_location"] = json!(label);
            assert_eq!(invalid_field(&document), "weather.default_location");
        }
    }

    #[test]
    fn rejects_malformed_and_non_object_documents() {
        assert_eq!(
            parse_preference_projection(b"{not json"),
            Err(ProjectionError::Malformed)
        );
        assert_eq!(
            parse_preference_projection(&[0xff, 0xfe]),
            Err(ProjectionError::Malformed)
        );
        assert_eq!(
            parse_preference_projection(b"[]"),
            Err(ProjectionError::Invalid { field: "(root)" })
        );
    }

    #[test]
    fn load_reports_missing_oversize_and_directory_inputs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.json");
        assert_eq!(
            load_preference_projection(&missing, now()),
            Err(ProjectionError::Unavailable)
        );
        assert_eq!(
            load_preference_projection(dir.path(), now()),
            Err(ProjectionError::Unavailable)
        );

        let oversize = dir.path().join("oversize.json");
        fs::write(&oversize, vec![b' '; 16_385]).expect("write");
        assert_eq!(
            load_preference_projection(&oversize, now()),
            Err(ProjectionError::TooLarge)
        );
    }

    #[test]
    fn load_accepts_fresh_file_and_classifies_stale_and_future_timestamps() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("projection.json");

        fs::write(&path, valid_document().to_string()).expect("write");
        let projection = load_preference_projection(&path, now()).expect("fresh");
        assert_eq!(projection.revision, Some(2));

        let mut document = valid_document();
        document["generatedAt"] = json!("2026-02-22T11:59:59Z");
        fs::write(&path, document.to_string()).expect("write");
        let error = load_preference_projection(&path, now()).expect_err("stale");
        assert!(matches!(error, ProjectionError::Stale { .. }));
        assert_eq!(error.state(), ProjectionState::Stale);

        document["generatedAt"] = json!("2026-03-01T12:04:00Z");
        fs::write(&path, document.to_string()).expect("write");
        assert!(load_preference_projection(&path, now()).is_ok());

        document["generatedAt"] = json!("2026-03-01T12:06:00Z");
        fs::write(&path, document.to_string()).expect("write");
        assert_eq!(
            load_preference_projection(&path, now()),
            Err(ProjectionError::FutureTimestamp)
        );
    }

    #[test]
    fn errors_never_echo_values_or_paths() {
        let mut document = valid_document();
        document["weather"]["default_location"] = json!(" Secret City");
        let message = parse(&document).expect_err("invalid").to_string();
        assert!(!message.contains("Secret"));
        assert_eq!(
            message,
            "projection field failed validation: weather.default_location"
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("private-projection.json");
        let message = load_preference_projection(&missing, now())
            .expect_err("missing")
            .to_string();
        assert!(!message.contains("private"));
    }

    #[test]
    fn status_titles_match_contract_wording() {
        let generated_at = Utc
            .with_ymd_and_hms(2026, 3, 1, 11, 48, 0)
            .single()
            .expect("time");
        let used = ProjectionStatus::Used {
            revision: Some(2),
            generated_at,
            skipped: 0,
        };
        assert_eq!(
            used.title(now()),
            "Preferences: projection revision 2 · synced 12m ago"
        );
        assert_eq!(used.state(), "used");

        let unrevisioned = ProjectionStatus::Used {
            revision: None,
            generated_at,
            skipped: 2,
        };
        assert_eq!(
            unrevisioned.title(now()),
            "Preferences: projection (no revision) · synced 12m ago"
        );
        assert_eq!(
            unrevisioned.subtitle("Hint."),
            "Hint. 2 entries skipped (unsupported)."
        );

        for (error, state) in [
            (ProjectionError::Unavailable, "unavailable"),
            (ProjectionError::Stale { age_secs: 900_000 }, "stale"),
            (ProjectionError::Malformed, "invalid"),
            (ProjectionError::TooLarge, "invalid"),
            (ProjectionError::FutureTimestamp, "invalid"),
            (ProjectionError::Invalid { field: "schema" }, "invalid"),
        ] {
            let status = ProjectionStatus::Failed(error);
            assert_eq!(
                status.title(now()),
                format!("Preferences: projection {state} — using workflow settings")
            );
            assert_eq!(status.state(), state);
        }

        let empty = ProjectionStatus::Empty {
            revision: Some(3),
            skipped: 1,
        };
        assert_eq!(
            empty.title(now()),
            "Preferences: projection revision 3 has no usable entries — using workflow settings"
        );
    }

    #[test]
    fn status_item_is_not_selectable() {
        let item = ProjectionStatus::Failed(ProjectionError::Unavailable).to_item(now(), "unused");
        let json = serde_json::to_value(&item).expect("json");
        assert_eq!(json.get("valid"), Some(&Value::Bool(false)));
        assert!(json.get("arg").is_none());
    }

    #[test]
    fn format_age_buckets() {
        assert_eq!(format_age(-30), "just now");
        assert_eq!(format_age(59), "just now");
        assert_eq!(format_age(60), "1m ago");
        assert_eq!(format_age(3_599), "59m ago");
        assert_eq!(format_age(3_600), "1h ago");
        assert_eq!(format_age(47 * 3_600), "47h ago");
        assert_eq!(format_age(3 * 86_400), "3d ago");
    }
}
