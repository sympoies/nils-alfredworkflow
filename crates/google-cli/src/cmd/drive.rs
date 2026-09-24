use std::ffi::OsString;

use clap::{Args, Subcommand};

use super::common::{ExtraArgs, Invocation, QueryArgs, TargetArgs};

#[derive(Debug, Clone, Args)]
pub struct DriveArgs {
    #[command(subcommand)]
    command: DriveCommand,
}

#[derive(Debug, Clone, Args)]
struct FolderNameArgs {
    #[arg(allow_hyphen_values = true)]
    name: OsString,
    #[command(flatten)]
    extra: ExtraArgs,
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
    /// Create a folder.
    Mkdir(FolderNameArgs),
    /// Replace file content by file ID.
    Update(TargetArgs),
    /// Rename a file.
    Rename(TargetArgs),
    /// Move a file between parents.
    Move(TargetArgs),
    /// Copy a file into a parent.
    Copy(TargetArgs),
    /// Move a file to trash.
    Trash(TargetArgs),
    /// Restore a file from trash.
    Untrash(TargetArgs),
}

impl DriveArgs {
    pub fn command_id_hint(&self) -> &str {
        match &self.command {
            DriveCommand::Ls(_) => "google.drive.ls",
            DriveCommand::Search(_) => "google.drive.search",
            DriveCommand::Get(_) => "google.drive.get",
            DriveCommand::Download(_) => "google.drive.download",
            DriveCommand::Upload(_) => "google.drive.upload",
            DriveCommand::Mkdir(_) => "google.drive.mkdir",
            DriveCommand::Update(_) => "google.drive.update",
            DriveCommand::Rename(_) => "google.drive.rename",
            DriveCommand::Move(_) => "google.drive.move",
            DriveCommand::Copy(_) => "google.drive.copy",
            DriveCommand::Trash(_) => "google.drive.trash",
            DriveCommand::Untrash(_) => "google.drive.untrash",
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
            DriveCommand::Mkdir(args) => {
                let mut values = vec![args.name];
                values.extend(args.extra.extra_args);
                Invocation::new("google.drive.mkdir", ["drive", "mkdir"], values)
            }
            DriveCommand::Update(args) => Invocation::new(
                "google.drive.update",
                ["drive", "update"],
                join_target(args),
            ),
            DriveCommand::Rename(args) => Invocation::new(
                "google.drive.rename",
                ["drive", "rename"],
                join_target(args),
            ),
            DriveCommand::Move(args) => {
                Invocation::new("google.drive.move", ["drive", "move"], join_target(args))
            }
            DriveCommand::Copy(args) => {
                Invocation::new("google.drive.copy", ["drive", "copy"], join_target(args))
            }
            DriveCommand::Trash(args) => {
                Invocation::new("google.drive.trash", ["drive", "trash"], join_target(args))
            }
            DriveCommand::Untrash(args) => Invocation::new(
                "google.drive.untrash",
                ["drive", "untrash"],
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

#[cfg(test)]
mod tests {
    use crate::cmd::Cli;
    use clap::Parser;

    #[test]
    fn hyphen_folder_name_is_local_to_mkdir_parser() {
        assert!(
            Cli::try_parse_from([
                "google-cli",
                "drive",
                "mkdir",
                "-folder",
                "--parent",
                "parent-id"
            ])
            .is_ok()
        );
        for args in [
            vec!["google-cli", "auth", "remove", "--bogus"],
            vec!["google-cli", "gmail", "get", "--bogus"],
            vec!["google-cli", "drive", "trash", "--bogus"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }
}
