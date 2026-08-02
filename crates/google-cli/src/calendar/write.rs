use serde_json::json;

use crate::error::AppError;

use super::client::{
    CalendarSession, EventCreateRequest, EventGetRequest, EventUpdateRequest, TimeSpec,
};
use super::read::{parse_property, require_calendar_id, value_for};
use super::{NativeCalendarResponse, response};

pub fn execute_events_delete(
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
                    "unknown events delete flag `{unknown}`"
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
                "`events delete` requires an event id, either positional or via --event-id",
            )
        })?,
    };
    session.delete_event(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "calendar_id": request.calendar_id,
            "event_id": request.event_id,
            "fixture_mode": session.is_fixture_mode(),
            "deleted": true,
        }),
        format!(
            "Deleted event `{}` from `{}`.",
            request.event_id, request.calendar_id
        ),
    ))
}

pub fn execute_events_update(
    session: &CalendarSession,
    args: &[String],
) -> Result<NativeCalendarResponse, AppError> {
    let mut calendar_id = None;
    let mut event_id = None;
    let mut summary = None;
    let mut start = None;
    let mut end = None;
    let mut time_zone = None;
    let mut location = None;
    let mut description = None;
    let mut google_meet = false;

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
            "--summary" | "--title" => {
                index += 1;
                summary = Some(value_for(args, index, "--summary")?.clone());
            }
            "--start" => {
                index += 1;
                start = Some(TimeSpec::parse(value_for(args, index, "--start")?)?);
            }
            "--end" => {
                index += 1;
                end = Some(TimeSpec::parse(value_for(args, index, "--end")?)?);
            }
            "--time-zone" => {
                index += 1;
                time_zone = Some(value_for(args, index, "--time-zone")?.clone());
            }
            "--location" => {
                index += 1;
                location = Some(value_for(args, index, "--location")?.clone());
            }
            "--description" => {
                index += 1;
                description = Some(value_for(args, index, "--description")?.clone());
            }
            "--google-meet" => google_meet = true,
            unknown if unknown.starts_with('-') => {
                return Err(AppError::invalid_calendar_input(format!(
                    "unknown events update flag `{unknown}`"
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

    let request = EventUpdateRequest {
        calendar_id: require_calendar_id(calendar_id)?,
        event_id: event_id.ok_or_else(|| {
            AppError::invalid_calendar_input(
                "`events update` requires an event id, either positional or via --event-id",
            )
        })?,
        summary,
        start,
        end,
        time_zone,
        location,
        description,
        google_meet,
    };
    let event = session.update_event(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "calendar_id": request.calendar_id,
            "fixture_mode": session.is_fixture_mode(),
            "event": event,
        }),
        format!(
            "Updated event `{}` in `{}`.",
            request.event_id, request.calendar_id
        ),
    ))
}

pub fn execute_events_create(
    session: &CalendarSession,
    args: &[String],
) -> Result<NativeCalendarResponse, AppError> {
    let mut calendar_id = None;
    let mut summary = None;
    let mut start = None;
    let mut end = None;
    let mut time_zone = None;
    let mut location = None;
    let mut description = None;
    let mut attendees = Vec::new();
    let mut private_properties = Vec::new();
    let mut google_meet = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--calendar-id" => {
                index += 1;
                calendar_id = Some(value_for(args, index, "--calendar-id")?.clone());
            }
            "--summary" | "--title" => {
                index += 1;
                summary = Some(value_for(args, index, "--summary")?.clone());
            }
            "--start" => {
                index += 1;
                start = Some(TimeSpec::parse(value_for(args, index, "--start")?)?);
            }
            "--end" => {
                index += 1;
                end = Some(TimeSpec::parse(value_for(args, index, "--end")?)?);
            }
            "--time-zone" => {
                index += 1;
                time_zone = Some(value_for(args, index, "--time-zone")?.clone());
            }
            "--location" => {
                index += 1;
                location = Some(value_for(args, index, "--location")?.clone());
            }
            "--description" => {
                index += 1;
                description = Some(value_for(args, index, "--description")?.clone());
            }
            "--attendee" => {
                index += 1;
                attendees.push(value_for(args, index, "--attendee")?.clone());
            }
            "--private-property" => {
                index += 1;
                private_properties.push(parse_property(value_for(
                    args,
                    index,
                    "--private-property",
                )?)?);
            }
            "--google-meet" => google_meet = true,
            unknown => {
                return Err(AppError::invalid_calendar_input(format!(
                    "unknown events create flag `{unknown}`"
                )));
            }
        }
        index += 1;
    }

    let request = EventCreateRequest {
        calendar_id: require_calendar_id(calendar_id)?,
        summary: summary.ok_or_else(|| {
            AppError::invalid_calendar_input("missing required `--summary <text>`")
        })?,
        start: start.ok_or_else(|| {
            AppError::invalid_calendar_input(
                "missing required `--start <YYYY-MM-DD|YYYY-MM-DDTHH:MM>`",
            )
        })?,
        end,
        time_zone,
        location,
        description,
        attendees,
        private_properties,
        google_meet,
    };

    let event = session.create_event(&request)?;

    Ok(response(
        json!({
            "account": session.account,
            "account_source": session.account_source,
            "calendar_id": request.calendar_id,
            "fixture_mode": session.is_fixture_mode(),
            "event": event,
        }),
        format!(
            "Created event `{}` ({}) in `{}`.",
            event.summary, event.start, request.calendar_id
        ),
    ))
}
