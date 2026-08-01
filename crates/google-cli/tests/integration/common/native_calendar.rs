use std::path::{Path, PathBuf};

use serde_json::Value;

pub use crate::native_gmail::{json, run, seed_account};

pub fn write_fixture(path: &Path, payload: &Value) -> PathBuf {
    let fixture_path = path.join("calendar-fixture.json");
    std::fs::write(
        &fixture_path,
        serde_json::to_vec_pretty(payload).expect("serialize fixture"),
    )
    .expect("write fixture");
    fixture_path
}

pub fn fixture_env(fixture_path: &Path) -> (&'static str, String) {
    (
        "GOOGLE_CLI_CALENDAR_FIXTURE_PATH",
        fixture_path.to_string_lossy().to_string(),
    )
}
