use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use workflow_common::http::build_blocking_client;

use crate::auth::account::resolve_account;
use crate::auth::config::{AuthPaths, load_credentials, load_metadata};
use crate::auth::oauth;
use crate::auth::store::{load_token, persist_token};
use crate::cmd::common::GlobalOptions;
use crate::error::{AppError, redact_sensitive};

const CALENDAR_API_BASE: &str = "https://www.googleapis.com/calendar/v3";
// Bound Calendar calls so a stalled server cannot hang the CLI indefinitely.
const CALENDAR_TIMEOUT: Duration = Duration::from_secs(15);
const GOOGLE_CLI_CALENDAR_FIXTURE_PATH_ENV: &str = "GOOGLE_CLI_CALENDAR_FIXTURE_PATH";
const GOOGLE_CLI_CALENDAR_FIXTURE_JSON_ENV: &str = "GOOGLE_CLI_CALENDAR_FIXTURE_JSON";

/// A `--start` / `--end` value: either a whole-day date or a wall-clock instant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeSpec {
    /// `YYYY-MM-DD`, used for all-day events.
    Date(String),
    /// A normalized `YYYY-MM-DDTHH:MM:SS[±HH:MM|Z]`, seconds always present.
    DateTime { value: String, has_offset: bool },
}

impl TimeSpec {
    pub fn parse(input: &str) -> Result<Self, AppError> {
        if is_date(input) {
            return Ok(Self::Date(input.to_string()));
        }

        let invalid = || {
            AppError::invalid_calendar_input(format!(
                "invalid time value `{input}`; expected `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM[:SS][±HH:MM|Z]`"
            ))
        };

        let Some((date, rest)) = input.split_once('T') else {
            return Err(invalid());
        };
        if !is_date(date) || rest.is_empty() {
            return Err(invalid());
        }

        let (clock, suffix) = split_offset(rest);
        // Calendar v3 requires a complete RFC3339 `dateTime`; `HH:MM` alone is
        // rejected with HTTP 400, so seconds are filled in here rather than
        // being forwarded as the caller typed them.
        let clock = normalize_clock(clock).ok_or_else(invalid)?;

        Ok(Self::DateTime {
            value: format!("{date}T{clock}{suffix}"),
            has_offset: !suffix.is_empty(),
        })
    }

    pub fn is_date(&self) -> bool {
        matches!(self, Self::Date(_))
    }
}

fn is_date(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

/// Split a time part into its clock and its trailing UTC offset (`""` when the
/// value is a bare wall clock).
fn split_offset(time_part: &str) -> (&str, &str) {
    if let Some(stripped) = time_part.strip_suffix(['Z', 'z']) {
        return (stripped, "Z");
    }
    if let Some(index) = time_part.rfind(['+', '-']) {
        return time_part.split_at(index);
    }
    (time_part, "")
}

/// Accept `HH:MM` or `HH:MM:SS` and always return `HH:MM:SS`.
fn normalize_clock(clock: &str) -> Option<String> {
    let mut parts = clock.split(':');
    let hour = two_digits(parts.next()?)?;
    let minute = two_digits(parts.next()?)?;
    let second = match parts.next() {
        Some(value) => two_digits(value)?,
        None => 0,
    };
    if parts.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some(format!("{hour:02}:{minute:02}:{second:02}"))
}

fn two_digits(value: &str) -> Option<u32> {
    if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<u32>().ok()
}

/// Advance a `YYYY-MM-DD` date by one day. The Calendar API treats an all-day
/// `end.date` as exclusive, so an omitted `--end` becomes start + 1 day.
pub fn next_day(date: &str) -> Result<String, AppError> {
    let invalid = || {
        AppError::invalid_calendar_input(format!(
            "invalid all-day date `{date}`; expected YYYY-MM-DD"
        ))
    };
    if !is_date(date) {
        return Err(invalid());
    }
    let year: i32 = date[0..4].parse().map_err(|_| invalid())?;
    let month: u32 = date[5..7].parse().map_err(|_| invalid())?;
    let day: u32 = date[8..10].parse().map_err(|_| invalid())?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return Err(invalid());
    }

    let (next_year, next_month, next_day) = if day < days_in_month(year, month) {
        (year, month, day + 1)
    } else if month < 12 {
        (year, month + 1, 1)
    } else {
        (year + 1, 1, 1)
    };
    Ok(format!("{next_year:04}-{next_month:02}-{next_day:02}"))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalendarView {
    pub id: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub time_zone: String,
    #[serde(default)]
    pub access_role: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventView {
    pub id: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub end: String,
    #[serde(default)]
    pub all_day: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub location: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub html_link: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attendees: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub private_properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CalendarFixtureStore {
    #[serde(default)]
    pub calendars: Vec<CalendarView>,
    #[serde(default)]
    pub events: Vec<EventView>,
}

#[derive(Debug, Clone)]
pub struct EventListRequest {
    pub calendar_id: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub max: usize,
    pub query: Option<String>,
    pub private_properties: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct EventGetRequest {
    pub calendar_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone)]
pub struct EventCreateRequest {
    pub calendar_id: String,
    pub summary: String,
    pub start: TimeSpec,
    pub end: Option<TimeSpec>,
    pub time_zone: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub attendees: Vec<String>,
    pub private_properties: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct CalendarSession {
    pub account: String,
    pub account_source: String,
    pub access_token: String,
    client: Client,
    fixture: Option<CalendarFixtureStore>,
}

impl CalendarSession {
    pub fn from_global(global: &GlobalOptions) -> Result<Self, AppError> {
        let paths = AuthPaths::resolve()?;
        let metadata = load_metadata(&paths)?;
        let resolved = resolve_account(global.account.as_deref(), &metadata)?;
        let token = load_token(&paths, &resolved.account)?.ok_or_else(|| {
            AppError::invalid_calendar_input(format!(
                "account `{}` has no token; run `auth add {}` first",
                resolved.account, resolved.account
            ))
        })?;

        let fixture = load_fixture_store()?;
        let active_token = if fixture.is_some() {
            token
        } else {
            let credentials = load_credentials(&paths)?.ok_or_else(|| {
                AppError::invalid_calendar_input(
                    "OAuth credentials are not configured; run `auth credentials set --client-id <id> --client-secret <secret>` first",
                )
            })?;

            let refreshed = oauth::refresh_access_token(&resolved.account, &credentials, &token)
                .map_err(|error| {
                    AppError::calendar_failure(format!(
                        "failed to refresh OAuth token for `{}`: {}",
                        resolved.account,
                        error.message()
                    ))
                })?;

            if refreshed != token {
                persist_token(&paths, &resolved.account, &refreshed).map_err(|error| {
                    AppError::calendar_failure(format!(
                        "failed to persist refreshed OAuth token for `{}`: {}",
                        resolved.account,
                        error.message()
                    ))
                })?;
            }
            refreshed
        };

        let client = build_blocking_client(None, Some(CALENDAR_TIMEOUT)).map_err(|error| {
            AppError::calendar_failure(format!("failed to build Calendar HTTP client: {error}"))
        })?;

        Ok(Self {
            account: resolved.account,
            account_source: resolved.source.as_str().to_string(),
            access_token: active_token.access_token,
            client,
            fixture,
        })
    }

    pub fn is_fixture_mode(&self) -> bool {
        self.fixture.is_some()
    }

    pub fn list_calendars(&self, max: usize) -> Result<Vec<CalendarView>, AppError> {
        if let Some(fixture) = &self.fixture {
            let mut items = fixture.calendars.clone();
            items.truncate(max);
            return Ok(items);
        }

        let response = self.get_json(
            "users/me/calendarList",
            &[("maxResults", max.to_string())],
            None,
        )?;
        Ok(response
            .get("items")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(calendar_view_from_json).collect())
            .unwrap_or_default())
    }

    pub fn list_events(&self, request: &EventListRequest) -> Result<Vec<EventView>, AppError> {
        if let Some(fixture) = &self.fixture {
            let mut items = fixture
                .events
                .iter()
                .filter(|event| {
                    request.private_properties.iter().all(|(key, value)| {
                        event.private_properties.get(key).map(String::as_str)
                            == Some(value.as_str())
                    })
                })
                .filter(|event| {
                    request.query.as_ref().is_none_or(|needle| {
                        event.summary.contains(needle)
                            || event.description.contains(needle)
                            || event.location.contains(needle)
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            items.truncate(request.max);
            return Ok(items);
        }

        let mut query = vec![
            ("maxResults", request.max.to_string()),
            // Expand recurrence so a chat-facing caller sees concrete instances,
            // and order by start time (which the API allows only with singleEvents).
            ("singleEvents", "true".to_string()),
            ("orderBy", "startTime".to_string()),
        ];
        if let Some(from) = &request.from {
            query.push(("timeMin", from.clone()));
        }
        if let Some(to) = &request.to {
            query.push(("timeMax", to.clone()));
        }
        if let Some(needle) = &request.query {
            query.push(("q", needle.clone()));
        }
        for (key, value) in &request.private_properties {
            query.push(("privateExtendedProperty", format!("{key}={value}")));
        }

        let path = format!("calendars/{}/events", encode_path(&request.calendar_id));
        let response = self.get_json(
            &path,
            &query,
            Some(("calendar", request.calendar_id.as_str())),
        )?;
        Ok(response
            .get("items")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(event_view_from_json).collect())
            .unwrap_or_default())
    }

    pub fn get_event(&self, request: &EventGetRequest) -> Result<EventView, AppError> {
        if let Some(fixture) = &self.fixture {
            return fixture
                .events
                .iter()
                .find(|event| event.id == request.event_id)
                .cloned()
                .ok_or_else(|| AppError::calendar_not_found("event", &request.event_id));
        }

        let path = format!(
            "calendars/{}/events/{}",
            encode_path(&request.calendar_id),
            encode_path(&request.event_id)
        );
        let response = self.get_json(&path, &[], Some(("event", request.event_id.as_str())))?;
        Ok(event_view_from_json(&response))
    }

    pub fn create_event(&self, request: &EventCreateRequest) -> Result<EventView, AppError> {
        let payload = build_event_payload(request)?;

        if self.fixture.is_some() {
            // Fixture mode never mutates remote state; echo the request back so
            // command wiring stays testable without a network call.
            let mut view = event_view_from_json(&payload);
            view.id = "fixture-event".to_string();
            view.status = "confirmed".to_string();
            return Ok(view);
        }

        let path = format!("calendars/{}/events", encode_path(&request.calendar_id));
        let response = self.post_json(
            &path,
            payload,
            Some(("calendar", request.calendar_id.as_str())),
        )?;
        Ok(event_view_from_json(&response))
    }

    fn get_json(
        &self,
        path: &str,
        query: &[(&str, String)],
        not_found: Option<(&str, &str)>,
    ) -> Result<Value, AppError> {
        let url = format!("{CALENDAR_API_BASE}/{path}");
        let response = bounded(
            self.client
                .get(&url)
                .bearer_auth(&self.access_token)
                .query(query),
        )
        .send()
        .map_err(|error| AppError::calendar_failure(format!("GET {url} failed: {error}")))?;
        parse_calendar_response(response, format!("GET {path}").as_str(), not_found)
    }

    fn post_json(
        &self,
        path: &str,
        payload: Value,
        not_found: Option<(&str, &str)>,
    ) -> Result<Value, AppError> {
        let url = format!("{CALENDAR_API_BASE}/{path}");
        let response = bounded(
            self.client
                .post(&url)
                .bearer_auth(&self.access_token)
                .json(&payload),
        )
        .send()
        .map_err(|error| AppError::calendar_failure(format!("POST {url} failed: {error}")))?;
        parse_calendar_response(response, format!("POST {path}").as_str(), not_found)
    }
}

pub fn build_event_payload(request: &EventCreateRequest) -> Result<Value, AppError> {
    if request.summary.trim().is_empty() {
        return Err(AppError::invalid_calendar_input(
            "`events create` requires a non-empty --summary",
        ));
    }

    let all_day = request.start.is_date();
    if let Some(end) = &request.end
        && end.is_date() != all_day
    {
        return Err(AppError::invalid_calendar_input(
            "--start and --end must both be dates (all-day) or both be date-times",
        ));
    }

    let (start, end) = match (&request.start, &request.end) {
        (TimeSpec::Date(start), Some(TimeSpec::Date(end))) => {
            if end <= start {
                return Err(AppError::invalid_calendar_input(format!(
                    "all-day --end `{end}` must be after --start `{start}`; the Calendar API treats end.date as exclusive"
                )));
            }
            (json!({ "date": start }), json!({ "date": end }))
        }
        // The Calendar API treats an all-day end.date as exclusive, so a
        // single-day event ends on the following date.
        (TimeSpec::Date(start), None) => (
            json!({ "date": start }),
            json!({ "date": next_day(start)? }),
        ),
        (
            TimeSpec::DateTime {
                value: start,
                has_offset: start_offset,
            },
            end,
        ) => {
            let time_zone = match (&request.time_zone, start_offset) {
                (Some(zone), _) => Some(zone.clone()),
                (None, true) => None,
                (None, false) => {
                    return Err(AppError::invalid_calendar_input(
                        "--start has no UTC offset; pass --time-zone <IANA zone> or include an offset such as `+08:00`",
                    ));
                }
            };
            let Some(TimeSpec::DateTime { value: end, .. }) = end else {
                return Err(AppError::invalid_calendar_input(
                    "a timed event requires --end",
                ));
            };
            if end <= start {
                return Err(AppError::invalid_calendar_input(format!(
                    "--end `{end}` must be after --start `{start}`"
                )));
            }
            let mut start_value = json!({ "dateTime": start });
            let mut end_value = json!({ "dateTime": end });
            if let Some(zone) = time_zone {
                start_value["timeZone"] = json!(zone);
                end_value["timeZone"] = json!(zone);
            }
            (start_value, end_value)
        }
        // The all-day/timed guard above already rejects this pairing. Return an
        // error rather than panicking so every failure still renders an envelope.
        (TimeSpec::Date(_), Some(TimeSpec::DateTime { .. })) => {
            return Err(AppError::invalid_calendar_input(
                "--start and --end must both be dates (all-day) or both be date-times",
            ));
        }
    };

    let mut payload = json!({
        "summary": request.summary,
        "start": start,
        "end": end,
    });

    if let Some(location) = &request.location {
        payload["location"] = json!(location);
    }
    if let Some(description) = &request.description {
        payload["description"] = json!(description);
    }
    if !request.attendees.is_empty() {
        payload["attendees"] = Value::Array(
            request
                .attendees
                .iter()
                .map(|email| json!({ "email": email }))
                .collect(),
        );
    }
    if !request.private_properties.is_empty() {
        let private = request
            .private_properties
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect::<serde_json::Map<_, _>>();
        payload["extendedProperties"] = json!({ "private": private });
    }

    Ok(payload)
}

fn bounded(request: RequestBuilder) -> RequestBuilder {
    request.timeout(CALENDAR_TIMEOUT)
}

fn encode_path(value: &str) -> String {
    // Calendar and event ids appear in the path; `@` and `/` must not be taken
    // as path structure.
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn calendar_view_from_json(value: &Value) -> CalendarView {
    CalendarView {
        id: string_field(value, "id"),
        summary: string_field(value, "summary"),
        time_zone: string_field(value, "timeZone"),
        access_role: string_field(value, "accessRole"),
        primary: value
            .get("primary")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn event_view_from_json(value: &Value) -> EventView {
    let start = value.get("start");
    let end = value.get("end");
    let all_day = start.is_some_and(|slot| slot.get("date").is_some());

    EventView {
        id: string_field(value, "id"),
        summary: string_field(value, "summary"),
        status: string_field(value, "status"),
        start: time_slot(start),
        end: time_slot(end),
        all_day,
        location: string_field(value, "location"),
        description: string_field(value, "description"),
        html_link: string_field(value, "htmlLink"),
        attendees: value
            .get("attendees")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("email").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        private_properties: value
            .get("extendedProperties")
            .and_then(|extended| extended.get("private"))
            .and_then(Value::as_object)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn time_slot(slot: Option<&Value>) -> String {
    slot.and_then(|value| {
        value
            .get("dateTime")
            .or_else(|| value.get("date"))
            .and_then(Value::as_str)
    })
    .unwrap_or_default()
    .to_string()
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn parse_calendar_response(
    response: Response,
    context: &str,
    not_found: Option<(&str, &str)>,
) -> Result<Value, AppError> {
    let status = response.status();
    let body = response.text().map_err(|error| {
        AppError::calendar_failure(format!("{context} failed reading body: {error}"))
    })?;

    if status.as_u16() == 404
        && let Some((entity, id)) = not_found
    {
        return Err(AppError::calendar_not_found(entity, id));
    }

    if !status.is_success() {
        let detail = extract_error_message(&body).unwrap_or(body);
        return Err(AppError::calendar_failure(format!(
            "{context} failed with HTTP {}: {}",
            status.as_u16(),
            redact_sensitive(&detail)
        )));
    }

    if body.trim().is_empty() {
        return Ok(Value::Null);
    }

    serde_json::from_str::<Value>(&body).map_err(|error| {
        AppError::calendar_failure(format!("{context} returned invalid JSON: {error}"))
    })
}

fn extract_error_message(body: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    let message = parsed
        .get("error")
        .and_then(Value::as_object)
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
        .or_else(|| parsed.get("error_description").and_then(Value::as_str))
        .or_else(|| parsed.get("error").and_then(Value::as_str))?;
    Some(message.to_string())
}

fn load_fixture_store() -> Result<Option<CalendarFixtureStore>, AppError> {
    if let Ok(inline) = env::var(GOOGLE_CLI_CALENDAR_FIXTURE_JSON_ENV)
        && !inline.trim().is_empty()
    {
        return serde_json::from_str(&inline).map(Some).map_err(|error| {
            AppError::calendar_failure(format!(
                "invalid {GOOGLE_CLI_CALENDAR_FIXTURE_JSON_ENV}: {error}"
            ))
        });
    }

    if let Ok(path) = env::var(GOOGLE_CLI_CALENDAR_FIXTURE_PATH_ENV)
        && !path.trim().is_empty()
    {
        let raw = fs::read_to_string(&path).map_err(|error| {
            AppError::calendar_failure(format!("failed to read calendar fixture `{path}`: {error}"))
        })?;
        return serde_json::from_str(&raw).map(Some).map_err(|error| {
            AppError::calendar_failure(format!("invalid calendar fixture `{path}`: {error}"))
        });
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_date_and_date_time_values() {
        assert_eq!(
            TimeSpec::parse("2026-08-15").expect("date"),
            TimeSpec::Date("2026-08-15".to_string())
        );
        assert_eq!(
            TimeSpec::parse("2026-08-15T11:20:30").expect("naive with seconds"),
            TimeSpec::DateTime {
                value: "2026-08-15T11:20:30".to_string(),
                has_offset: false,
            }
        );
        assert_eq!(
            TimeSpec::parse("2026-08-15T11:20:00+08:00").expect("offset"),
            TimeSpec::DateTime {
                value: "2026-08-15T11:20:00+08:00".to_string(),
                has_offset: true,
            }
        );
        assert_eq!(
            TimeSpec::parse("2026-08-15T03:20:00Z").expect("utc"),
            TimeSpec::DateTime {
                value: "2026-08-15T03:20:00Z".to_string(),
                has_offset: true,
            }
        );
        assert!(TimeSpec::parse("15/08/2026").is_err());
    }

    /// Calendar v3 rejects an `HH:MM` `dateTime` with HTTP 400, so a minute-only
    /// value must gain seconds before it is sent.
    #[test]
    fn fills_in_missing_seconds_for_rfc3339() {
        assert_eq!(
            TimeSpec::parse("2026-08-15T11:20").expect("naive"),
            TimeSpec::DateTime {
                value: "2026-08-15T11:20:00".to_string(),
                has_offset: false,
            }
        );
        assert_eq!(
            TimeSpec::parse("2026-08-15T11:20+08:00").expect("offset"),
            TimeSpec::DateTime {
                value: "2026-08-15T11:20:00+08:00".to_string(),
                has_offset: true,
            }
        );
        assert_eq!(
            TimeSpec::parse("2026-08-15T11:20Z").expect("utc"),
            TimeSpec::DateTime {
                value: "2026-08-15T11:20:00Z".to_string(),
                has_offset: true,
            }
        );
    }

    #[test]
    fn rejects_malformed_clock_values() {
        for input in [
            "2026-08-15T1:20",
            "2026-08-15T11",
            "2026-08-15T24:00",
            "2026-08-15T11:60",
            "2026-08-15T11:20:00:00",
            "2026-08-15Tzz:20",
            "2026-08-15T",
        ] {
            assert!(
                TimeSpec::parse(input).is_err(),
                "`{input}` should be rejected before it reaches the API"
            );
        }
    }

    #[test]
    fn next_day_crosses_month_and_leap_year_boundaries() {
        assert_eq!(next_day("2026-08-15").expect("mid-month"), "2026-08-16");
        assert_eq!(next_day("2026-08-31").expect("month end"), "2026-09-01");
        assert_eq!(next_day("2026-12-31").expect("year end"), "2027-01-01");
        assert_eq!(next_day("2028-02-28").expect("leap"), "2028-02-29");
        assert_eq!(next_day("2026-02-28").expect("non-leap"), "2026-03-01");
        assert!(next_day("2026-02-30").is_err());
    }

    #[test]
    fn all_day_payload_defaults_to_exclusive_next_day_end() {
        let payload = build_event_payload(&EventCreateRequest {
            calendar_id: "primary".to_string(),
            summary: "Trip".to_string(),
            start: TimeSpec::parse("2026-08-15").expect("start"),
            end: None,
            time_zone: None,
            location: None,
            description: None,
            attendees: Vec::new(),
            private_properties: Vec::new(),
        })
        .expect("payload");

        assert_eq!(payload["start"]["date"], "2026-08-15");
        assert_eq!(payload["end"]["date"], "2026-08-16");
    }

    #[test]
    fn timed_payload_requires_a_zone_when_the_start_has_no_offset() {
        let error = build_event_payload(&EventCreateRequest {
            calendar_id: "primary".to_string(),
            summary: "Hotpot".to_string(),
            start: TimeSpec::parse("2026-08-15T11:20").expect("start"),
            end: Some(TimeSpec::parse("2026-08-15T13:00").expect("end")),
            time_zone: None,
            location: None,
            description: None,
            attendees: Vec::new(),
            private_properties: Vec::new(),
        })
        .expect_err("missing zone");

        assert_eq!(
            error.code(),
            crate::error::ERROR_CODE_USER_CALENDAR_INVALID_INPUT
        );
    }

    #[test]
    fn timed_payload_carries_zone_and_private_properties() {
        let payload = build_event_payload(&EventCreateRequest {
            calendar_id: "primary".to_string(),
            summary: "全家去吃千葉火鍋".to_string(),
            start: TimeSpec::parse("2026-08-15T11:20").expect("start"),
            end: Some(TimeSpec::parse("2026-08-15T13:00").expect("end")),
            time_zone: Some("Asia/Taipei".to_string()),
            location: Some("千葉火鍋".to_string()),
            description: None,
            attendees: vec!["someone@example.com".to_string()],
            private_properties: vec![("gn_note_id".to_string(), "n_e923e876".to_string())],
        })
        .expect("payload");

        assert_eq!(payload["start"]["timeZone"], "Asia/Taipei");
        assert_eq!(payload["start"]["dateTime"], "2026-08-15T11:20:00");
        assert_eq!(payload["end"]["dateTime"], "2026-08-15T13:00:00");
        assert_eq!(payload["attendees"][0]["email"], "someone@example.com");
        assert_eq!(
            payload["extendedProperties"]["private"]["gn_note_id"],
            "n_e923e876"
        );
    }

    #[test]
    fn rejects_mixed_date_and_date_time_bounds() {
        let error = build_event_payload(&EventCreateRequest {
            calendar_id: "primary".to_string(),
            summary: "Mixed".to_string(),
            start: TimeSpec::parse("2026-08-15").expect("start"),
            end: Some(TimeSpec::parse("2026-08-15T13:00").expect("end")),
            time_zone: None,
            location: None,
            description: None,
            attendees: Vec::new(),
            private_properties: Vec::new(),
        })
        .expect_err("mixed bounds");

        assert_eq!(
            error.code(),
            crate::error::ERROR_CODE_USER_CALENDAR_INVALID_INPUT
        );
    }

    #[test]
    fn encodes_calendar_ids_for_path_use() {
        assert_eq!(
            encode_path("abc123@group.calendar.google.com"),
            "abc123%40group.calendar.google.com"
        );
    }
}
