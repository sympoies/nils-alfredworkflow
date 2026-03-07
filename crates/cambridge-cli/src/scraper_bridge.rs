use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use thiserror::Error;

use crate::config::RuntimeConfig;

const POLL_INTERVAL_MS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScraperStage {
    Suggest,
    Define,
}

impl ScraperStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            ScraperStage::Suggest => "suggest",
            ScraperStage::Define => "define",
        }
    }

    const fn query_flag(self) -> &'static str {
        match self {
            ScraperStage::Suggest => "--query",
            ScraperStage::Define => "--entry",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "suggest" => Some(ScraperStage::Suggest),
            "define" => Some(ScraperStage::Define),
            _ => None,
        }
    }
}

impl std::fmt::Display for ScraperStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestItem {
    pub word: String,
    pub subtitle: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionLine {
    pub text: String,
    pub part_of_speech: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub headword: String,
    pub part_of_speech: Option<String>,
    pub phonetics: Option<String>,
    pub url: Option<String>,
    pub definitions: Vec<DefinitionLine>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScraperErrorInfo {
    pub code: Option<String>,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScraperResponse {
    pub ok: bool,
    pub stage: ScraperStage,
    pub items: Vec<SuggestItem>,
    pub entry: Option<Entry>,
    pub error: Option<ScraperErrorInfo>,
}

pub fn run_scraper(
    config: &RuntimeConfig,
    stage: ScraperStage,
    term: &str,
) -> Result<ScraperResponse, BridgeError> {
    let args = build_scraper_args(config, stage, term);
    let mut child = Command::new(&config.node_bin)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| BridgeError::Spawn {
            program: config.node_bin.clone(),
            source,
        })?;

    let deadline = Instant::now() + Duration::from_millis(config.timeout_ms);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(BridgeError::Timeout {
                        timeout_ms: config.timeout_ms,
                    });
                }
                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(source) => return Err(BridgeError::Wait { source }),
        }
    };

    let stdout_bytes = if let Some(stdout) = child.stdout.take() {
        read_child_pipe(stdout).map_err(|source| BridgeError::ReadStdout { source })?
    } else {
        Vec::new()
    };

    let stderr_bytes = if let Some(stderr) = child.stderr.take() {
        read_child_pipe(stderr).map_err(|source| BridgeError::ReadStderr { source })?
    } else {
        Vec::new()
    };

    if !status.success() {
        if let Ok(stdout) = String::from_utf8(stdout_bytes.clone())
            && let Ok(decoded) = decode_scraper_json(&stdout, stage)
        {
            return Ok(decoded);
        }

        let stderr = normalize_string(String::from_utf8_lossy(&stderr_bytes).as_ref())
            .unwrap_or_else(|| "no stderr output".to_string());
        return Err(BridgeError::NonZeroExit {
            code: status.code(),
            stderr,
        });
    }

    let stdout = String::from_utf8(stdout_bytes).map_err(|_| BridgeError::InvalidUtf8Stdout)?;
    decode_scraper_json(&stdout, stage)
}

pub(crate) fn build_scraper_args(
    config: &RuntimeConfig,
    stage: ScraperStage,
    term: &str,
) -> Vec<String> {
    let mut args = vec![
        config.scraper_script.to_string_lossy().into_owned(),
        stage.as_str().to_string(),
        "--mode".to_string(),
        config.dict_mode.as_str().to_string(),
        "--max-results".to_string(),
        config.max_results.to_string(),
        "--timeout-ms".to_string(),
        config.timeout_ms.to_string(),
        "--headless".to_string(),
        if config.headless {
            "true".to_string()
        } else {
            "false".to_string()
        },
        stage.query_flag().to_string(),
        term.to_string(),
    ];
    args.shrink_to_fit();
    args
}

pub fn decode_scraper_json(
    raw_json: &str,
    expected_stage: ScraperStage,
) -> Result<ScraperResponse, BridgeError> {
    let raw: RawResponse = serde_json::from_str(raw_json)
        .map_err(|error| BridgeError::InvalidJson(error.to_string()))?;

    let stage = match raw.stage {
        Some(value) => {
            let normalized = normalize_string(value).unwrap_or_default();
            let parsed = ScraperStage::parse(&normalized)
                .ok_or_else(|| BridgeError::UnsupportedStage(normalized.clone()))?;
            if parsed != expected_stage {
                return Err(BridgeError::StageMismatch {
                    expected: expected_stage.as_str().to_string(),
                    actual: parsed.as_str().to_string(),
                });
            }
            parsed
        }
        None => expected_stage,
    };

    let items = raw
        .items
        .into_iter()
        .filter_map(normalize_suggest_item)
        .collect();
    let entry = raw.entry.map(normalize_entry);
    let error = raw.error.map(normalize_error);

    Ok(ScraperResponse {
        ok: raw.ok,
        stage,
        items,
        entry,
        error,
    })
}

fn read_child_pipe<R: Read>(mut reader: R) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    reader.read_to_end(&mut output)?;
    Ok(output)
}

fn normalize_suggest_item(raw: RawSuggestItem) -> Option<SuggestItem> {
    match raw {
        RawSuggestItem::Word(word) => {
            let word = normalize_string(word)?;
            Some(SuggestItem {
                word,
                subtitle: None,
                url: None,
            })
        }
        RawSuggestItem::Object {
            word,
            headword,
            entry,
            title,
            label,
            subtitle,
            pos,
            url,
        } => {
            let word = first_non_empty([word, headword, entry, title, label])?;
            Some(SuggestItem {
                word,
                subtitle: first_non_empty([subtitle, pos]),
                url: normalize_optional_string(url),
            })
        }
    }
}

fn normalize_entry(raw: RawEntry) -> Entry {
    Entry {
        headword: first_non_empty([raw.headword, raw.word, raw.term]).unwrap_or_default(),
        part_of_speech: first_non_empty([raw.part_of_speech, raw.pos]),
        phonetics: first_non_empty([
            normalize_string_or_list(raw.phonetics),
            normalize_string_or_list(raw.ipa),
            normalize_string_or_list(raw.pronunciation),
        ]),
        url: first_non_empty([raw.url, raw.link]),
        definitions: raw
            .definitions
            .into_iter()
            .filter_map(normalize_definition)
            .collect(),
        examples: raw
            .examples
            .into_iter()
            .filter_map(normalize_string)
            .collect(),
    }
}

fn normalize_definition(raw: RawDefinition) -> Option<DefinitionLine> {
    match raw {
        RawDefinition::Text(text) => {
            let text = normalize_string(text)?;
            Some(DefinitionLine {
                text,
                part_of_speech: None,
            })
        }
        RawDefinition::Object {
            text,
            definition,
            gloss,
            title,
            pos,
            part_of_speech,
        } => {
            let text = first_non_empty([text, definition, gloss, title])?;
            Some(DefinitionLine {
                text,
                part_of_speech: first_non_empty([part_of_speech, pos]),
            })
        }
    }
}

fn normalize_error(raw: RawError) -> ScraperErrorInfo {
    match raw {
        RawError::Message(message) => ScraperErrorInfo {
            code: None,
            message: normalize_string(message)
                .unwrap_or_else(|| "unknown scraper error".to_string()),
            hint: None,
        },
        RawError::Object {
            code,
            message,
            hint,
            detail,
        } => ScraperErrorInfo {
            code: normalize_optional_string(code),
            message: first_non_empty([message, detail])
                .unwrap_or_else(|| "unknown scraper error".to_string()),
            hint: normalize_optional_string(hint),
        },
    }
}

fn first_non_empty<const N: usize>(values: [Option<String>; N]) -> Option<String> {
    values.into_iter().find_map(normalize_optional_string)
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(normalize_string)
}

fn normalize_string_or_list(value: Option<RawStringOrList>) -> Option<String> {
    match value {
        Some(RawStringOrList::Text(text)) => normalize_string(text),
        Some(RawStringOrList::TextList(list)) => list.into_iter().find_map(normalize_string),
        None => None,
    }
}

fn normalize_string(value: impl AsRef<str>) -> Option<String> {
    let collapsed = value
        .as_ref()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct RawResponse {
    ok: bool,
    stage: Option<String>,
    #[serde(default)]
    items: Vec<RawSuggestItem>,
    entry: Option<RawEntry>,
    error: Option<RawError>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawSuggestItem {
    Word(String),
    Object {
        word: Option<String>,
        headword: Option<String>,
        entry: Option<String>,
        title: Option<String>,
        label: Option<String>,
        subtitle: Option<String>,
        pos: Option<String>,
        url: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct RawEntry {
    headword: Option<String>,
    word: Option<String>,
    term: Option<String>,
    pos: Option<String>,
    part_of_speech: Option<String>,
    phonetics: Option<RawStringOrList>,
    ipa: Option<RawStringOrList>,
    pronunciation: Option<RawStringOrList>,
    url: Option<String>,
    link: Option<String>,
    #[serde(default, alias = "senses", alias = "items", alias = "rows")]
    definitions: Vec<RawDefinition>,
    #[serde(default)]
    examples: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawStringOrList {
    Text(String),
    TextList(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawDefinition {
    Text(String),
    Object {
        text: Option<String>,
        definition: Option<String>,
        gloss: Option<String>,
        title: Option<String>,
        pos: Option<String>,
        part_of_speech: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawError {
    Message(String),
    Object {
        code: Option<String>,
        message: Option<String>,
        hint: Option<String>,
        detail: Option<String>,
    },
}

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("failed to spawn scraper process `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed while waiting for scraper process: {source}")]
    Wait {
        #[source]
        source: std::io::Error,
    },
    #[error("failed while reading scraper stdout: {source}")]
    ReadStdout {
        #[source]
        source: std::io::Error,
    },
    #[error("failed while reading scraper stderr: {source}")]
    ReadStderr {
        #[source]
        source: std::io::Error,
    },
    #[error("scraper process timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("scraper process exited with code {code:?}: {stderr}")]
    NonZeroExit { code: Option<i32>, stderr: String },
    #[error("scraper stdout is not valid UTF-8")]
    InvalidUtf8Stdout,
    #[error("invalid scraper JSON: {0}")]
    InvalidJson(String),
    #[error("unsupported scraper stage: {0}")]
    UnsupportedStage(String),
    #[error("scraper stage mismatch: expected {expected}, got {actual}")]
    StageMismatch { expected: String, actual: String },
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use crate::config::DictionaryMode;

    use super::*;

    fn fixture_config(script_path: PathBuf) -> RuntimeConfig {
        RuntimeConfig {
            dict_mode: DictionaryMode::English,
            max_results: 10,
            timeout_ms: 12_000,
            headless: true,
            node_bin: "node".to_string(),
            scraper_script: script_path,
        }
    }

    #[test]
    fn bridge_decode_suggest_response_accepts_string_and_object_items() {
        let json = r#"{
          "ok": true,
          "stage": "suggest",
          "items": [
            "open",
            {"headword": "close", "pos": "verb", "url": "https://example.com/close"},
            {"entry": "meaning of take", "label": "Meaning of take", "url": "https://example.com/take"}
          ]
        }"#;

        let decoded = decode_scraper_json(json, ScraperStage::Suggest)
            .expect("suggest response should decode");

        assert!(decoded.ok);
        assert_eq!(decoded.stage, ScraperStage::Suggest);
        assert_eq!(decoded.items.len(), 3);
        assert_eq!(decoded.items[0].word, "open");
        assert_eq!(decoded.items[1].word, "close");
        assert_eq!(decoded.items[1].subtitle.as_deref(), Some("verb"));
        assert_eq!(
            decoded.items[1].url.as_deref(),
            Some("https://example.com/close")
        );
        assert_eq!(decoded.items[2].word, "meaning of take");
        assert_eq!(
            decoded.items[2].url.as_deref(),
            Some("https://example.com/take")
        );
    }

    #[test]
    fn bridge_decode_define_response_maps_entry_and_definition_rows() {
        let json = r#"{
          "ok": true,
          "stage": "define",
          "entry": {
            "headword": "open",
            "part_of_speech": "adjective",
            "phonetics": "oh-puhn",
            "url": "https://example.com/open",
            "definitions": [
              "not closed",
              {"definition": "ready for use", "part_of_speech": "adjective"}
            ],
            "examples": [
              "Leave the door open.",
              "The museum is open until six. | 博物館營業到六點。"
            ]
          }
        }"#;

        let decoded =
            decode_scraper_json(json, ScraperStage::Define).expect("define response should decode");
        let entry = decoded.entry.expect("entry should exist");

        assert!(decoded.ok);
        assert_eq!(decoded.stage, ScraperStage::Define);
        assert_eq!(entry.headword, "open");
        assert_eq!(entry.part_of_speech.as_deref(), Some("adjective"));
        assert_eq!(entry.phonetics.as_deref(), Some("oh-puhn"));
        assert_eq!(entry.url.as_deref(), Some("https://example.com/open"));
        assert_eq!(entry.definitions.len(), 2);
        assert_eq!(entry.definitions[0].text, "not closed");
        assert_eq!(entry.definitions[1].text, "ready for use");
        assert_eq!(entry.examples.len(), 2);
        assert_eq!(entry.examples[0], "Leave the door open.");
    }

    #[test]
    fn bridge_decode_define_response_accepts_phonetics_array() {
        let json = r#"{
          "ok": true,
          "stage": "define",
          "entry": {
            "headword": "open",
            "phonetics": ["ˈəʊ.p ə", "ˈoʊ.p ə"],
            "definitions": ["not closed"],
            "examples": ["an open door/window"]
          }
        }"#;

        let decoded =
            decode_scraper_json(json, ScraperStage::Define).expect("define response should decode");
        let entry = decoded.entry.expect("entry should exist");

        assert_eq!(entry.phonetics.as_deref(), Some("ˈəʊ.p ə"));
        assert_eq!(entry.definitions.len(), 1);
        assert_eq!(entry.examples, vec!["an open door/window".to_string()]);
    }

    #[test]
    fn bridge_decode_uses_expected_stage_when_stage_field_is_missing() {
        let json = r#"{"ok": true, "items": ["open"]}"#;

        let decoded = decode_scraper_json(json, ScraperStage::Suggest)
            .expect("missing stage should default to expected stage");

        assert_eq!(decoded.stage, ScraperStage::Suggest);
        assert_eq!(decoded.items.len(), 1);
    }

    #[test]
    fn bridge_decode_rejects_stage_mismatch() {
        let json = r#"{"ok": true, "stage": "define", "entry": {"headword":"open"}}"#;

        let err = decode_scraper_json(json, ScraperStage::Suggest)
            .expect_err("stage mismatch should fail");

        assert!(matches!(err, BridgeError::StageMismatch { .. }));
    }

    #[test]
    fn bridge_decode_reads_error_payload() {
        let json = r#"{
          "ok": false,
          "stage": "suggest",
          "error": {
            "code": "timeout",
            "message": "Request timeout",
            "hint": "Retry later"
          }
        }"#;

        let decoded =
            decode_scraper_json(json, ScraperStage::Suggest).expect("error payload should decode");
        let error = decoded.error.expect("error should exist");

        assert!(!decoded.ok);
        assert_eq!(error.code.as_deref(), Some("timeout"));
        assert_eq!(error.message, "Request timeout");
        assert_eq!(error.hint.as_deref(), Some("Retry later"));
    }

    #[test]
    fn bridge_decode_reports_malformed_json() {
        let err = decode_scraper_json("{not-json", ScraperStage::Suggest)
            .expect_err("invalid json should fail");

        assert!(matches!(err, BridgeError::InvalidJson(_)));
    }

    #[test]
    fn bridge_build_args_uses_expected_flags() {
        let script_path = PathBuf::from("/tmp/scraper.mjs");
        let config = fixture_config(script_path.clone());

        let suggest_args = build_scraper_args(&config, ScraperStage::Suggest, "open");
        assert_eq!(suggest_args[0], script_path.to_string_lossy());
        assert_eq!(suggest_args[1], "suggest");
        assert!(suggest_args.contains(&"--mode".to_string()));
        assert!(suggest_args.contains(&"english".to_string()));
        assert!(suggest_args.contains(&"--query".to_string()));
        assert!(suggest_args.contains(&"open".to_string()));

        let define_args = build_scraper_args(&config, ScraperStage::Define, "open");
        assert_eq!(define_args[1], "define");
        assert!(define_args.contains(&"--entry".to_string()));
        assert!(define_args.contains(&"open".to_string()));
    }

    #[test]
    fn bridge_run_scraper_reports_spawn_failure_for_missing_runtime() {
        let dir = tempdir().expect("create temp dir");
        let script_path = dir.path().join("cambridge_scraper.mjs");
        fs::write(&script_path, "console.log('{}')").expect("write script");
        let mut config = fixture_config(script_path);
        config.node_bin = "/definitely-missing-node-runtime".to_string();

        let err = run_scraper(&config, ScraperStage::Suggest, "open")
            .expect_err("missing binary should fail");

        assert!(matches!(err, BridgeError::Spawn { .. }));
    }
}
