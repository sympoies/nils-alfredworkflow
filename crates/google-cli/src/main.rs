use clap::Parser;

use google_cli::{GoogleCli, render_failure, run};

// `--json` output is serialized in output.rs via serde_json::to_string(...)
// when OutputMode::Json is selected by the parsed CLI flags.
fn main() {
    let cli = GoogleCli::parse();

    match run(cli.clone()) {
        Ok(output) => output.emit(),
        Err(error) => {
            render_failure(&cli, &error).emit();
            std::process::exit(error.exit_code());
        }
    }
}
