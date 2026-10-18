use clap::{Args, Subcommand};

use super::common::{Invocation, NestedArgs, dynamic_command_id};

#[derive(Debug, Clone, Args)]
pub struct CalendarArgs {
    #[command(subcommand)]
    command: CalendarCommand,
}

#[derive(Debug, Clone, Subcommand)]
enum CalendarCommand {
    /// Calendar list operations: `list`.
    #[command(alias = "calendar")]
    Calendars(NestedArgs),
    /// Event operations: `list`, `get`, `create`, `delete`.
    #[command(alias = "event")]
    Events(NestedArgs),
}

impl CalendarArgs {
    pub fn command_id_hint(&self) -> &str {
        match &self.command {
            CalendarCommand::Calendars(args) => {
                match args.args.first().and_then(|arg| arg.to_str()) {
                    Some("list") => "google.calendar.calendars.list",
                    _ => "google.calendar.calendars",
                }
            }
            CalendarCommand::Events(args) => match args.args.first().and_then(|arg| arg.to_str()) {
                Some("list") => "google.calendar.events.list",
                Some("get") => "google.calendar.events.get",
                Some("create") => "google.calendar.events.create",
                Some("update") => "google.calendar.events.update",
                Some("delete") => "google.calendar.events.delete",
                Some("respond") => "google.calendar.events.respond",
                _ => "google.calendar.events",
            },
        }
    }

    pub fn into_invocation(self) -> Invocation {
        match self.command {
            CalendarCommand::Calendars(args) => Invocation::new(
                dynamic_command_id("google.calendar.calendars", &args.args),
                ["calendar", "calendars"],
                args.args,
            ),
            CalendarCommand::Events(args) => Invocation::new(
                dynamic_command_id("google.calendar.events", &args.args),
                ["calendar", "events"],
                args.args,
            ),
        }
    }
}
