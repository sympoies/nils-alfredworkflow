pub mod auth;
pub mod client;
pub mod cmd;
pub mod drive;
pub mod error;
pub mod gmail;
pub mod output;
pub mod runtime;

use cmd::{Cli, Request};
use error::AppError;
use output::{ENVELOPE_SCHEMA_VERSION, OutputMode, RenderedOutput, render_error, render_success};
use runtime::Runtime;
use serde_json::json;

pub use cmd::Cli as GoogleCli;

pub fn run(cli: Cli) -> Result<RenderedOutput, AppError> {
    let request = cli.into_request()?;
    run_request(&request)
}

pub fn run_request(request: &Request) -> Result<RenderedOutput, AppError> {
    if request.invocation.command_id.starts_with("google.auth.") {
        let native = auth::execute_native(&request.global, &request.invocation)?;
        return Ok(render_native_response(
            request.invocation.command_id.as_str(),
            request.global.output_mode_hint(),
            native.payload,
            native.text,
        ));
    }

    if request.invocation.command_id.starts_with("google.gmail.") {
        let native = gmail::execute_native(&request.global, &request.invocation)?;
        return Ok(render_native_response(
            request.invocation.command_id.as_str(),
            request.global.output_mode_hint(),
            native.payload,
            native.text,
        ));
    }

    if request.invocation.command_id.starts_with("google.drive.") {
        let native = drive::execute_native(&request.global, &request.invocation)?;
        return Ok(render_native_response(
            request.invocation.command_id.as_str(),
            request.global.output_mode_hint(),
            native.payload,
            native.text,
        ));
    }

    let runtime = Runtime::from_global(&request.global)?;
    let process = runtime.execute(&request.global, &request.invocation)?;
    render_success(
        request.invocation.command_id.as_str(),
        request.global.output_mode_hint(),
        process,
    )
}

pub fn render_failure(cli: &Cli, error: &AppError) -> RenderedOutput {
    render_error(cli.command_id_hint(), cli.output_mode_hint(), error)
}

fn render_native_response(
    command_id: &str,
    mode: OutputMode,
    payload: serde_json::Value,
    text: String,
) -> RenderedOutput {
    match mode {
        OutputMode::Json => RenderedOutput {
            stdout: json!({
                "schema_version": ENVELOPE_SCHEMA_VERSION,
                "command": command_id,
                "ok": true,
                "result": payload,
            })
            .to_string(),
            stderr: String::new(),
        },
        OutputMode::Human | OutputMode::Plain => RenderedOutput {
            stdout: format!("{text}\n"),
            stderr: String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cmd::Cli;

    #[test]
    fn cli_routes_auth_command_into_gog_args() {
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
        let argv = request.global.gog_flags().expect("flags");
        let mut full = argv;
        full.extend(request.invocation.path.clone());
        full.extend(request.invocation.args.clone());

        let rendered = full
            .iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                "--account",
                "me@example.com",
                "--json",
                "auth",
                "add",
                "me@example.com",
                "--manual",
            ]
        );
        assert_eq!(request.invocation.command_id, "google.auth.add");
    }
}
