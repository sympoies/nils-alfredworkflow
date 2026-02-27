use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::{TempDir, tempdir};

pub struct TestHarness {
    _temp: TempDir,
    gog_bin: PathBuf,
    log_path: PathBuf,
}

#[allow(dead_code)]
impl TestHarness {
    pub fn new() -> Self {
        let temp = tempdir().expect("create tempdir");
        let gog_bin = temp.path().join("gog");
        let log_path = temp.path().join("fake-gog.log");
        let fixture = fixture_path();

        fs::copy(&fixture, &gog_bin).expect("copy fake gog fixture");
        let mut permissions = fs::metadata(&gog_bin).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&gog_bin, permissions).expect("chmod");

        Self {
            _temp: temp,
            gog_bin,
            log_path,
        }
    }

    pub fn run(&self, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_google-cli"));
        command.args(args);
        command.env("GOOGLE_CLI_GOG_BIN", &self.gog_bin);
        command.env("FAKE_GOG_LOG", &self.log_path);
        for (key, value) in envs {
            command.env(key, value);
        }
        command.output().expect("run google-cli")
    }

    pub fn logged_args(&self) -> Vec<String> {
        if !self.log_path.exists() {
            return Vec::new();
        }
        fs::read_to_string(&self.log_path)
            .expect("read fake gog log")
            .lines()
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn missing_gog_path(&self) -> String {
        self._temp.path().join("missing-gog").display().to_string()
    }
}

pub fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake_gog.sh")
}
