use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime};

use crate::error::WorkflowError;

const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Debug, Clone, Default)]
pub struct UsageLog {
    entries: HashMap<String, String>,
}

impl UsageLog {
    pub fn load(path: &Path) -> Self {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => return Self::default(),
        };

        let mut entries = HashMap::new();
        for line in content.lines() {
            let Some((key, timestamp)) = line.split_once('|') else {
                continue;
            };

            let key = key.trim();
            let timestamp = timestamp.trim();
            if key.is_empty() || timestamp.is_empty() {
                continue;
            }

            // Keep the most recent occurrence in the file for each key.
            entries.insert(key.to_string(), timestamp.to_string());
        }

        Self { entries }
    }

    pub fn timestamp_for(&self, project_path: &Path, project_name: &str) -> Option<&str> {
        let path_key = project_path.to_string_lossy();
        self.entries
            .get(path_key.as_ref())
            .map(String::as_str)
            .or_else(|| self.entries.get(project_name).map(String::as_str))
    }
}

pub fn parse_usage_timestamp(raw: Option<&str>) -> i64 {
    raw.and_then(|timestamp| NaiveDateTime::parse_from_str(timestamp, TIMESTAMP_FORMAT).ok())
        .map(|value| value.and_utc().timestamp())
        .unwrap_or(0)
}

pub fn record_usage(project_path: &Path, usage_file: &Path) -> Result<(), WorkflowError> {
    let project_path_string = project_path.to_string_lossy().to_string();
    let project_name = project_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    let existing = fs::read_to_string(usage_file).unwrap_or_default();
    let mut lines = Vec::new();

    for line in existing.lines() {
        let Some((key, _)) = line.split_once('|') else {
            continue;
        };

        let key = key.trim();
        if key != project_path_string && key != project_name {
            lines.push(line.to_string());
        }
    }

    let timestamp = Local::now().format(TIMESTAMP_FORMAT).to_string();
    lines.push(format!("{project_path_string} | {timestamp}"));

    let output = format!("{}\n", lines.join("\n"));

    let parent = usage_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&parent).map_err(|source| WorkflowError::UsageWrite {
        path: usage_file.to_path_buf(),
        source,
    })?;

    // Use a per-process temp name so two concurrent `record-usage` invocations
    // (Alfred can dispatch an action while another runs) cannot write the same
    // temp path and corrupt/truncate each other's content. The final rename is
    // atomic, so the log always ends up as one complete write rather than an
    // interleaved one.
    let temp_file = usage_file.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&temp_file, output).map_err(|source| WorkflowError::UsageWrite {
        path: temp_file.clone(),
        source,
    })?;

    fs::rename(&temp_file, usage_file).map_err(|source| WorkflowError::UsageWrite {
        path: usage_file.to_path_buf(),
        source,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn usage_log_prefers_path_key_then_name_fallback() {
        let temp = tempdir().expect("create temp dir");
        let log_path = temp.path().join("usage.log");
        let project_path = temp.path().join("projects/alpha");
        fs::create_dir_all(&project_path).expect("create project dir");

        fs::write(
            &log_path,
            format!(
                "alpha | 2024-01-01 00:00:00\n{} | 2025-01-02 03:04:05\n",
                project_path.to_string_lossy()
            ),
        )
        .expect("write usage log");

        let usage = UsageLog::load(&log_path);
        let timestamp = usage.timestamp_for(&project_path, "alpha");

        assert_eq!(
            timestamp,
            Some("2025-01-02 03:04:05"),
            "path key should override basename key when both exist"
        );
    }

    #[test]
    fn usage_log_keeps_latest_occurrence_for_same_key() {
        let temp = tempdir().expect("create temp dir");
        let log_path = temp.path().join("usage.log");
        let project_path = temp.path().join("projects/beta");
        fs::create_dir_all(&project_path).expect("create project dir");

        fs::write(
            &log_path,
            format!(
                "{} | 2023-01-01 00:00:00\n{} | 2024-03-05 06:07:08\n",
                project_path.to_string_lossy(),
                project_path.to_string_lossy()
            ),
        )
        .expect("write usage log");

        let usage = UsageLog::load(&log_path);
        assert_eq!(
            usage.timestamp_for(&project_path, "beta"),
            Some("2024-03-05 06:07:08"),
            "latest duplicate entry should win"
        );
    }

    #[test]
    fn usage_log_record_usage_replaces_legacy_name_key() {
        let temp = tempdir().expect("create temp dir");
        let usage_file = temp.path().join("logs/usage.log");
        let project_path = temp.path().join("workspace/gamma");
        fs::create_dir_all(&project_path).expect("create project dir");

        fs::create_dir_all(usage_file.parent().expect("usage parent"))
            .expect("create usage parent");
        fs::write(&usage_file, "gamma | 2024-01-01 00:00:00\n").expect("seed legacy usage");

        record_usage(&project_path, &usage_file).expect("record usage should succeed");

        let content = fs::read_to_string(&usage_file).expect("read usage file");
        assert!(
            !content.contains("gamma | 2024-01-01 00:00:00"),
            "legacy basename entry should be removed"
        );
        assert!(
            content.contains(project_path.to_string_lossy().as_ref()),
            "path key should be written"
        );
    }

    #[test]
    fn usage_log_parse_timestamp_invalid_falls_back_to_zero() {
        let parsed = parse_usage_timestamp(Some("invalid"));
        assert_eq!(parsed, 0, "invalid timestamp should map to sort key zero");

        let parsed = parse_usage_timestamp(Some("2025-01-02 03:04:05"));
        assert!(
            parsed > 0,
            "valid timestamp should map to positive sort key"
        );
    }
}
