use std::ffi::OsString;

use clap::{Args, Subcommand};

use super::common::{ExtraArgs, Invocation, NestedArgs, TargetArgs, dynamic_command_id};

#[derive(Debug, Clone, Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    command: AuthCommand,
}

#[derive(Debug, Clone, Subcommand)]
enum AuthCommand {
    /// Manage OAuth client credentials.
    Credentials(NestedArgs),
    /// Authorize and store a refresh token.
    Add(TargetArgs),
    /// List stored accounts.
    List(ExtraArgs),
    /// Show auth configuration and keyring backend.
    Status(ExtraArgs),
    /// Remove a stored refresh token.
    Remove(TargetArgs),
    /// Manage account aliases.
    Alias(NestedArgs),
    /// Show terminal-native account management summary.
    Manage(ExtraArgs),
}

impl AuthArgs {
    pub fn command_id_hint(&self) -> &str {
        match &self.command {
            AuthCommand::Credentials(_) => "google.auth.credentials",
            AuthCommand::Add(_) => "google.auth.add",
            AuthCommand::List(_) => "google.auth.list",
            AuthCommand::Status(_) => "google.auth.status",
            AuthCommand::Remove(_) => "google.auth.remove",
            AuthCommand::Alias(_) => "google.auth.alias",
            AuthCommand::Manage(_) => "google.auth.manage",
        }
    }

    pub fn into_invocation(self) -> Invocation {
        match self.command {
            AuthCommand::Credentials(args) => Invocation::new(
                dynamic_command_id("google.auth.credentials", &args.args),
                ["auth", "credentials"],
                args.args,
            ),
            AuthCommand::Add(args) => {
                Invocation::new("google.auth.add", ["auth", "add"], join_target(args))
            }
            AuthCommand::List(args) => {
                Invocation::new("google.auth.list", ["auth", "list"], args.extra_args)
            }
            AuthCommand::Status(args) => {
                Invocation::new("google.auth.status", ["auth", "status"], args.extra_args)
            }
            AuthCommand::Remove(args) => {
                Invocation::new("google.auth.remove", ["auth", "remove"], join_target(args))
            }
            AuthCommand::Alias(args) => Invocation::new(
                dynamic_command_id("google.auth.alias", &args.args),
                ["auth", "alias"],
                args.args,
            ),
            AuthCommand::Manage(args) => {
                Invocation::new("google.auth.manage", ["auth", "manage"], args.extra_args)
            }
        }
    }
}

fn join_target(args: TargetArgs) -> Vec<OsString> {
    let mut values = vec![args.target];
    values.extend(args.extra.extra_args);
    values
}
