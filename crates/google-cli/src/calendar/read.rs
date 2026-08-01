use serde_json::json;

use crate::error::AppError;

use super::client::{CalendarSession, EventGetRequest, EventListRequest};
use super::{NativeCalendarResponse, response};

const DEFAULT_MAX: usize = 25;

pub fn execute_calendars_list(
    session: &CalendarSession,
    args: &[String],
) -> Result<NativeCalendarResponse, AppError> {
    let mut max = DEFAULT_MAX;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--max" => {
                index += 1;
                max = parse_max(value_for(args, index, "--max")?)?;
            }
            unknown => {
                return Err(AppError::invalid_calendar_input(format!(
                    "unknown calendars list flag `{unknown}`"
                )));
            }
        }
        index += 1;
    }

    let calendars = session.list_calendars(max)?;
    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "max": max,
            "count": calendars.len(),
            "calendars": calendars,
        }),
        format!("Listed {} calendar(s).", calendars.len()),
    ))
}

pub fn execute_events_list(
    session: &CalendarSession,
    args: &[String],
) -> Result<NativeCalendarResponse, AppError> {
    let mut calendar_id = None;
    let mut from = None;
    let mut to = None;
    let mut query = None;
    let mut max = DEFAULT_MAX;
    let mut private_properties = Vec::new();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--calendar-id" => {
                index += 1;
                calendar_id = Some(value_for(args, index, "--calendar-id")?.clone());
            }
            "--from" => {
                index += 1;
                from = Some(require_rfc3339(
                    value_for(args, index, "--from")?,
                    "--from",
                )?);
            }
            "--to" => {
                index += 1;
                to = Some(require_rfc3339(value_for(args, index, "--to")?, "--to")?);
            }
            "--query" => {
                index += 1;
                query = Some(value_for(args, index, "--query")?.clone());
            }
            "--max" => {
                index += 1;
                max = parse_max(value_for(args, index, "--max")?)?;
            }
            "--private-property" => {
                index += 1;
                private_properties.push(parse_property(value_for(
                    args,
                    index,
                    "--private-property",
                )?)?);
            }
            unknown => {
                return Err(AppError::invalid_calendar_input(format!(
                    "unknown events list flag `{unknown}`"
                )));
            }
        }
        index += 1;
    }

    let request = EventListRequest {
        calendar_id: require_calendar_id(calendar_id)?,
        from,
        to,
        max,
        query,
        private_properties,
    };
    let events = session.list_events(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "calendar_id": request.calendar_id,
            "from": request.from,
            "to": request.to,
            "query": request.query,
            "max": request.max,
            "count": events.len(),
            "events": events,
        }),
        format!(
            "Found {} event(s) in `{}`.",
            events.len(),
            request.calendar_id
        ),
    ))
}

pub fn execute_events_get(
    session: &CalendarSession,
    args: &[String],
) -> Result<NativeCalendarResponse, AppError> {
    let mut calendar_id = None;
    let mut event_id = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--calendar-id" => {
                index += 1;
                calendar_id = Some(value_for(args, index, "--calendar-id")?.clone());
            }
            "--event-id" => {
                index += 1;
                event_id = Some(value_for(args, index, "--event-id")?.clone());
            }
            unknown if unknown.starts_with('-') => {
                return Err(AppError::invalid_calendar_input(format!(
                    "unknown events get flag `{unknown}`"
                )));
            }
            positional => {
                if event_id.is_some() {
                    return Err(AppError::invalid_calendar_input(format!(
                        "unexpected extra event id `{positional}`"
                    )));
                }
                event_id = Some(positional.to_string());
            }
        }
        index += 1;
    }

    let request = EventGetRequest {
        calendar_id: require_calendar_id(calendar_id)?,
        event_id: event_id.ok_or_else(|| {
            AppError::invalid_calendar_input(
                "`events get` requires an event id, either positional or via --event-id",
            )
        })?,
    };
    let event = session.get_event(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "calendar_id": request.calendar_id,
            "event": event,
        }),
        format!("Fetched event `{}`.", request.event_id),
    ))
}

pub(super) fn value_for<'a>(
    args: &'a [String],
    index: usize,
    flag: &str,
) -> Result<&'a String, AppError> {
    args.get(index)
        .ok_or_else(|| AppError::invalid_calendar_input(format!("missing value for `{flag}`")))
}

pub(super) fn parse_max(value: &str) -> Result<usize, AppError> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| AppError::invalid_calendar_input(format!("invalid --max value `{value}`")))?;
    if parsed == 0 {
        return Err(AppError::invalid_calendar_input(
            "--max must be greater than zero",
        ));
    }
    Ok(parsed)
}

pub(super) fn parse_property(value: &str) -> Result<(String, String), AppError> {
    let Some((key, property_value)) = value.split_once('=') else {
        return Err(AppError::invalid_calendar_input(format!(
            "invalid --private-property `{value}`; expected key=value"
        )));
    };
    if key.is_empty() {
        return Err(AppError::invalid_calendar_input(format!(
            "invalid --private-property `{value}`; key must not be empty"
        )));
    }
    Ok((key.to_string(), property_value.to_string()))
}

pub(super) fn require_calendar_id(value: Option<String>) -> Result<String, AppError> {
    let id = value
        .ok_or_else(|| AppError::invalid_calendar_input("missing required `--calendar-id <id>`"))?;
    if id.trim().is_empty() {
        return Err(AppError::invalid_calendar_input(
            "`--calendar-id` must not be empty",
        ));
    }
    Ok(id)
}

/// `timeMin` / `timeMax` are RFC3339 instants; unlike event bodies the list API
/// has no companion `timeZone` field, so a bare wall-clock value is ambiguous
/// and is rejected rather than silently assumed to be UTC.
fn require_rfc3339(value: &str, flag: &str) -> Result<String, AppError> {
    let has_offset = value.ends_with('Z')
        || value.ends_with('z')
        || value
            .rsplit_once(['+', '-'])
            .is_some_and(|(head, tail)| head.contains('T') && tail.contains(':'));
    if !value.contains('T') || !has_offset {
        return Err(AppError::invalid_calendar_input(format!(
            "`{flag}` must be an RFC3339 instant with an offset, such as `2026-08-15T00:00:00+08:00` or `2026-08-14T16:00:00Z`"
        )));
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bare_dates_for_time_bounds() {
        assert!(require_rfc3339("2026-08-15", "--from").is_err());
        assert!(require_rfc3339("2026-08-15T00:00:00", "--from").is_err());
        assert!(require_rfc3339("2026-08-15T00:00:00+08:00", "--from").is_ok());
        assert!(require_rfc3339("2026-08-14T16:00:00Z", "--from").is_ok());
    }

    #[test]
    fn parses_private_property_pairs() {
        assert_eq!(
            parse_property("gn_group_id=C5be").expect("pair"),
            ("gn_group_id".to_string(), "C5be".to_string())
        );
        assert!(parse_property("gn_group_id").is_err());
        assert!(parse_property("=value").is_err());
    }

    #[test]
    fn rejects_zero_and_non_numeric_max() {
        assert!(parse_max("0").is_err());
        assert!(parse_max("many").is_err());
        assert_eq!(parse_max("10").expect("max"), 10);
    }
}
