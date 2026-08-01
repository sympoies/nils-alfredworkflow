use serde_json::json;

use crate::error::AppError;

use super::client::{CalendarSession, EventCreateRequest, TimeSpec};
use super::read::{parse_property, require_calendar_id, value_for};
use super::{NativeCalendarResponse, response};

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
