use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use workflow_common::{
    EnvelopePayloadKind, OutputMode, RuntimeConfig, ScriptFilterMode, WorkflowError,
    build_alfred_error_feedback, build_error_details_json, build_error_envelope,
    build_script_filter_feedback_with_mode, build_success_envelope, github_url_for_project,
    record_usage, select_output_mode,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Shared Alfred workflow CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Render Alfred script-filter JSON.
    ScriptFilter {
        /// Input query from Alfred.
        #[arg(long, short, default_value = "")]
        query: String,
        /// Display mode for icon treatment.
        #[arg(long, value_enum, default_value_t = ScriptFilterModeArg::Open)]
        mode: ScriptFilterModeArg,
        /// Explicit output mode (`human`, `json`, `alfred-json`).
        #[arg(long, value_enum)]
        output: Option<OutputModeArg>,
        /// Legacy compatibility flag for JSON service mode.
        #[arg(long)]
        json: bool,
    },
    /// Record usage timestamp for a selected project path.
    RecordUsage {
        /// Selected project path.
        #[arg(long)]
        path: PathBuf,
    },
    /// Resolve project origin URL to a canonical GitHub URL.
    GithubUrl {
        /// Selected project path.
        #[arg(long)]
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ScriptFilterModeArg {
    Open,
    Github,
}

impl From<ScriptFilterModeArg> for ScriptFilterMode {
    fn from(value: ScriptFilterModeArg) -> Self {
        match value {
            ScriptFilterModeArg::Open => ScriptFilterMode::Open,
            ScriptFilterModeArg::Github => ScriptFilterMode::Github,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputModeArg {
    Human,
    Json,
    AlfredJson,
}

impl From<OutputModeArg> for OutputMode {
    fn from(value: OutputModeArg) -> Self {
        match value {
            OutputModeArg::Human => OutputMode::Human,
            OutputModeArg::Json => OutputMode::Json,
            OutputModeArg::AlfredJson => OutputMode::AlfredJson,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorKind {
    User,
    Runtime,
}

#[derive(Debug)]
struct AppError {
    kind: ErrorKind,
    code: &'static str,
    message: String,
}

impl AppError {
    fn user(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::User,
            code,
            message: message.into(),
        }
    }

    fn runtime(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            code,
            message: message.into(),
        }
    }

    fn exit_code(&self) -> i32 {
        match self.kind {
            ErrorKind::User => 2,
            ErrorKind::Runtime => 1,
        }
    }
}

const ERROR_CODE_USER_INVALID_PATH: &str = "user.invalid_path";
const ERROR_CODE_USER_OUTPUT_MODE_CONFLICT: &str = "user.output_mode_conflict";
const ERROR_CODE_RUNTIME_GIT: &str = "runtime.git_failed";
const ERROR_CODE_RUNTIME_USAGE_WRITE: &str = "runtime.usage_persist_failed";
const ERROR_CODE_RUNTIME_SERIALIZE: &str = "runtime.serialize_failed";

impl Cli {
    fn command_name(&self) -> &'static str {
        match &self.command {
            Commands::ScriptFilter { .. } => "workflow.script-filter",
            Commands::RecordUsage { .. } => "workflow.record-usage",
            Commands::GithubUrl { .. } => "workflow.github-url",
        }
    }

    fn output_mode_hint(&self) -> OutputMode {
        match &self.command {
            Commands::ScriptFilter { output, json, .. } => {
                if *json {
                    OutputMode::Json
                } else if let Some(mode) = output {
                    (*mode).into()
                } else {
                    OutputMode::AlfredJson
                }
            }
            Commands::RecordUsage { .. } | Commands::GithubUrl { .. } => OutputMode::Human,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let command = cli.command_name();
    let output_mode = cli.output_mode_hint();

    match run(cli) {
        Ok(stdout) => {
            println!("{stdout}");
        }
        Err(err) => {
            emit_error(command, output_mode, &err);
            std::process::exit(err.exit_code());
        }
    }
}

fn run(cli: Cli) -> Result<String, AppError> {
    let config = RuntimeConfig::from_env();
    run_with_config(cli, &config)
}

fn run_with_config(cli: Cli, config: &RuntimeConfig) -> Result<String, AppError> {
    match cli.command {
        Commands::ScriptFilter {
            query,
            mode,
            output,
            json,
        } => {
            let output_mode =
                select_output_mode(output.map(Into::into), json, OutputMode::AlfredJson).map_err(
                    |error| AppError::user(ERROR_CODE_USER_OUTPUT_MODE_CONFLICT, error.to_string()),
                )?;
            let feedback = build_script_filter_feedback_with_mode(&query, config, mode.into());
            let alfred_json = feedback.to_json().map_err(|error| {
                AppError::runtime(
                    ERROR_CODE_RUNTIME_SERIALIZE,
                    format!("failed to serialize Alfred feedback: {error}"),
                )
            })?;

            match output_mode {
                OutputMode::AlfredJson => Ok(alfred_json),
                OutputMode::Json => Ok(build_success_envelope(
                    "workflow.script-filter",
                    EnvelopePayloadKind::Result,
                    &alfred_json,
                )),
                OutputMode::Human => Ok(render_script_filter_human(&feedback)),
            }
        }
        Commands::RecordUsage { path } => {
            validate_project_path(&path)?;
            record_usage(&path, &config.usage_file).map_err(map_workflow_error)?;
            Ok(path.to_string_lossy().to_string())
        }
        Commands::GithubUrl { path } => {
            validate_project_path(&path)?;
            github_url_for_project(&path).map_err(map_workflow_error)
        }
    }
}

fn render_script_filter_human(feedback: &workflow_common::Feedback) -> String {
    if feedback.items.is_empty() {
        return "No projects matched".to_string();
    }

    let mut lines = Vec::with_capacity(feedback.items.len());
    for item in &feedback.items {
        if let Some(subtitle) = &item.subtitle {
            lines.push(format!("{} | {}", item.title, subtitle));
        } else {
            lines.push(item.title.clone());
        }
    }
    lines.join("\n")
}

fn emit_error(command: &str, output_mode: OutputMode, error: &AppError) {
    match output_mode {
        OutputMode::Json => {
            let details = build_error_details_json(error_kind_label(error.kind), error.exit_code());
            println!(
                "{}",
                build_error_envelope(command, error.code, &error.message, Some(&details))
            );
        }
        OutputMode::AlfredJson => {
            println!(
                "{}",
                build_alfred_error_feedback(error.code, &error.message)
            );
        }
        OutputMode::Human => {
            eprintln!(
                "error[{}]: {}",
                error.code,
                workflow_common::redact_sensitive(&error.message),
            );
        }
    }
}

fn error_kind_label(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::User => "user",
        ErrorKind::Runtime => "runtime",
    }
}

fn validate_project_path(path: &Path) -> Result<(), AppError> {
    if !path.exists() {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_PATH,
            format!("path does not exist: {}", path.to_string_lossy()),
        ));
    }

    if !path.is_dir() {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_PATH,
            format!("path is not a directory: {}", path.to_string_lossy()),
        ));
    }

    Ok(())
}

fn map_workflow_error(error: WorkflowError) -> AppError {
    match error {
        WorkflowError::MissingPath(path) => AppError::user(
            ERROR_CODE_USER_INVALID_PATH,
            format!("path does not exist: {}", path.to_string_lossy()),
        ),
        WorkflowError::NotDirectory(path) => AppError::user(
            ERROR_CODE_USER_INVALID_PATH,
            format!("path is not a directory: {}", path.to_string_lossy()),
        ),
        WorkflowError::MissingOrigin(path) => AppError::runtime(
            ERROR_CODE_RUNTIME_GIT,
            format!("no remote 'origin' found in {}", path.to_string_lossy()),
        ),
        WorkflowError::UnsupportedRemote(remote) => AppError::runtime(
            ERROR_CODE_RUNTIME_GIT,
            format!("unsupported remote URL format: {remote}"),
        ),
        WorkflowError::GitCommand { path, message } => AppError::runtime(
            ERROR_CODE_RUNTIME_GIT,
            format!(
                "failed to execute git in {}: {message}",
                path.to_string_lossy()
            ),
        ),
        WorkflowError::UsageWrite { path, source } => AppError::runtime(
            ERROR_CODE_RUNTIME_USAGE_WRITE,
            format!(
                "failed to persist usage log at {}: {source}",
                path.to_string_lossy()
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn script_filter_command_outputs_json_contract() {
        let temp = tempdir().expect("create temp dir");
        let root = temp.path().join("projects");
        let repo = root.join("alpha");
        init_repo(&repo);

        let config = RuntimeConfig {
            project_roots: vec![root],
            usage_file: temp.path().join("usage.log"),
            vscode_path: "code".to_string(),
            max_results: 10,
        };

        let output = run_with_config(
            Cli {
                command: Commands::ScriptFilter {
                    query: String::new(),
                    mode: ScriptFilterModeArg::Open,
                    output: Some(OutputModeArg::Json),
                    json: false,
                },
            },
            &config,
        )
        .expect("script-filter should succeed");

        let json: serde_json::Value =
            serde_json::from_str(&output).expect("script-filter output should be valid JSON");
        assert_eq!(
            json.get("schema_version").and_then(|x| x.as_str()),
            Some("v1")
        );
        assert_eq!(
            json.get("command").and_then(|x| x.as_str()),
            Some("workflow.script-filter")
        );
        assert_eq!(json.get("ok").and_then(|x| x.as_bool()), Some(true));
        assert!(
            json.get("result").is_some(),
            "JSON output should contain result field"
        );
    }

    #[test]
    fn action_commands_output_plain_values() {
        let temp = tempdir().expect("create temp dir");
        let root = temp.path().join("projects");
        let repo = root.join("alpha");
        init_repo(&repo);

        let status = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["remote", "add", "origin", "git@github.com:owner/repo.git"])
            .status()
            .expect("set git remote");
        assert!(status.success(), "git remote add should succeed");

        let config = RuntimeConfig {
            project_roots: vec![root],
            usage_file: temp.path().join("usage.log"),
            vscode_path: "code".to_string(),
            max_results: 10,
        };

        let recorded = run_with_config(
            Cli {
                command: Commands::RecordUsage { path: repo.clone() },
            },
            &config,
        )
        .expect("record-usage should succeed");
        assert_eq!(
            recorded,
            repo.to_string_lossy(),
            "record-usage should output plain path"
        );

        let github_url = run_with_config(
            Cli {
                command: Commands::GithubUrl { path: repo.clone() },
            },
            &config,
        )
        .expect("github-url should succeed");
        assert_eq!(
            github_url, "https://github.com/owner/repo",
            "github-url should output canonical URL only"
        );
    }

    #[test]
    fn github_url_accepts_ssh_url_remote_format() {
        let temp = tempdir().expect("create temp dir");
        let root = temp.path().join("projects");
        let repo = root.join("alpha");
        init_repo(&repo);

        let status = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "ssh://git@github.com/owner/repo.git",
            ])
            .status()
            .expect("set git remote");
        assert!(status.success(), "git remote add should succeed");

        let config = RuntimeConfig {
            project_roots: vec![root],
            usage_file: temp.path().join("usage.log"),
            vscode_path: "code".to_string(),
            max_results: 10,
        };

        let github_url = run_with_config(
            Cli {
                command: Commands::GithubUrl { path: repo.clone() },
            },
            &config,
        )
        .expect("github-url should succeed for ssh url remotes");
        assert_eq!(
            github_url, "https://github.com/owner/repo",
            "github-url should output canonical URL for ssh url remotes"
        );
    }

    #[test]
    fn action_commands_report_user_error_for_invalid_path() {
        let temp = tempdir().expect("create temp dir");
        let config = RuntimeConfig {
            project_roots: vec![temp.path().join("projects")],
            usage_file: temp.path().join("usage.log"),
            vscode_path: "code".to_string(),
            max_results: 10,
        };

        let missing = temp.path().join("missing-project");
        let err = run_with_config(
            Cli {
                command: Commands::RecordUsage {
                    path: missing.clone(),
                },
            },
            &config,
        )
        .expect_err("missing project should produce user error");

        assert_eq!(
            err.kind,
            ErrorKind::User,
            "missing path should be treated as user error"
        );
        assert!(
            err.message.contains(missing.to_string_lossy().as_ref()),
            "error message should include offending path"
        );
        assert_eq!(err.code, ERROR_CODE_USER_INVALID_PATH);
    }

    #[test]
    fn script_filter_github_mode_sets_primary_item_icon() {
        let temp = tempdir().expect("create temp dir");
        let root = temp.path().join("projects");
        let repo = root.join("alpha");
        init_repo(&repo);

        let config = RuntimeConfig {
            project_roots: vec![root],
            usage_file: temp.path().join("usage.log"),
            vscode_path: "code".to_string(),
            max_results: 10,
        };

        let output = run_with_config(
            Cli {
                command: Commands::ScriptFilter {
                    query: String::new(),
                    mode: ScriptFilterModeArg::Github,
                    output: None,
                    json: false,
                },
            },
            &config,
        )
        .expect("script-filter should succeed");

        let json: serde_json::Value =
            serde_json::from_str(&output).expect("script-filter output should be valid JSON");
        let icon_path = json
            .get("items")
            .and_then(|items| items.get(0))
            .and_then(|item| item.get("icon"))
            .and_then(|icon| icon.get("path"))
            .and_then(|path| path.as_str())
            .expect("github mode should include primary icon path");

        assert_eq!(icon_path, "assets/icon-github.png");
    }

    #[test]
    fn script_filter_default_mode_keeps_alfred_json_contract() {
        let temp = tempdir().expect("create temp dir");
        let root = temp.path().join("projects");
        let repo = root.join("alpha");
        init_repo(&repo);

        let config = RuntimeConfig {
            project_roots: vec![root],
            usage_file: temp.path().join("usage.log"),
            vscode_path: "code".to_string(),
            max_results: 10,
        };

        let output = run_with_config(
            Cli {
                command: Commands::ScriptFilter {
                    query: String::new(),
                    mode: ScriptFilterModeArg::Open,
                    output: None,
                    json: false,
                },
            },
            &config,
        )
        .expect("script-filter should succeed");

        let json: serde_json::Value =
            serde_json::from_str(&output).expect("script-filter output should be valid JSON");
        assert!(json.get("items").is_some());
    }

    #[test]
    fn script_filter_rejects_conflicting_json_flags() {
        let temp = tempdir().expect("create temp dir");
        let config = RuntimeConfig {
            project_roots: vec![temp.path().join("projects")],
            usage_file: temp.path().join("usage.log"),
            vscode_path: "code".to_string(),
            max_results: 10,
        };

        let err = run_with_config(
            Cli {
                command: Commands::ScriptFilter {
                    query: String::new(),
                    mode: ScriptFilterModeArg::Open,
                    output: Some(OutputModeArg::Human),
                    json: true,
                },
            },
            &config,
        )
        .expect_err("must fail");

        assert_eq!(err.kind, ErrorKind::User);
        assert_eq!(err.code, ERROR_CODE_USER_OUTPUT_MODE_CONFLICT);
    }

    #[test]
    fn script_filter_error_redaction_masks_sensitive_tokens() {
        let redacted = workflow_common::redact_sensitive(
            "Authorization: Bearer abc token=xyz client_secret=demo",
        );
        assert!(!redacted.contains("abc"));
        assert!(!redacted.contains("xyz"));
        assert!(!redacted.contains("demo"));
        assert!(redacted.contains("Bearer [REDACTED]"));
    }

    fn init_repo(path: &Path) {
        fs::create_dir_all(path).expect("create repo dir");
        let status = Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(path)
            .status()
            .expect("run git init");
        assert!(status.success(), "git init should succeed");
    }
}
