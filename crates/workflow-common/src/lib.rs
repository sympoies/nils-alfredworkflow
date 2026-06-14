//! Shared open-project domain modules.
//!
//! - `config`: environment/default parsing and path expansion.
//! - `discovery`: git repository scan + query filtering.
//! - `usage_log`: usage file read/write + timestamp sort keys.
//! - `git`: git metadata helpers and remote URL normalization for GitHub + generic `host/path` hosts.
//! - `feedback`: Alfred item assembly.
//! - `output_contract`: shared output modes + JSON envelope helpers.
//! - `list_parser`: ordered comma/newline list parsing utilities.
//! - `http`: opt-in blocking reqwest client builder primitive.

pub mod config;
pub mod discovery;
pub mod error;
pub mod feedback;
pub mod git;
#[cfg(feature = "http")]
pub mod http;
pub mod list_parser;
pub mod output_contract;
pub mod usage_log;

pub use alfred_core::Feedback;
pub use config::{
    DEFAULT_OPEN_PROJECT_MAX_RESULTS, DEFAULT_PROJECT_DIRS, DEFAULT_USAGE_FILE,
    DEFAULT_VSCODE_PATH, RuntimeConfig, expand_home_tokens, parse_project_dirs,
};
pub use error::{AppError, CliErrorKind, WorkflowError};
pub use feedback::{
    ScriptFilterMode, build_script_filter_feedback, build_script_filter_feedback_with_mode,
    no_projects_feedback, subtitle_format,
};
pub use git::{normalize_remote, web_url_for_project};
pub use list_parser::{parse_ordered_list_with, split_ordered_list};
#[cfg(feature = "clap")]
pub use output_contract::ScriptFilterOutputModeArg;
pub use output_contract::{
    ENVELOPE_SCHEMA_VERSION, EnvelopePayloadKind, OutputMode, build_alfred_error_feedback,
    build_error_details_json, build_error_envelope, build_feedback_result_envelope,
    build_success_envelope, redact_sensitive,
};
pub use usage_log::{parse_usage_timestamp, record_usage};

pub fn build_feedback(query: &str) -> Feedback {
    let config = RuntimeConfig::from_env();
    build_script_filter_feedback(query, &config)
}
