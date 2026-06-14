use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use workflow_common::http::build_blocking_client;

use crate::config::{ApiFallbackPolicy, DEFAULT_USER_AGENT, RuntimeConfig};
use crate::input::{ParsedInput, SubjectType};

pub const V0_SEARCH_ENDPOINT: &str = "https://api.bgm.tv/v0/search/subjects";
pub const LEGACY_SEARCH_ENDPOINT: &str = "https://api.bgm.tv/search/subject";

#[derive(Debug, Clone, PartialEq)]
pub struct BangumiSubject {
    pub id: u64,
    pub subject_type: Option<SubjectType>,
    pub name: String,
    pub name_cn: Option<String>,
    pub summary: Option<String>,
    pub url: String,
    pub rank: Option<u32>,
    pub score: Option<f64>,
    pub images: SubjectImages,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubjectImages {
    pub small: Option<String>,
    pub grid: Option<String>,
    pub common: Option<String>,
    pub large: Option<String>,
}

impl SubjectImages {
    pub fn preferred_image_candidate(&self) -> Option<(&'static str, &str)> {
        self.small
            .as_deref()
            .map(|url| ("small", url))
            .or_else(|| self.grid.as_deref().map(|url| ("grid", url)))
            .or_else(|| self.common.as_deref().map(|url| ("common", url)))
            .or_else(|| self.large.as_deref().map(|url| ("large", url)))
    }
}

pub fn canonical_subject_url(subject_id: u64) -> String {
    format!("https://bgm.tv/subject/{subject_id}")
}

pub fn fallback_subject_image_url(subject_id: u64) -> String {
    format!("https://api.bgm.tv/v0/subjects/{subject_id}/image?type=small")
}

pub fn build_headers(config: &RuntimeConfig) -> HeaderMap {
    let mut headers = HeaderMap::new();

    let user_agent = sanitize_header_value(&config.user_agent)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_USER_AGENT.to_string());
    let user_agent_header = HeaderValue::from_str(&user_agent)
        .unwrap_or_else(|_| HeaderValue::from_static(DEFAULT_USER_AGENT));
    headers.insert(USER_AGENT, user_agent_header);

    if let Some(api_key) = config
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let mut authorization = b"Bearer ".to_vec();
        authorization.extend_from_slice(api_key.as_bytes());
        if let Ok(header) = HeaderValue::from_bytes(&authorization) {
            headers.insert(AUTHORIZATION, header);
        }
    }

    headers
}

fn sanitize_header_value(value: &str) -> Option<String> {
    let compact = value
        .chars()
        .filter(|ch| !matches!(ch, '\r' | '\n'))
        .collect::<String>();
    let trimmed = compact.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn search_subjects(
    config: &RuntimeConfig,
    query: &ParsedInput,
) -> Result<Vec<BangumiSubject>, BangumiApiError> {
    let client = build_blocking_client(None, Some(Duration::from_millis(config.timeout_ms)))
        .map_err(|source| BangumiApiError::Transport { source })?;

    search_subjects_with(
        config,
        query,
        |runtime_config, parsed| search_v0_with_client(&client, runtime_config, parsed),
        |runtime_config, parsed| search_legacy_with_client(&client, runtime_config, parsed),
    )
}

pub fn search_subjects_with<PrimarySearch, LegacySearch>(
    config: &RuntimeConfig,
    query: &ParsedInput,
    primary_search: PrimarySearch,
    legacy_search: LegacySearch,
) -> Result<Vec<BangumiSubject>, BangumiApiError>
where
    PrimarySearch: Fn(&RuntimeConfig, &ParsedInput) -> Result<Vec<BangumiSubject>, BangumiApiError>,
    LegacySearch: Fn(&RuntimeConfig, &ParsedInput) -> Result<Vec<BangumiSubject>, BangumiApiError>,
{
    let primary_result = primary_search(config, query);

    match primary_result {
        Ok(subjects) => Ok(subjects),
        Err(primary_error) => {
            if should_try_legacy(config.api_fallback, &primary_error) {
                match legacy_search(config, query) {
                    Ok(subjects) => Ok(subjects),
                    Err(legacy_error) => Err(BangumiApiError::Fallback {
                        primary: Box::new(primary_error),
                        legacy: Box::new(legacy_error),
                    }),
                }
            } else {
                Err(primary_error)
            }
        }
    }
}

pub fn should_try_legacy(policy: ApiFallbackPolicy, error: &BangumiApiError) -> bool {
    match policy {
        ApiFallbackPolicy::Always => true,
        ApiFallbackPolicy::Never => false,
        ApiFallbackPolicy::Auto => match error {
            BangumiApiError::InvalidResponse { .. } => true,
            BangumiApiError::Http { status, .. } => {
                matches!(status, 404 | 405 | 410 | 422 | 500 | 501 | 502 | 503 | 504)
            }
            _ => false,
        },
    }
}

fn search_v0_with_client(
    client: &Client,
    config: &RuntimeConfig,
    query: &ParsedInput,
) -> Result<Vec<BangumiSubject>, BangumiApiError> {
    let limit = config.max_results.clamp(1, 20);
    let response = client
        .post(V0_SEARCH_ENDPOINT)
        .query(&[("limit", limit), ("offset", 0_u8)])
        .headers(build_headers(config))
        .json(&build_v0_request_payload(query))
        .send()
        .map_err(|source| BangumiApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .map_err(|source| BangumiApiError::Transport { source })?;

    parse_v0_search_response(status_code, &body, query.subject_type)
}

fn search_legacy_with_client(
    client: &Client,
    config: &RuntimeConfig,
    query: &ParsedInput,
) -> Result<Vec<BangumiSubject>, BangumiApiError> {
    let url = build_legacy_search_url(query)?;

    let response = client
        .get(url)
        .headers(build_headers(config))
        .send()
        .map_err(|source| BangumiApiError::Transport { source })?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .map_err(|source| BangumiApiError::Transport { source })?;

    parse_legacy_search_response(status_code, &body, query.subject_type)
}

pub fn build_v0_request_payload(query: &ParsedInput) -> Value {
    let mut filter = serde_json::Map::new();
    if let Some(subject_type) = query.subject_type.as_bangumi_type() {
        filter.insert("type".to_string(), serde_json::json!([subject_type]));
    }

    serde_json::json!({
        "keyword": query.keyword,
        "sort": "match",
        "filter": Value::Object(filter),
    })
}

pub fn build_legacy_search_url(query: &ParsedInput) -> Result<reqwest::Url, BangumiApiError> {
    let mut url = reqwest::Url::parse(LEGACY_SEARCH_ENDPOINT).map_err(|source| {
        BangumiApiError::InvalidLegacyUrl {
            keyword: query.keyword.clone(),
            reason: source.to_string(),
        }
    })?;

    {
        let mut path_segments =
            url.path_segments_mut()
                .map_err(|_| BangumiApiError::InvalidLegacyUrl {
                    keyword: query.keyword.clone(),
                    reason: "legacy endpoint base url does not support path segments".to_string(),
                })?;
        path_segments.push(query.keyword.as_str());
    }

    {
        let mut query_pairs = url.query_pairs_mut();
        query_pairs.append_pair("responseGroup", "small");
        if let Some(subject_type) = query.subject_type.as_bangumi_type() {
            query_pairs.append_pair("type", &subject_type.to_string());
        }
    }

    Ok(url)
}

pub fn parse_v0_search_response(
    status_code: u16,
    body: &str,
    query_type: SubjectType,
) -> Result<Vec<BangumiSubject>, BangumiApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(BangumiApiError::Http {
            status: status_code,
            message,
        });
    }

    let payload: V0SearchResponse =
        serde_json::from_str(body).map_err(|source| BangumiApiError::InvalidResponse {
            endpoint: "v0",
            source,
        })?;

    Ok(payload
        .data
        .into_iter()
        .filter_map(|subject| normalize_subject(subject, query_type))
        .collect())
}

pub fn parse_legacy_search_response(
    status_code: u16,
    body: &str,
    query_type: SubjectType,
) -> Result<Vec<BangumiSubject>, BangumiApiError> {
    if !(200..=299).contains(&status_code) {
        let message = extract_error_message(body).unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(BangumiApiError::Http {
            status: status_code,
            message,
        });
    }

    let payload: LegacySearchResponse =
        serde_json::from_str(body).map_err(|source| BangumiApiError::InvalidResponse {
            endpoint: "legacy",
            source,
        })?;

    Ok(payload
        .list
        .into_iter()
        .filter_map(|subject| normalize_subject(subject, query_type))
        .collect())
}

fn normalize_subject(subject: RawSubject, query_type: SubjectType) -> Option<BangumiSubject> {
    if subject.id == 0 {
        return None;
    }

    let name_cn = normalize_optional(subject.name_cn);
    let mut name = normalize_optional(Some(subject.name)).unwrap_or_default();
    if name.is_empty() {
        name = name_cn.clone().unwrap_or_default();
    }
    if name.is_empty() {
        return None;
    }

    let summary = normalize_optional(subject.summary);
    let url = normalize_optional(subject.url).unwrap_or_else(|| canonical_subject_url(subject.id));
    let rating = subject.rating.unwrap_or_default();

    let subject_type = subject
        .subject_type
        .and_then(SubjectType::from_bangumi_type)
        .or(match query_type {
            SubjectType::All => None,
            fixed_type => Some(fixed_type),
        });

    Some(BangumiSubject {
        id: subject.id,
        subject_type,
        name,
        name_cn,
        summary,
        url,
        rank: subject.rank.or(rating.rank),
        score: subject.score.or(rating.score),
        images: subject.images.unwrap_or_default().into_images(),
    })
}

fn normalize_optional(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn extract_error_message(body: &str) -> Option<String> {
    let payload = serde_json::from_str::<Value>(body).ok()?;

    [
        payload.pointer("/error/message"),
        payload.pointer("/title"),
        payload.pointer("/description"),
        payload.pointer("/message"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .map(str::trim)
    .find(|value| !value.is_empty())
    .map(ToOwned::to_owned)
}

#[derive(Debug, Error)]
pub enum BangumiApiError {
    #[error("bangumi api request failed")]
    Transport {
        #[source]
        source: reqwest::Error,
    },
    #[error("bangumi api error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("invalid bangumi api response from {endpoint}")]
    InvalidResponse {
        endpoint: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("legacy endpoint url is invalid for keyword `{keyword}`: {reason}")]
    InvalidLegacyUrl { keyword: String, reason: String },
    #[error("legacy fallback failed after v0 error: primary={primary}; legacy={legacy}")]
    Fallback {
        primary: Box<BangumiApiError>,
        legacy: Box<BangumiApiError>,
    },
}

#[derive(Debug, Deserialize)]
struct V0SearchResponse {
    #[serde(default)]
    data: Vec<RawSubject>,
}

#[derive(Debug, Deserialize)]
struct LegacySearchResponse {
    #[serde(default)]
    list: Vec<RawSubject>,
}

#[derive(Debug, Deserialize)]
struct RawSubject {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    name: String,
    name_cn: Option<String>,
    summary: Option<String>,
    url: Option<String>,
    #[serde(rename = "type")]
    subject_type: Option<u8>,
    rank: Option<u32>,
    score: Option<f64>,
    rating: Option<RawRating>,
    images: Option<RawImages>,
}

#[derive(Debug, Default, Deserialize)]
struct RawRating {
    rank: Option<u32>,
    score: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct RawImages {
    small: Option<String>,
    grid: Option<String>,
    common: Option<String>,
    large: Option<String>,
}

impl RawImages {
    fn into_images(self) -> SubjectImages {
        SubjectImages {
            small: normalize_optional(self.small),
            grid: normalize_optional(self.grid),
            common: normalize_optional(self.common),
            large: normalize_optional(self.large),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_IMAGE_CACHE_MAX_MB;

    fn fixture_config(policy: ApiFallbackPolicy) -> RuntimeConfig {
        RuntimeConfig {
            api_key: Some("demo-key".to_string()),
            max_results: 8,
            timeout_ms: 8_000,
            user_agent: "demo-agent".to_string(),
            cache_dir: std::path::PathBuf::from("/tmp/bangumi-cli-cache"),
            image_cache_ttl_seconds: 60,
            image_cache_max_bytes: DEFAULT_IMAGE_CACHE_MAX_MB * 1024 * 1024,
            api_fallback: policy,
        }
    }

    fn fixture_query(subject_type: SubjectType) -> ParsedInput {
        ParsedInput {
            subject_type,
            keyword: "naruto".to_string(),
        }
    }

    #[test]
    fn bangumi_api_build_v0_payload_uses_keyword_and_type_filter() {
        let payload = build_v0_request_payload(&fixture_query(SubjectType::Anime));

        assert_eq!(
            payload.get("keyword").and_then(Value::as_str),
            Some("naruto")
        );
        assert!(
            payload.get("limit").is_none(),
            "limit must be passed via URL query params for v0 endpoint"
        );
        assert_eq!(
            payload.pointer("/filter/type/0").and_then(Value::as_u64),
            Some(2)
        );
    }

    #[test]
    fn bangumi_api_build_v0_payload_omits_type_filter_for_all_query() {
        let payload = build_v0_request_payload(&fixture_query(SubjectType::All));

        assert!(
            payload
                .pointer("/filter/type")
                .and_then(Value::as_array)
                .is_none(),
            "all queries should not force a type filter"
        );
    }

    #[test]
    fn bangumi_api_parse_v0_response_handles_nullable_fields_and_url_fallback() {
        let body = r#"{
            "data": [
                {
                    "id": 2782,
                    "type": 2,
                    "name": "Cowboy Bebop",
                    "name_cn": null,
                    "summary": "  Space western classic.  ",
                    "url": "",
                    "score": 8.7,
                    "images": {
                        "small": "https://img.example.com/2782-small.jpg"
                    }
                }
            ]
        }"#;

        let subjects =
            parse_v0_search_response(200, body, SubjectType::All).expect("response should parse");
        assert_eq!(subjects.len(), 1);

        let subject = &subjects[0];
        assert_eq!(subject.id, 2782);
        assert_eq!(subject.subject_type, Some(SubjectType::Anime));
        assert_eq!(subject.name, "Cowboy Bebop");
        assert_eq!(subject.summary.as_deref(), Some("Space western classic."));
        assert_eq!(subject.url, "https://bgm.tv/subject/2782");
        assert_eq!(subject.score, Some(8.7));
        assert_eq!(
            subject.images.small.as_deref(),
            Some("https://img.example.com/2782-small.jpg")
        );
    }

    #[test]
    fn bangumi_api_parse_legacy_response_reads_rating_object_when_rank_and_score_missing() {
        let body = r#"{
            "results": 1,
            "list": [
                {
                    "id": 42,
                    "type": 1,
                    "name": "Berserk",
                    "name_cn": "烙印勇士",
                    "summary": "dark fantasy",
                    "rating": {
                        "rank": 3,
                        "score": 9.2
                    },
                    "images": {
                        "grid": "https://img.example.com/42-grid.jpg"
                    }
                }
            ]
        }"#;

        let subjects = parse_legacy_search_response(200, body, SubjectType::Book)
            .expect("legacy response should parse");

        assert_eq!(subjects.len(), 1);
        let subject = &subjects[0];
        assert_eq!(subject.rank, Some(3));
        assert_eq!(subject.score, Some(9.2));
        assert_eq!(subject.subject_type, Some(SubjectType::Book));
        assert_eq!(subject.name_cn.as_deref(), Some("烙印勇士"));
        assert_eq!(
            subject.images.grid.as_deref(),
            Some("https://img.example.com/42-grid.jpg")
        );
    }

    #[test]
    fn bangumi_api_http_error_extracts_service_message() {
        let body = r#"{"error": {"message": "invalid token"}}"#;

        let err = parse_v0_search_response(403, body, SubjectType::Anime)
            .expect_err("non-2xx should fail");

        match err {
            BangumiApiError::Http { status, message } => {
                assert_eq!(status, 403);
                assert_eq!(message, "invalid token");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn auth_header_includes_bearer_and_user_agent_when_api_key_exists() {
        let config = fixture_config(ApiFallbackPolicy::Auto);
        let headers = build_headers(&config);

        assert_eq!(
            headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some("demo-agent")
        );
        let authorization = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .expect("authorization header");
        assert!(authorization.starts_with("Bearer "));
        assert!(authorization.len() > "Bearer ".len());
    }

    #[test]
    fn auth_header_omits_bearer_when_api_key_missing() {
        let mut config = fixture_config(ApiFallbackPolicy::Auto);
        config.api_key = None;

        let headers = build_headers(&config);
        assert!(
            headers.get(AUTHORIZATION).is_none(),
            "authorization should be omitted when api key is absent"
        );
    }

    #[test]
    fn fallback_auto_policy_triggers_legacy_on_invalid_v0_response() {
        let config = fixture_config(ApiFallbackPolicy::Auto);
        let query = fixture_query(SubjectType::Anime);

        let result = search_subjects_with(
            &config,
            &query,
            |_config, _query| {
                Err(BangumiApiError::InvalidResponse {
                    endpoint: "v0",
                    source: serde_json::from_str::<Value>("not-json")
                        .expect_err("fixture should produce parse error"),
                })
            },
            |_config, _query| {
                Ok(vec![BangumiSubject {
                    id: 1,
                    subject_type: Some(SubjectType::Anime),
                    name: "fallback-item".to_string(),
                    name_cn: None,
                    summary: None,
                    url: canonical_subject_url(1),
                    rank: None,
                    score: None,
                    images: SubjectImages::default(),
                }])
            },
        )
        .expect("legacy should be used");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "fallback-item");
    }

    #[test]
    fn fallback_auto_policy_does_not_mask_rate_limit_errors() {
        let config = fixture_config(ApiFallbackPolicy::Auto);
        let query = fixture_query(SubjectType::Anime);

        let err = search_subjects_with(
            &config,
            &query,
            |_config, _query| {
                Err(BangumiApiError::Http {
                    status: 429,
                    message: "too many requests".to_string(),
                })
            },
            |_config, _query| {
                panic!("legacy search must not be called for 429 errors");
            },
        )
        .expect_err("auto fallback should not trigger on 429");

        assert!(matches!(err, BangumiApiError::Http { status: 429, .. }));
    }

    #[test]
    fn fallback_never_policy_disables_legacy_even_for_schema_regression() {
        let config = fixture_config(ApiFallbackPolicy::Never);
        let query = fixture_query(SubjectType::Anime);

        let err = search_subjects_with(
            &config,
            &query,
            |_config, _query| {
                Err(BangumiApiError::InvalidResponse {
                    endpoint: "v0",
                    source: serde_json::from_str::<Value>("not-json")
                        .expect_err("fixture should produce parse error"),
                })
            },
            |_config, _query| {
                panic!("legacy search must not run when fallback policy is never");
            },
        )
        .expect_err("never policy should keep primary failure");

        assert!(matches!(
            err,
            BangumiApiError::InvalidResponse { endpoint: "v0", .. }
        ));
    }

    #[test]
    fn fallback_always_policy_forces_legacy_for_any_primary_error() {
        let config = fixture_config(ApiFallbackPolicy::Always);
        let query = fixture_query(SubjectType::Anime);

        let result = search_subjects_with(
            &config,
            &query,
            |_config, _query| {
                Err(BangumiApiError::Http {
                    status: 429,
                    message: "too many requests".to_string(),
                })
            },
            |_config, _query| Ok(Vec::new()),
        )
        .expect("legacy should be forced");

        assert!(result.is_empty());
    }

    #[test]
    fn fallback_reports_both_primary_and_legacy_errors_when_both_fail() {
        let config = fixture_config(ApiFallbackPolicy::Always);
        let query = fixture_query(SubjectType::Anime);

        let err = search_subjects_with(
            &config,
            &query,
            |_config, _query| {
                Err(BangumiApiError::Http {
                    status: 500,
                    message: "v0 unstable".to_string(),
                })
            },
            |_config, _query| {
                Err(BangumiApiError::Http {
                    status: 503,
                    message: "legacy unavailable".to_string(),
                })
            },
        )
        .expect_err("both errors should surface as fallback error");

        match err {
            BangumiApiError::Fallback { primary, legacy } => {
                assert!(matches!(
                    *primary,
                    BangumiApiError::Http { status: 500, .. }
                ));
                assert!(matches!(*legacy, BangumiApiError::Http { status: 503, .. }));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
