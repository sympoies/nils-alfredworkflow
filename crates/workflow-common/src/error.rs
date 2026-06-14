use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliErrorKind {
    User,
    Runtime,
}

impl CliErrorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Runtime => "runtime",
        }
    }

    pub const fn exit_code(self) -> i32 {
        match self {
            Self::User => 2,
            Self::Runtime => 1,
        }
    }
}

/// CLI-level error carrying a stable error code plus a human-readable message.
///
/// Every workflow CLI maps its domain failures into an [`AppError`] before
/// emitting the Alfred error item or the `cli-envelope@v1` service error
/// envelope, so the shared type owns the classification (`user`/`runtime`),
/// the exit code, and the stable `NILS_*` code that callers branch on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    kind: CliErrorKind,
    code: &'static str,
    message: String,
}

impl AppError {
    /// Build an error from an explicit kind, stable code, and message.
    pub fn new(kind: CliErrorKind, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            code,
            message: message.into(),
        }
    }

    /// Build a user-facing error (exit code `2`) with a stable `NILS_*` code.
    pub fn user(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(CliErrorKind::User, code, message)
    }

    /// Build a runtime error (exit code `1`) with a stable `NILS_*` code.
    pub fn runtime(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(CliErrorKind::Runtime, code, message)
    }

    /// Error classification (`user` vs `runtime`).
    pub fn kind(&self) -> CliErrorKind {
        self.kind
    }

    /// Stable machine-readable error code (for example `NILS_MARKET_001`).
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// Human-readable error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Process exit code derived from the error kind.
    pub fn exit_code(&self) -> i32 {
        self.kind.exit_code()
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("path does not exist: {0}")]
    MissingPath(PathBuf),
    #[error("path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("no remote 'origin' found in {0}")]
    MissingOrigin(PathBuf),
    #[error("unsupported remote URL format: {0}")]
    UnsupportedRemote(String),
    #[error("failed to execute git in {path}: {message}")]
    GitCommand { path: PathBuf, message: String },
    #[error("failed to persist usage log at {path}: {source}")]
    UsageWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_error_carries_kind_code_and_exit_code() {
        let error = AppError::user("NILS_DEMO_001", "bad input");

        assert_eq!(error.kind(), CliErrorKind::User);
        assert_eq!(error.code(), "NILS_DEMO_001");
        assert_eq!(error.message(), "bad input");
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn runtime_error_carries_kind_code_and_exit_code() {
        let error = AppError::runtime("NILS_DEMO_002", String::from("boom"));

        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert_eq!(error.code(), "NILS_DEMO_002");
        assert_eq!(error.message(), "boom");
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn new_matches_kind_specific_constructors() {
        assert_eq!(
            AppError::new(CliErrorKind::User, "NILS_DEMO_003", "x"),
            AppError::user("NILS_DEMO_003", "x")
        );
        assert_eq!(
            AppError::new(CliErrorKind::Runtime, "NILS_DEMO_004", "y"),
            AppError::runtime("NILS_DEMO_004", "y")
        );
    }

    #[test]
    fn display_renders_the_message_only() {
        let error = AppError::runtime("NILS_DEMO_005", "render me");

        assert_eq!(error.to_string(), "render me");
    }

    #[test]
    fn exit_code_tracks_cli_error_kind() {
        assert_eq!(CliErrorKind::User.exit_code(), 2);
        assert_eq!(CliErrorKind::Runtime.exit_code(), 1);
    }
}
