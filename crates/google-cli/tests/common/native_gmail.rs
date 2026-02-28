use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

pub fn run(config_dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_google-cli"));
    command.args(args);
    command.env("GOOGLE_CLI_CONFIG_DIR", config_dir);
    command.env("GOOGLE_CLI_KEYRING_MODE", "file");
    command.env("GOOGLE_CLI_AUTH_DISABLE_BROWSER", "1");
    command.env("PATH", config_dir);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("run google-cli")
}

pub fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be json")
}

pub fn seed_credentials(config_dir: &Path) {
    let output = run(
        config_dir,
        &[
            "--json",
            "auth",
            "credentials",
            "set",
            "--client-id",
            "client-id",
            "--client-secret",
            "client-secret",
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(0));
}

#[allow(dead_code)]
pub fn seed_account(config_dir: &Path, account: &str) {
    seed_credentials(config_dir);
    let output = run(
        config_dir,
        &[
            "--json",
            "auth",
            "add",
            account,
            "--manual",
            "--code",
            "manual-code",
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(0));
}

pub fn write_fixture(path: &Path, payload: &Value) -> PathBuf {
    let fixture_path = path.join("gmail-fixture.json");
    std::fs::write(
        &fixture_path,
        serde_json::to_vec_pretty(payload).expect("serialize fixture"),
    )
    .expect("write fixture");
    fixture_path
}
