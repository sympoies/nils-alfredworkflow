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
            CalendarCommand::Calendars(_) => "google.calendar.calendars",
            CalendarCommand::Events(_) => "google.calendar.events",
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
