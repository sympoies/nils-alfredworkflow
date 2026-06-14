use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use workflow_common::{
    EnvelopePayloadKind, OutputMode, build_error_details_json, build_error_envelope,
    build_success_envelope, redact_sensitive,
};
use workflow_readme_cli::{AppError, ConvertRequest, convert};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Workflow README converter for Alfred plist readme"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Convert README markdown and inject readme content into plist.
    Convert {
        /// Workflow root directory that contains README and local image assets.
        #[arg(long)]
        workflow_root: PathBuf,
        /// Relative path to README from workflow root.
        #[arg(long)]
        readme_source: PathBuf,
        /// Stage directory where local image assets should be copied.
        #[arg(long)]
        stage_dir: PathBuf,
        /// Target plist file that will receive converted readme content.
        #[arg(long)]
        plist: PathBuf,
        /// Validate and render without writing files.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Canonical output mode (`human` or `json`).
        #[arg(long, value_enum, default_value_t = OutputModeArg::Human)]
        output: OutputModeArg,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputModeArg {
    Human,
    Json,
}

impl From<OutputModeArg> for OutputMode {
    fn from(value: OutputModeArg) -> Self {
        match value {
            OutputModeArg::Human => OutputMode::Human,
            OutputModeArg::Json => OutputMode::Json,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ConvertSummary {
    converted_readme_length: usize,
    copied_assets: Vec<String>,
    dry_run: bool,
}

const COMMAND_CONVERT: &str = "workflow-readme.convert";

impl Cli {
    fn command_name(&self) -> &'static str {
        match self.command {
            Commands::Convert { .. } => COMMAND_CONVERT,
        }
    }

    fn output_mode_hint(&self) -> OutputMode {
        match &self.command {
            Commands::Convert { output, .. } => (*output).into(),
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let command = cli.command_name();
    let output_mode = cli.output_mode_hint();

    match run(cli) {
        Ok(summary) => emit_success(command, output_mode, &summary),
        Err(error) => {
            emit_error(command, output_mode, &error);
            std::process::exit(error.exit_code());
        }
    }
}

fn run(cli: Cli) -> Result<ConvertSummary, AppError> {
    match cli.command {
        Commands::Convert {
            workflow_root,
            readme_source,
            stage_dir,
            plist,
            dry_run,
            ..
        } => {
            let result = convert(&ConvertRequest {
                workflow_root,
                readme_source,
                stage_dir,
                plist,
                dry_run,
            })?;

            Ok(ConvertSummary {
                converted_readme_length: result.converted_readme.len(),
                copied_assets: result
                    .copied_assets
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                dry_run,
            })
        }
    }
}

fn emit_success(command: &str, output_mode: OutputMode, summary: &ConvertSummary) {
    match output_mode {
        OutputMode::Human => {
            if summary.dry_run {
                println!(
                    "dry-run: converted {} bytes, detected {} local image asset(s)",
                    summary.converted_readme_length,
                    summary.copied_assets.len()
                );
            } else {
                println!(
                    "converted {} bytes, copied {} local image asset(s)",
                    summary.converted_readme_length,
                    summary.copied_assets.len()
                );
            }
        }
        OutputMode::Json => {
            let result = serde_json::to_string(summary).expect("serialize conversion summary");
            println!(
                "{}",
                build_success_envelope(command, EnvelopePayloadKind::Result, &result)
            );
        }
        OutputMode::AlfredJson => unreachable!("workflow-readme does not expose alfred-json mode"),
    }
}

fn emit_error(command: &str, output_mode: OutputMode, error: &AppError) {
    match output_mode {
        OutputMode::Json => {
            let details = build_error_details_json(error.kind().as_str(), error.exit_code());
            println!(
                "{}",
                build_error_envelope(command, error.code(), error.message(), Some(&details))
            );
        }
        OutputMode::Human => {
            eprintln!(
                "error[{}]: {}",
                error.code(),
                redact_sensitive(error.message())
            );
        }
        OutputMode::AlfredJson => unreachable!("workflow-readme does not expose alfred-json mode"),
    }
}
