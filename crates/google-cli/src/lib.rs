pub mod cmd;
pub mod error;
pub mod output;
pub mod runtime;

use cmd::{Cli, Request};
use error::AppError;
use output::{RenderedOutput, render_error, render_success};
use runtime::Runtime;

pub use cmd::Cli as GoogleCli;

pub fn run(cli: Cli) -> Result<RenderedOutput, AppError> {
    let request = cli.into_request()?;
    run_request(&request)
}

pub fn run_request(request: &Request) -> Result<RenderedOutput, AppError> {
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
