use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, ValueEnum};

use crate::error::AppError;
use crate::output::OutputMode;

pub const GOOGLE_CLI_GOG_BIN_ENV: &str = "GOOGLE_CLI_GOG_BIN";

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct GlobalOptions {
    /// Account email forwarded to gog.
    #[arg(short = 'a', long, global = true)]
    pub account: Option<String>,
    /// OAuth client name forwarded to gog.
    #[arg(long, global = true)]
    pub client: Option<String>,
    /// Request wrapped JSON output.
    #[arg(short = 'j', long, global = true, default_value_t = false)]
    pub json: bool,
    /// Request stable plain-text output from gog.
    #[arg(short = 'p', long, global = true, default_value_t = false)]
    pub plain: bool,
    /// Drop gog envelope fields in JSON mode.
    #[arg(long, global = true, default_value_t = false)]
    pub results_only: bool,
    /// Select JSON fields in gog JSON mode.
    #[arg(long, global = true)]
    pub select: Option<String>,
    /// Do not make changes.
    #[arg(short = 'n', long, global = true, default_value_t = false)]
    pub dry_run: bool,
    /// Skip confirmations for destructive commands.
    #[arg(short = 'y', long, global = true, default_value_t = false)]
    pub force: bool,
    /// Never prompt interactively.
    #[arg(long, global = true, default_value_t = false)]
    pub no_input: bool,
    /// Enable verbose logging in gog.
    #[arg(short = 'v', long, global = true, default_value_t = false)]
    pub verbose: bool,
    /// Forward gog color mode.
    #[arg(long, global = true, value_enum)]
    pub color: Option<ColorArg>,
    /// Restrict enabled gog top-level commands.
    #[arg(long, global = true)]
    pub enable_commands: Option<String>,
    /// Override gog binary path for development and tests.
    #[arg(long, global = true, hide = true, value_name = "PATH")]
    pub gog_bin: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorArg {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct ExtraArgs {
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        num_args = 0..
    )]
    pub extra_args: Vec<OsString>,
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct NestedArgs {
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        num_args = 1..
    )]
    pub args: Vec<OsString>,
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct TargetArgs {
    pub target: OsString,
    #[command(flatten)]
    pub extra: ExtraArgs,
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct QueryArgs {
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        num_args = 1..
    )]
    pub args: Vec<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub command_id: String,
    pub path: Vec<OsString>,
    pub args: Vec<OsString>,
}

impl Invocation {
    pub fn new(
        command_id: impl Into<String>,
        path: impl IntoIterator<Item = impl Into<OsString>>,
        args: Vec<OsString>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            path: path.into_iter().map(Into::into).collect(),
            args,
        }
    }
}

impl GlobalOptions {
    pub fn output_mode_hint(&self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else if self.plain {
            OutputMode::Plain
        } else {
            OutputMode::Human
        }
    }

    pub fn resolved_gog_bin_override(&self) -> Option<PathBuf> {
        self.gog_bin
            .clone()
            .or_else(|| env::var_os(GOOGLE_CLI_GOG_BIN_ENV).map(PathBuf::from))
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if self.json && self.plain {
            return Err(AppError::invalid_output_flags(
                "`--json` cannot be combined with `--plain`",
            ));
        }
        if !self.json && self.results_only {
            return Err(AppError::invalid_output_flags(
                "`--results-only` requires `--json`",
            ));
        }
        if !self.json && self.select.is_some() {
            return Err(AppError::invalid_output_flags(
                "`--select` requires `--json`",
            ));
        }
        Ok(())
    }

    pub fn gog_flags(&self) -> Result<Vec<OsString>, AppError> {
        self.validate()?;

        let mut flags = Vec::new();
        push_option(&mut flags, "--account", self.account.as_ref());
        push_option(&mut flags, "--client", self.client.as_ref());
        push_option(
            &mut flags,
            "--enable-commands",
            self.enable_commands.as_ref(),
        );
        push_option(
            &mut flags,
            "--color",
            self.color.map(|mode| match mode {
                ColorArg::Auto => "auto",
                ColorArg::Always => "always",
                ColorArg::Never => "never",
            }),
        );
        push_flag(&mut flags, "--json", self.json);
        push_flag(&mut flags, "--plain", self.plain);
        push_flag(&mut flags, "--results-only", self.results_only);
        push_option(&mut flags, "--select", self.select.as_ref());
        push_flag(&mut flags, "--dry-run", self.dry_run);
        push_flag(&mut flags, "--force", self.force);
        push_flag(&mut flags, "--no-input", self.no_input);
        push_flag(&mut flags, "--verbose", self.verbose);
        Ok(flags)
    }
}

pub fn dynamic_command_id(base: &str, args: &[OsString]) -> String {
    if let Some(first) = args.first() {
        let text = first.to_string_lossy();
        if !text.starts_with('-') && !text.is_empty() {
            return format!("{base}.{}", sanitize_token(text.as_ref()));
        }
    }
    base.to_string()
}

fn push_flag(flags: &mut Vec<OsString>, flag: &str, enabled: bool) {
    if enabled {
        flags.push(flag.into());
    }
}

fn push_option<T>(flags: &mut Vec<OsString>, flag: &str, value: Option<T>)
where
    T: Into<OsString>,
{
    if let Some(value) = value {
        flags.push(flag.into());
        flags.push(value.into());
    }
}

fn sanitize_token(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut previous_dash = false;

    for character in input.chars() {
        let normalized = if character.is_ascii_alphanumeric() {
            previous_dash = false;
            character.to_ascii_lowercase()
        } else if previous_dash {
            continue;
        } else {
            previous_dash = true;
            '-'
        };
        output.push(normalized);
    }

    output.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::{GlobalOptions, dynamic_command_id};

    #[test]
    fn gog_flags_validate_json_conflicts() {
        let options = GlobalOptions {
            json: true,
            plain: true,
            ..Default::default()
        };

        let error = options.gog_flags().expect_err("conflict should fail");
        assert_eq!(
            error.code(),
            crate::error::ERROR_CODE_USER_INVALID_OUTPUT_FLAGS
        );
    }

    #[test]
    fn dynamic_command_id_appends_nested_action() {
        let command = dynamic_command_id("google.auth.alias", &[std::ffi::OsString::from("set")]);
        assert_eq!(command, "google.auth.alias.set");
    }
}
