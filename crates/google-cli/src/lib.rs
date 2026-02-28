pub mod auth;
pub mod client;
pub mod cmd;
pub mod drive;
pub mod error;
pub mod gmail;
pub mod output;

use cmd::{Cli, Request};
use error::AppError;
use output::{RenderedOutput, render_error, render_success};

pub use cmd::Cli as GoogleCli;

pub fn run(cli: Cli) -> Result<RenderedOutput, AppError> {
    let request = cli.into_request()?;
    run_request(&request)
}

pub fn run_request(request: &Request) -> Result<RenderedOutput, AppError> {
    if request.invocation.command_id.starts_with("google.auth.") {
        let native = auth::execute_native(&request.global, &request.invocation)?;
        return Ok(render_success(
            request.invocation.command_id.as_str(),
            request.global.output_mode_hint(),
            native.payload,
            native.text.as_str(),
        ));
    }

    if request.invocation.command_id.starts_with("google.gmail.") {
        let native = gmail::execute_native(&request.global, &request.invocation)?;
        return Ok(render_success(
            request.invocation.command_id.as_str(),
            request.global.output_mode_hint(),
            native.payload,
            native.text.as_str(),
        ));
    }

    if request.invocation.command_id.starts_with("google.drive.") {
        let native = drive::execute_native(&request.global, &request.invocation)?;
        return Ok(render_success(
            request.invocation.command_id.as_str(),
            request.global.output_mode_hint(),
            native.payload,
            native.text.as_str(),
        ));
    }

    Err(AppError::invalid_auth_input(format!(
        "unsupported command id `{}`",
        request.invocation.command_id
    )))
}

pub fn render_failure(cli: &Cli, error: &AppError) -> RenderedOutput {
    render_error(cli.command_id_hint(), cli.output_mode_hint(), error)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cmd::Cli;

    #[test]
    fn cli_routes_auth_command_into_native_invocation() {
        let cli = Cli::parse_from([
            "google-cli",
            "--account",
            "me@example.com",
            "--json",
            "auth",
            "add",
            "me@example.com",
            "--manual",
        ]);

        let request = cli.into_request().expect("request");
        assert_eq!(request.global.account.as_deref(), Some("me@example.com"));
        assert!(request.global.json);
        assert_eq!(request.invocation.path, vec!["auth", "add"]);
        assert_eq!(request.invocation.args, vec!["me@example.com", "--manual"]);
        assert_eq!(request.invocation.command_id, "google.auth.add");
    }
}
