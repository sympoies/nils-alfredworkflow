use std::path::Path;
use std::process::Command;

use crate::error::WorkflowError;

pub fn last_commit_summary(project_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("log")
        .arg("-1")
        .arg("--pretty=format:%s (by %an, %ad)")
        .arg("--date=short")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

pub fn github_url_for_project(project_path: &Path) -> Result<String, WorkflowError> {
    if !project_path.exists() {
        return Err(WorkflowError::MissingPath(project_path.to_path_buf()));
    }
    if !project_path.is_dir() {
        return Err(WorkflowError::NotDirectory(project_path.to_path_buf()));
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .output()
        .map_err(|error| WorkflowError::GitCommand {
            path: project_path.to_path_buf(),
            message: error.to_string(),
        })?;

    if !output.status.success() {
        return Err(WorkflowError::MissingOrigin(project_path.to_path_buf()));
    }

    let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if remote_url.is_empty() {
        return Err(WorkflowError::MissingOrigin(project_path.to_path_buf()));
    }

    normalize_github_remote(&remote_url)
}

pub fn normalize_github_remote(remote_url: &str) -> Result<String, WorkflowError> {
    let normalized = if let Some(rest) = remote_url.strip_prefix("git@github.com:") {
        normalize_repo_path(rest)
    } else if let Some(rest) = remote_url.strip_prefix("ssh://git@github.com/") {
        normalize_repo_path(rest)
    } else if let Some(rest) = remote_url.strip_prefix("https://github.com/") {
        normalize_repo_path(rest)
    } else {
        return Err(WorkflowError::UnsupportedRemote(remote_url.to_string()));
    };

    if normalized.split('/').count() != 2 {
        return Err(WorkflowError::UnsupportedRemote(remote_url.to_string()));
    }

    Ok(format!("https://github.com/{normalized}"))
}

fn normalize_repo_path(raw: &str) -> String {
    raw.trim_end_matches('/')
        .trim_end_matches(".git")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn github_remote_normalizes_ssh_and_https_formats() {
        let ssh = normalize_github_remote("git@github.com:owner/repo.git")
            .expect("ssh remote should normalize");
        assert_eq!(ssh, "https://github.com/owner/repo");

        let ssh_url = normalize_github_remote("ssh://git@github.com/owner/repo.git")
            .expect("ssh url remote should normalize");
        assert_eq!(ssh_url, "https://github.com/owner/repo");

        let https = normalize_github_remote("https://github.com/owner/repo.git")
            .expect("https remote should normalize");
        assert_eq!(https, "https://github.com/owner/repo");

        let https_no_suffix =
            normalize_github_remote("https://github.com/owner/repo").expect("suffix-less remote");
        assert_eq!(https_no_suffix, "https://github.com/owner/repo");
    }

    #[test]
    fn github_remote_rejects_unsupported_formats() {
        let err = normalize_github_remote("ssh://git@example.com/org/repo.git")
            .expect_err("unsupported remote should fail");
        assert!(
            matches!(err, WorkflowError::UnsupportedRemote(_)),
            "expected unsupported remote error"
        );
    }

    #[test]
    fn github_remote_reports_missing_origin() {
        let temp = tempdir().expect("create temp dir");
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).expect("create repo dir");

        let status = Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(&repo)
            .status()
            .expect("run git init");
        assert!(status.success(), "git init should succeed");

        let err = github_url_for_project(&repo).expect_err("missing origin should fail");
        assert!(
            matches!(err, WorkflowError::MissingOrigin(_)),
            "expected missing origin error"
        );
    }
}
