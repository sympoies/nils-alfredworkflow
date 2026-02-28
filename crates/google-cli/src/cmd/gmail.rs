use std::ffi::OsString;

use clap::{Args, Subcommand};

use super::common::{ExtraArgs, Invocation, NestedArgs, QueryArgs, TargetArgs, dynamic_command_id};

#[derive(Debug, Clone, Args)]
pub struct GmailArgs {
    #[command(subcommand)]
    command: GmailCommand,
}

#[derive(Debug, Clone, Subcommand)]
enum GmailCommand {
    /// Search threads using Gmail query syntax.
    #[command(alias = "list", alias = "query")]
    Search(QueryArgs),
    /// Get a message.
    #[command(alias = "show")]
    Get(TargetArgs),
    /// Send an email.
    Send(ExtraArgs),
    /// Thread operations.
    #[command(alias = "threads")]
    Thread(NestedArgs),
}

impl GmailArgs {
    pub fn command_id_hint(&self) -> &str {
        match &self.command {
            GmailCommand::Search(_) => "google.gmail.search",
            GmailCommand::Get(_) => "google.gmail.get",
            GmailCommand::Send(_) => "google.gmail.send",
            GmailCommand::Thread(_) => "google.gmail.thread",
        }
    }

    pub fn into_invocation(self) -> Invocation {
        match self.command {
            GmailCommand::Search(args) => {
                Invocation::new("google.gmail.search", ["gmail", "search"], args.args)
            }
            GmailCommand::Get(args) => {
                Invocation::new("google.gmail.get", ["gmail", "get"], join_target(args))
            }
            GmailCommand::Send(args) => {
                Invocation::new("google.gmail.send", ["gmail", "send"], args.extra_args)
            }
            GmailCommand::Thread(args) => Invocation::new(
                dynamic_command_id("google.gmail.thread", &args.args),
                ["gmail", "thread"],
                args.args,
            ),
        }
    }
}

fn join_target(args: TargetArgs) -> Vec<OsString> {
    let mut values = vec![args.target];
    values.extend(args.extra.extra_args);
    values
}
