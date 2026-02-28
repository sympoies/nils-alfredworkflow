use std::ffi::OsString;

use clap::Args;

use crate::error::AppError;
use crate::output::OutputMode;

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct GlobalOptions {
    /// Account email for auth-scoped commands.
    #[arg(short = 'a', long, global = true)]
    pub account: Option<String>,
    /// Emit machine-readable JSON envelope output.
    #[arg(short = 'j', long, global = true, default_value_t = false)]
    pub json: bool,
    /// Emit stable plain-text output.
    #[arg(short = 'p', long, global = true, default_value_t = false)]
    pub plain: bool,
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

    pub fn validate(&self) -> Result<(), AppError> {
        if self.json && self.plain {
            return Err(AppError::invalid_output_flags(
                "`--json` cannot be combined with `--plain`",
            ));
        }
        Ok(())
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
    fn validate_rejects_conflicting_output_modes() {
        let options = GlobalOptions {
            json: true,
            plain: true,
            ..Default::default()
        };

        let error = options.validate().expect_err("conflict should fail");
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
