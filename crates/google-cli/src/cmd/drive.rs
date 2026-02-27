use std::ffi::OsString;

use clap::{Args, Subcommand};

use super::common::{ExtraArgs, Invocation, QueryArgs, TargetArgs};

#[derive(Debug, Clone, Args)]
pub struct DriveArgs {
    #[command(subcommand)]
    command: DriveCommand,
}

#[derive(Debug, Clone, Subcommand)]
enum DriveCommand {
    /// List files in a folder.
    #[command(alias = "list")]
    Ls(ExtraArgs),
    /// Full-text search across Drive.
    #[command(alias = "find")]
    Search(QueryArgs),
    /// Get file metadata.
    Get(TargetArgs),
    /// Download a file.
    #[command(alias = "dl")]
    Download(TargetArgs),
    /// Upload a file.
    #[command(alias = "up", alias = "put")]
    Upload(TargetArgs),
}

impl DriveArgs {
    pub fn command_id_hint(&self) -> &str {
        match &self.command {
            DriveCommand::Ls(_) => "google.drive.ls",
            DriveCommand::Search(_) => "google.drive.search",
            DriveCommand::Get(_) => "google.drive.get",
            DriveCommand::Download(_) => "google.drive.download",
            DriveCommand::Upload(_) => "google.drive.upload",
        }
    }

    pub fn into_invocation(self) -> Invocation {
        match self.command {
            DriveCommand::Ls(args) => {
                Invocation::new("google.drive.ls", ["drive", "ls"], args.extra_args)
            }
            DriveCommand::Search(args) => {
                Invocation::new("google.drive.search", ["drive", "search"], args.args)
            }
            DriveCommand::Get(args) => {
                Invocation::new("google.drive.get", ["drive", "get"], join_target(args))
            }
            DriveCommand::Download(args) => Invocation::new(
                "google.drive.download",
                ["drive", "download"],
                join_target(args),
            ),
            DriveCommand::Upload(args) => Invocation::new(
                "google.drive.upload",
                ["drive", "upload"],
                join_target(args),
            ),
        }
    }
}

fn join_target(args: TargetArgs) -> Vec<OsString> {
    let mut values = vec![args.target];
    values.extend(args.extra.extra_args);
    values
}
