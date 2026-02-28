use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cmd::common::GlobalOptions;
use crate::cmd::common::Invocation;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runtime {
    program: PathBuf,
}

impl Runtime {
    pub fn from_global(global: &GlobalOptions) -> Result<Self, AppError> {
        Ok(Self {
            program: resolve_gog(global.resolved_gog_bin_override())?,
        })
    }

    pub fn execute(
        &self,
        global: &GlobalOptions,
        invocation: &Invocation,
    ) -> Result<ProcessOutput, AppError> {
        // Wrapper runtime remains for commands that are not yet migrated (for now: Drive).
        let mut command = Command::new(&self.program);
        command.args(global.gog_flags()?);
        command.args(invocation.path.clone());
        command.args(invocation.args.clone());

        let output = command
            .output()
            .map_err(|error| AppError::process_launch(&self.program, &error))?;

        if output.status.success() {
            Ok(ProcessOutput {
                stdout: output.stdout,
                stderr: output.stderr,
            })
        } else {
            Err(AppError::process_failure(
                invocation.command_id.as_str(),
                output.status.code(),
                &output.stderr,
            ))
        }
    }
}

pub fn resolve_gog(override_path: Option<PathBuf>) -> Result<PathBuf, AppError> {
    match override_path {
        Some(path) if looks_like_path(&path) => {
            if path.is_file() {
                Ok(path)
            } else {
                let requested = path.to_string_lossy().to_string();
                Err(AppError::missing_gog(requested.as_str(), &[path]))
            }
        }
        Some(path) => resolve_on_path(path.as_os_str(), env::var_os("PATH")),
        None => resolve_on_path(OsStr::new("gog"), env::var_os("PATH")),
    }
}

fn resolve_on_path(program: &OsStr, path_var: Option<OsString>) -> Result<PathBuf, AppError> {
    let mut searched = Vec::new();

    if let Some(path_var) = path_var {
        for directory in env::split_paths(&path_var) {
            let candidate = directory.join(program);
            searched.push(candidate.clone());
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(AppError::missing_gog(
        program.to_string_lossy().as_ref(),
        &searched,
    ))
}

fn looks_like_path(path: &Path) -> bool {
    path.is_absolute()
        || path
            .to_string_lossy()
            .chars()
            .any(|character| character == std::path::MAIN_SEPARATOR)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::resolve_gog;
    use crate::cmd::common::GOOGLE_CLI_GOG_BIN_ENV;

    #[test]
    fn resolve_gog_accepts_explicit_existing_path() {
        let dir = tempdir().expect("tempdir");
        let binary = dir.path().join("gog");
        fs::write(&binary, "#!/bin/sh\n").expect("binary");

        let resolved = resolve_gog(Some(binary.clone())).expect("resolved");
        assert_eq!(resolved, binary);
    }

    #[test]
    fn resolve_gog_rejects_missing_override() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("missing-gog");
        let error = resolve_gog(Some(missing)).expect_err("missing override should fail");
        assert_eq!(error.code(), crate::error::ERROR_CODE_RUNTIME_MISSING_GOG);
        assert!(error.message().contains(GOOGLE_CLI_GOG_BIN_ENV));
    }
}
