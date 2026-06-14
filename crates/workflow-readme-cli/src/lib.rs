use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub use workflow_common::{AppError, CliErrorKind as ErrorKind};

const ERROR_CODE_USER_INVALID_WORKFLOW_ROOT: &str = "NILS_WORKFLOW_README_001";
const ERROR_CODE_USER_INVALID_README_SOURCE: &str = "NILS_WORKFLOW_README_002";
const ERROR_CODE_USER_README_NOT_FOUND: &str = "NILS_WORKFLOW_README_003";
const ERROR_CODE_USER_PLIST_NOT_FOUND: &str = "NILS_WORKFLOW_README_004";
const ERROR_CODE_USER_REMOTE_IMAGE_NOT_ALLOWED: &str = "NILS_WORKFLOW_README_005";
const ERROR_CODE_USER_INVALID_IMAGE_PATH: &str = "NILS_WORKFLOW_README_006";
const ERROR_CODE_USER_IMAGE_NOT_FOUND: &str = "NILS_WORKFLOW_README_007";
const ERROR_CODE_USER_PLIST_README_KEY_MISSING: &str = "NILS_WORKFLOW_README_008";
const ERROR_CODE_RUNTIME_READ_FAILED: &str = "NILS_WORKFLOW_README_009";
const ERROR_CODE_RUNTIME_WRITE_FAILED: &str = "NILS_WORKFLOW_README_010";
const ERROR_CODE_RUNTIME_CREATE_DIR_FAILED: &str = "NILS_WORKFLOW_README_011";
const ERROR_CODE_RUNTIME_COPY_FAILED: &str = "NILS_WORKFLOW_README_012";

#[derive(Debug, Clone)]
pub struct ConvertRequest {
    pub workflow_root: PathBuf,
    pub readme_source: PathBuf,
    pub stage_dir: PathBuf,
    pub plist: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertOutput {
    pub converted_readme: String,
    pub copied_assets: Vec<PathBuf>,
}

pub fn convert(request: &ConvertRequest) -> Result<ConvertOutput, AppError> {
    validate_workflow_root(&request.workflow_root)?;
    validate_relative_readme_source(&request.readme_source)?;

    let readme_path = request.workflow_root.join(&request.readme_source);
    if !readme_path.exists() || !readme_path.is_file() {
        return Err(AppError::user(
            ERROR_CODE_USER_README_NOT_FOUND,
            format!("README source not found: {}", readme_path.display()),
        ));
    }

    if !request.plist.exists() || !request.plist.is_file() {
        return Err(AppError::user(
            ERROR_CODE_USER_PLIST_NOT_FOUND,
            format!("plist path not found: {}", request.plist.display()),
        ));
    }

    let readme_markdown = fs::read_to_string(&readme_path).map_err(|error| {
        AppError::runtime(
            ERROR_CODE_RUNTIME_READ_FAILED,
            format!("failed to read README {}: {error}", readme_path.display()),
        )
    })?;

    let converted_readme = downgrade_markdown_tables(&readme_markdown);
    let image_targets = extract_markdown_image_targets(&converted_readme)?;
    let copied_assets = stage_local_images(
        &request.workflow_root,
        &request.stage_dir,
        &image_targets,
        request.dry_run,
    )?;

    let plist_contents = fs::read_to_string(&request.plist).map_err(|error| {
        AppError::runtime(
            ERROR_CODE_RUNTIME_READ_FAILED,
            format!("failed to read plist {}: {error}", request.plist.display()),
        )
    })?;
    let updated_plist = inject_readme_into_plist(&plist_contents, &converted_readme)?;

    if !request.dry_run {
        fs::write(&request.plist, updated_plist).map_err(|error| {
            AppError::runtime(
                ERROR_CODE_RUNTIME_WRITE_FAILED,
                format!("failed to write plist {}: {error}", request.plist.display()),
            )
        })?;
    }

    Ok(ConvertOutput {
        converted_readme,
        copied_assets,
    })
}

fn validate_workflow_root(workflow_root: &Path) -> Result<(), AppError> {
    if !workflow_root.exists() {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_WORKFLOW_ROOT,
            format!("workflow root does not exist: {}", workflow_root.display()),
        ));
    }
    if !workflow_root.is_dir() {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_WORKFLOW_ROOT,
            format!(
                "workflow root is not a directory: {}",
                workflow_root.display()
            ),
        ));
    }
    Ok(())
}

fn validate_relative_readme_source(readme_source: &Path) -> Result<(), AppError> {
    if readme_source.is_absolute() || readme_source.as_os_str().is_empty() {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_README_SOURCE,
            format!(
                "--readme-source must be a non-empty relative path: {}",
                readme_source.display()
            ),
        ));
    }

    for component in readme_source.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(AppError::user(
                ERROR_CODE_USER_INVALID_README_SOURCE,
                format!(
                    "--readme-source cannot contain parent/root components: {}",
                    readme_source.display()
                ),
            ));
        }
    }

    Ok(())
}

pub fn downgrade_markdown_tables(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut output: Vec<String> = Vec::with_capacity(lines.len());
    let mut index = 0usize;
    let mut in_fence = false;

    while index < lines.len() {
        let line = lines[index];

        if is_fence_delimiter(line) {
            in_fence = !in_fence;
            output.push(line.to_string());
            index += 1;
            continue;
        }

        if !in_fence
            && let Some(headers) = parse_table_row(line)
            && index + 1 < lines.len()
            && is_table_separator_line(lines[index + 1])
        {
            index += 2;

            let mut rows: Vec<Vec<String>> = Vec::new();
            while index < lines.len() {
                let row_line = lines[index];
                if row_line.trim().is_empty() {
                    break;
                }
                if is_fence_delimiter(row_line) {
                    break;
                }
                if is_table_separator_line(row_line) {
                    index += 1;
                    continue;
                }
                if let Some(row) = parse_table_row(row_line) {
                    rows.push(row);
                    index += 1;
                    continue;
                }
                break;
            }

            if rows.is_empty() {
                output.push(format!("- {}", format_table_row(&headers, &[])));
            } else {
                for row in rows {
                    output.push(format!("- {}", format_table_row(&headers, &row)));
                }
            }
            continue;
        }

        output.push(line.to_string());
        index += 1;
    }

    output.join("\n")
}

fn is_fence_delimiter(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn parse_table_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return None;
    }

    let without_leading = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let without_trailing = without_leading.strip_suffix('|').unwrap_or(without_leading);
    let cells: Vec<String> = without_trailing
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect();
    if cells.len() < 2 {
        return None;
    }
    Some(cells)
}

fn is_table_separator_line(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return false;
    }

    let candidate = trimmed.trim_matches('|');
    if candidate.is_empty() {
        return false;
    }

    let mut has_any = false;
    for cell in candidate.split('|') {
        let cell = cell.trim();
        if cell.is_empty() {
            return false;
        }
        if !cell.chars().all(|ch| ch == '-' || ch == ':') {
            return false;
        }
        if !cell.contains('-') {
            return false;
        }
        has_any = true;
    }
    has_any
}

fn format_table_row(headers: &[String], row: &[String]) -> String {
    let width = headers.len().max(row.len());
    let mut parts: Vec<String> = Vec::with_capacity(width);
    for idx in 0..width {
        let label = headers
            .get(idx)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("Column {}", idx + 1));
        let value = row
            .get(idx)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or("(empty)");
        parts.push(format!("{label}: {value}"));
    }
    parts.join("; ")
}

pub fn extract_markdown_image_targets(markdown: &str) -> Result<Vec<String>, AppError> {
    let bytes = markdown.as_bytes();
    let mut index = 0usize;
    let mut targets: Vec<String> = Vec::new();

    while index + 1 < bytes.len() {
        if bytes[index] == b'!' && bytes[index + 1] == b'[' {
            let Some(alt_end) = find_unescaped_byte(bytes, index + 2, b']') else {
                index += 2;
                continue;
            };

            if alt_end + 1 >= bytes.len() || bytes[alt_end + 1] != b'(' {
                index = alt_end + 1;
                continue;
            }

            let Some(target_end) = find_unescaped_byte(bytes, alt_end + 2, b')') else {
                return Err(AppError::user(
                    ERROR_CODE_USER_INVALID_IMAGE_PATH,
                    "malformed markdown image: missing closing ')'".to_string(),
                ));
            };

            let raw_target = &markdown[alt_end + 2..target_end];
            let destination = parse_image_destination(raw_target)?;
            targets.push(destination);

            index = target_end + 1;
            continue;
        }

        index += 1;
    }

    Ok(targets)
}

fn find_unescaped_byte(bytes: &[u8], mut index: usize, target: u8) -> Option<usize> {
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = index.saturating_add(2);
            continue;
        }
        if bytes[index] == target {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn parse_image_destination(raw_target: &str) -> Result<String, AppError> {
    let trimmed = raw_target.trim();
    if trimmed.is_empty() {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_IMAGE_PATH,
            "markdown image has an empty destination".to_string(),
        ));
    }

    let destination = if trimmed.starts_with('<') {
        let Some(end_index) = trimmed.find('>') else {
            return Err(AppError::user(
                ERROR_CODE_USER_INVALID_IMAGE_PATH,
                format!("invalid angle-bracket image destination: {trimmed}"),
            ));
        };
        trimmed[1..end_index].trim()
    } else {
        trimmed.split_whitespace().next().unwrap_or("")
    };

    if destination.is_empty() {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_IMAGE_PATH,
            format!("invalid image destination: {trimmed}"),
        ));
    }

    Ok(destination.to_string())
}

fn stage_local_images(
    workflow_root: &Path,
    stage_dir: &Path,
    image_targets: &[String],
    dry_run: bool,
) -> Result<Vec<PathBuf>, AppError> {
    let mut copied: BTreeSet<PathBuf> = BTreeSet::new();

    for target in image_targets {
        if is_remote_image_target(target) {
            return Err(AppError::user(
                ERROR_CODE_USER_REMOTE_IMAGE_NOT_ALLOWED,
                format!("remote image URL is not allowed: {target}"),
            ));
        }

        let relative_path = normalize_local_image_path(target)?;
        let source_path = workflow_root.join(&relative_path);
        if !source_path.exists() || !source_path.is_file() {
            return Err(AppError::user(
                ERROR_CODE_USER_IMAGE_NOT_FOUND,
                format!("local image not found: {}", source_path.display()),
            ));
        }

        let destination_path = stage_dir.join(&relative_path);
        if !dry_run {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    AppError::runtime(
                        ERROR_CODE_RUNTIME_CREATE_DIR_FAILED,
                        format!("failed to create directory {}: {error}", parent.display()),
                    )
                })?;
            }
            fs::copy(&source_path, &destination_path).map_err(|error| {
                AppError::runtime(
                    ERROR_CODE_RUNTIME_COPY_FAILED,
                    format!(
                        "failed to copy image {} -> {}: {error}",
                        source_path.display(),
                        destination_path.display()
                    ),
                )
            })?;
        }

        copied.insert(relative_path);
    }

    Ok(copied.into_iter().collect())
}

fn normalize_local_image_path(target: &str) -> Result<PathBuf, AppError> {
    if target.contains('?') || target.contains('#') {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_IMAGE_PATH,
            format!("image path cannot contain query/fragment: {target}"),
        ));
    }

    let path = Path::new(target);
    if path.is_absolute() {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_IMAGE_PATH,
            format!("image path must be relative: {target}"),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            Component::ParentDir => {
                return Err(AppError::user(
                    ERROR_CODE_USER_INVALID_IMAGE_PATH,
                    format!("image path cannot use '..': {target}"),
                ));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(AppError::user(
                    ERROR_CODE_USER_INVALID_IMAGE_PATH,
                    format!("image path must stay within workflow root: {target}"),
                ));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(AppError::user(
            ERROR_CODE_USER_INVALID_IMAGE_PATH,
            format!("image path is empty after normalization: {target}"),
        ));
    }

    Ok(normalized)
}

fn is_remote_image_target(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("//") {
        return true;
    }
    if let Some((scheme, _)) = target.split_once(':')
        && !scheme.is_empty()
        && scheme
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
    {
        return true;
    }
    false
}

pub fn inject_readme_into_plist(plist: &str, readme_markdown: &str) -> Result<String, AppError> {
    let key_marker = "<key>readme</key>";
    let open_marker = "<string>";
    let close_marker = "</string>";

    let Some(key_index) = plist.find(key_marker) else {
        return Err(AppError::user(
            ERROR_CODE_USER_PLIST_README_KEY_MISSING,
            "plist does not contain <key>readme</key>".to_string(),
        ));
    };

    let search_start = key_index + key_marker.len();
    let Some(open_rel) = plist[search_start..].find(open_marker) else {
        return Err(AppError::user(
            ERROR_CODE_USER_PLIST_README_KEY_MISSING,
            "plist readme key is missing a <string> value".to_string(),
        ));
    };
    let open_index = search_start + open_rel;
    let content_start = open_index + open_marker.len();

    let Some(close_rel) = plist[content_start..].find(close_marker) else {
        return Err(AppError::user(
            ERROR_CODE_USER_PLIST_README_KEY_MISSING,
            "plist readme value is missing </string>".to_string(),
        ));
    };
    let content_end = content_start + close_rel;

    let escaped_readme = escape_xml_text(readme_markdown);
    let mut updated = String::with_capacity(plist.len() + escaped_readme.len());
    updated.push_str(&plist[..content_start]);
    updated.push_str(&escaped_readme);
    updated.push_str(&plist[content_end..]);
    Ok(updated)
}

pub fn escape_xml_text(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escape_handles_reserved_chars() {
        let escaped = escape_xml_text(r#"<tag a="1">Tom & Jerry</tag>"#);
        assert_eq!(
            escaped,
            "&lt;tag a=&quot;1&quot;&gt;Tom &amp; Jerry&lt;/tag&gt;"
        );
    }

    #[test]
    fn inject_readme_replaces_existing_value() {
        let plist = "<dict><key>readme</key><string>old</string></dict>";
        let injected = inject_readme_into_plist(plist, "hello & <world>")
            .expect("injecting readme should succeed");
        assert_eq!(
            injected,
            "<dict><key>readme</key><string>hello &amp; &lt;world&gt;</string></dict>"
        );
    }
}
