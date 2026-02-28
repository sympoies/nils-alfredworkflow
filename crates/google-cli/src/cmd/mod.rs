pub mod auth;
pub mod common;
pub mod drive;
pub mod gmail;

use clap::{Parser, Subcommand};

use crate::error::AppError;
use crate::output::OutputMode;

pub use common::GlobalOptions;

#[derive(Debug, Clone, Parser)]
#[command(
    author,
    version,
    about = "Native Rust Google CLI for auth, Gmail, and Drive commands"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOptions,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    /// Native auth commands.
    Auth(auth::AuthArgs),
    /// Native Gmail commands.
    Gmail(gmail::GmailArgs),
    /// Drive commands (wrapper-backed until native Drive sprint lands).
    Drive(drive::DriveArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub global: GlobalOptions,
    pub invocation: common::Invocation,
}

impl Cli {
    pub fn output_mode_hint(&self) -> OutputMode {
        self.global.output_mode_hint()
    }

    pub fn command_id_hint(&self) -> &str {
        match &self.command {
            Commands::Auth(command) => command.command_id_hint(),
            Commands::Gmail(command) => command.command_id_hint(),
            Commands::Drive(command) => command.command_id_hint(),
        }
    }

    pub fn into_request(self) -> Result<Request, AppError> {
        self.global.validate()?;
        let invocation = match self.command {
            Commands::Auth(command) => command.into_invocation(),
            Commands::Gmail(command) => command.into_invocation(),
            Commands::Drive(command) => command.into_invocation(),
        };

        Ok(Request {
            global: self.global,
            invocation,
        })
    }
}
