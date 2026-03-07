use alfred_core::{Feedback, Item};

use crate::config::DictionaryMode;
use crate::scraper_bridge::{Entry, ScraperErrorInfo, ScraperResponse, SuggestItem};

const EMPTY_INPUT_TITLE: &str = "Type a word to search Cambridge";
const EMPTY_INPUT_SUBTITLE: &str = "Select a suggestion, or use def::<word> for direct lookup";
const MISSING_DEFINE_TITLE: &str = "Definition token is incomplete";
const MISSING_DEFINE_SUBTITLE: &str = "Use def::<word>, for example def::open";

const SUGGEST_ERROR_TITLE: &str = "Cambridge suggestions unavailable";
const SUGGEST_ERROR_SUBTITLE: &str = "Retry in a moment or check scraper availability";
const SUGGEST_EMPTY_TITLE: &str = "No candidate entries found";
const SUGGEST_EMPTY_SUBTITLE: &str = "Try another keyword";
const SUGGEST_GUIDANCE: &str = "Press Tab to load definition";

const DEFINE_ERROR_TITLE: &str = "Cambridge definition unavailable";
const DEFINE_ERROR_SUBTITLE: &str = "Retry this entry or pick another suggestion";
const DEFINE_EMPTY_TITLE: &str = "No definitions found";
const DEFINE_EMPTY_SUBTITLE: &str = "Try another headword";
const DEFINE_ROW_SUBTITLE_PREFIX: &str = "Definition";
const EXAMPLE_ROW_SUBTITLE_PREFIX: &str = "Example";

pub fn empty_input_feedback() -> Feedback {
    single_invalid_item(EMPTY_INPUT_TITLE, EMPTY_INPUT_SUBTITLE)
}

pub fn missing_define_target_feedback() -> Feedback {
    single_invalid_item(MISSING_DEFINE_TITLE, MISSING_DEFINE_SUBTITLE)
}

pub fn suggest_feedback(response: &ScraperResponse) -> Feedback {
    if !response.ok {
        return single_invalid_item(
            SUGGEST_ERROR_TITLE,
            &error_subtitle(response.error.as_ref(), SUGGEST_ERROR_SUBTITLE),
        );
    }

    let items: Vec<Item> = response
        .items
        .iter()
        .filter_map(suggest_item_to_feedback_item)
        .collect();

    if items.is_empty() {
        return single_invalid_item(SUGGEST_EMPTY_TITLE, SUGGEST_EMPTY_SUBTITLE);
    }

    Feedback::new(items)
}

pub fn define_feedback(
    response: &ScraperResponse,
    requested_entry: &str,
    mode: DictionaryMode,
) -> Feedback {
    if !response.ok {
        return single_invalid_item(
            DEFINE_ERROR_TITLE,
            &error_subtitle(response.error.as_ref(), DEFINE_ERROR_SUBTITLE),
        );
    }

    let Some(entry) = response.entry.as_ref() else {
        return single_invalid_item(DEFINE_EMPTY_TITLE, DEFINE_EMPTY_SUBTITLE);
    };

    let headword = normalize_text(&entry.headword)
        .or_else(|| normalize_text(requested_entry))
        .unwrap_or_else(|| "entry".to_string());
    let entry_url = resolve_entry_url(entry, &headword, mode);

    let mut items = vec![definition_header_item(entry, &headword, &entry_url)];
    let mut definition_count = 0usize;

    for (idx, row) in entry.definitions.iter().enumerate() {
        let Some(definition_text) = normalize_text(&row.text) else {
            continue;
        };

        let (title, translation) = split_bilingual_definition(&definition_text)
            .unwrap_or_else(|| (definition_text.clone(), None));

        items.push(detail_row_item(
            &title,
            build_detail_subtitle(
                row.part_of_speech
                    .as_deref()
                    .or(entry.part_of_speech.as_deref()),
                DEFINE_ROW_SUBTITLE_PREFIX,
                idx + 1,
                translation.as_deref(),
            ),
            &entry_url,
        ));
        definition_count += 1;
    }

    for (idx, example) in entry.examples.iter().enumerate() {
        let Some(example_text) = normalize_text(example) else {
            continue;
        };

        let (title, translation) = split_bilingual_definition(&example_text)
            .unwrap_or_else(|| (example_text.clone(), None));

        items.push(detail_row_item(
            &title,
            build_detail_subtitle(
                None,
                EXAMPLE_ROW_SUBTITLE_PREFIX,
                idx + 1,
                translation.as_deref(),
            ),
            &entry_url,
        ));
    }

    if definition_count == 0 {
        items.push(
            Item::new("No definition rows parsed")
                .with_subtitle("Press Enter to open Cambridge webpage")
                .with_arg(entry_url)
                .with_valid(true),
        );
    }

    Feedback::new(items)
}

fn suggest_item_to_feedback_item(item: &SuggestItem) -> Option<Item> {
    let word = normalize_text(&item.word)?;
    let subtitle = item
        .subtitle
        .as_deref()
        .and_then(normalize_text)
        .map(|message| format!("{message} | {SUGGEST_GUIDANCE}"))
        .unwrap_or_else(|| SUGGEST_GUIDANCE.to_string());

    Some(
        Item::new(word.clone())
            .with_subtitle(subtitle)
            .with_autocomplete(format!("def::{word}"))
            .with_valid(false),
    )
}

fn definition_header_item(entry: &Entry, headword: &str, entry_url: &str) -> Item {
    let mut details = Vec::new();
    if let Some(part) = entry.part_of_speech.as_deref().and_then(normalize_text) {
        details.push(part);
    }
    if let Some(phonetics) = entry.phonetics.as_deref().and_then(normalize_text) {
        details.push(format!("/{phonetics}/"));
    }
    details.push("Press Enter on a row to open Cambridge".to_string());

    Item::new(format!("{headword} - Cambridge"))
        .with_subtitle(details.join(" | "))
        .with_arg(entry_url.to_string())
        .with_valid(true)
}

fn detail_row_item(title: &str, subtitle: String, entry_url: &str) -> Item {
    Item::new(title)
        .with_subtitle(subtitle)
        .with_arg(entry_url.to_string())
        .with_valid(true)
}

fn build_detail_subtitle(
    part_of_speech: Option<&str>,
    row_kind: &str,
    idx: usize,
    translation: Option<&str>,
) -> String {
    let mut subtitle_parts = Vec::new();
    if let Some(part) = part_of_speech.and_then(normalize_text) {
        subtitle_parts.push(part);
    }
    subtitle_parts.push(format!("{row_kind} {idx}"));
    if let Some(text) = translation.and_then(normalize_text) {
        subtitle_parts.push(text);
    }
    subtitle_parts.join(" | ")
}

fn single_invalid_item(title: &str, subtitle: &str) -> Feedback {
    Feedback::new(vec![
        Item::new(title).with_subtitle(subtitle).with_valid(false),
    ])
}

fn error_subtitle(error: Option<&ScraperErrorInfo>, fallback: &str) -> String {
    let Some(error) = error else {
        return fallback.to_string();
    };

    let mut parts = Vec::new();
    if let Some(code) = error.code.as_deref().and_then(normalize_text) {
        parts.push(format!("code: {code}"));
    }
    if let Some(message) = normalize_text(&error.message) {
        parts.push(message);
    }
    if let Some(hint) = error.hint.as_deref().and_then(normalize_text) {
        parts.push(format!("hint: {hint}"));
    }

    if parts.is_empty() {
        fallback.to_string()
    } else {
        parts.join(" | ")
    }
}

fn resolve_entry_url(entry: &Entry, headword: &str, mode: DictionaryMode) -> String {
    if let Some(url) = entry.url.as_deref().and_then(normalize_text) {
        return url;
    }

    let encoded_headword = percent_encode_path_segment(headword);
    format!(
        "https://dictionary.cambridge.org/dictionary/{}/{}",
        mode.as_str(),
        encoded_headword
    )
}

fn percent_encode_path_segment(input: &str) -> String {
    let mut output = String::new();
    for byte in input.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(*byte as char)
            }
            b' ' => output.push_str("%20"),
            _ => output.push_str(format!("%{byte:02X}").as_str()),
        }
    }

    if output.is_empty() {
        "entry".to_string()
    } else {
        output
    }
}

fn normalize_text(input: &str) -> Option<String> {
    let normalized = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn split_bilingual_definition(input: &str) -> Option<(String, Option<String>)> {
    let normalized = normalize_text(input)?;
    let Some((left, right)) = normalized.rsplit_once(" | ") else {
        return Some((normalized, None));
    };

    let english = normalize_text(left)?;
    let translation = normalize_text(right);
    Some((english, translation))
}

#[cfg(test)]
mod tests {
    use crate::scraper_bridge::{DefinitionLine, Entry, ScraperResponse, ScraperStage};

    use super::*;

    fn fixture_suggest_response(items: Vec<SuggestItem>) -> ScraperResponse {
        ScraperResponse {
            ok: true,
            stage: ScraperStage::Suggest,
            items,
            entry: None,
            error: None,
        }
    }

    fn fixture_define_response(entry: Entry) -> ScraperResponse {
        ScraperResponse {
            ok: true,
            stage: ScraperStage::Define,
            items: Vec::new(),
            entry: Some(entry),
            error: None,
        }
    }

    #[test]
    fn feedback_empty_input_returns_single_invalid_item() {
        let feedback = empty_input_feedback();

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].title, EMPTY_INPUT_TITLE);
        assert_eq!(feedback.items[0].valid, Some(false));
    }

    #[test]
    fn feedback_suggest_maps_autocomplete_and_invalid_items() {
        let feedback = suggest_feedback(&fixture_suggest_response(vec![SuggestItem {
            word: "open".to_string(),
            subtitle: Some("verb".to_string()),
            url: None,
        }]));

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].title, "open");
        assert_eq!(feedback.items[0].valid, Some(false));
        assert_eq!(feedback.items[0].autocomplete.as_deref(), Some("def::open"));
        assert!(
            feedback.items[0]
                .subtitle
                .as_deref()
                .is_some_and(|subtitle| subtitle.contains("Press Tab")),
            "suggest subtitle should include transition guidance"
        );
    }

    #[test]
    fn feedback_suggest_returns_no_results_fallback_when_items_are_empty() {
        let feedback = suggest_feedback(&fixture_suggest_response(Vec::new()));

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].title, SUGGEST_EMPTY_TITLE);
        assert_eq!(feedback.items[0].valid, Some(false));
    }

    #[test]
    fn feedback_suggest_uses_error_payload_for_fallback_subtitle() {
        let response = ScraperResponse {
            ok: false,
            stage: ScraperStage::Suggest,
            items: Vec::new(),
            entry: None,
            error: Some(ScraperErrorInfo {
                code: Some("timeout".to_string()),
                message: "Request timed out".to_string(),
                hint: Some("Retry later".to_string()),
            }),
        };

        let feedback = suggest_feedback(&response);

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].title, SUGGEST_ERROR_TITLE);
        let subtitle = feedback.items[0].subtitle.as_deref().expect("subtitle");
        assert!(subtitle.contains("timeout"));
        assert!(subtitle.contains("Retry later"));
    }

    #[test]
    fn feedback_define_outputs_header_and_valid_definition_rows() {
        let response = fixture_define_response(Entry {
            headword: "open".to_string(),
            part_of_speech: Some("verb".to_string()),
            phonetics: Some("oh-puhn".to_string()),
            url: Some("https://example.com/open".to_string()),
            definitions: vec![
                DefinitionLine {
                    text: "not closed".to_string(),
                    part_of_speech: Some("adjective".to_string()),
                },
                DefinitionLine {
                    text: "ready for use".to_string(),
                    part_of_speech: None,
                },
            ],
            examples: Vec::new(),
        });

        let feedback = define_feedback(&response, "open", DictionaryMode::English);

        assert_eq!(feedback.items.len(), 3);
        assert_eq!(
            feedback.items[0].valid,
            Some(true),
            "header should be valid"
        );
        assert_eq!(feedback.items[1].valid, Some(true));
        assert_eq!(feedback.items[2].valid, Some(true));
        assert_eq!(
            feedback.items[0].arg.as_deref(),
            Some("https://example.com/open")
        );
        assert_eq!(
            feedback.items[1].arg.as_deref(),
            Some("https://example.com/open")
        );
        assert_eq!(
            feedback.items[2].arg.as_deref(),
            Some("https://example.com/open")
        );
    }

    #[test]
    fn feedback_define_builds_fallback_entry_url_when_entry_url_is_missing() {
        let response = fixture_define_response(Entry {
            headword: "open up".to_string(),
            part_of_speech: None,
            phonetics: None,
            url: None,
            definitions: vec![DefinitionLine {
                text: "to become available".to_string(),
                part_of_speech: None,
            }],
            examples: Vec::new(),
        });

        let feedback = define_feedback(
            &response,
            "open up",
            DictionaryMode::EnglishChineseTraditional,
        );

        assert_eq!(feedback.items.len(), 2);
        assert_eq!(
            feedback.items[1].arg.as_deref(),
            Some(
                "https://dictionary.cambridge.org/dictionary/english-chinese-traditional/open%20up"
            )
        );
    }

    #[test]
    fn feedback_define_moves_translation_text_to_subtitle_for_bilingual_rows() {
        let response = fixture_define_response(Entry {
            headword: "ghost".to_string(),
            part_of_speech: Some("noun".to_string()),
            phonetics: None,
            url: Some("https://example.com/ghost".to_string()),
            definitions: vec![DefinitionLine {
                text: "the spirit of a dead person | 鬼，幽靈".to_string(),
                part_of_speech: None,
            }],
            examples: Vec::new(),
        });

        let feedback = define_feedback(
            &response,
            "ghost",
            DictionaryMode::EnglishChineseTraditional,
        );

        assert_eq!(feedback.items.len(), 2);
        assert_eq!(feedback.items[1].title, "the spirit of a dead person");
        let subtitle = feedback.items[1].subtitle.as_deref().expect("subtitle");
        assert!(subtitle.contains("noun"));
        assert!(subtitle.contains("Definition 1"));
        assert!(subtitle.contains("鬼，幽靈"));
    }

    #[test]
    fn feedback_define_appends_example_rows_after_definitions() {
        let response = fixture_define_response(Entry {
            headword: "symphony".to_string(),
            part_of_speech: Some("noun".to_string()),
            phonetics: None,
            url: Some("https://example.com/symphony".to_string()),
            definitions: vec![DefinitionLine {
                text: "a long piece of music | 交響樂".to_string(),
                part_of_speech: None,
            }],
            examples: vec!["Mahler's ninth symphony | 馬勒的《第九交響曲》".to_string()],
        });

        let feedback = define_feedback(
            &response,
            "symphony",
            DictionaryMode::EnglishChineseTraditional,
        );

        assert_eq!(feedback.items.len(), 3);
        assert_eq!(feedback.items[2].title, "Mahler's ninth symphony");
        let subtitle = feedback.items[2].subtitle.as_deref().expect("subtitle");
        assert!(subtitle.contains("Example 1"));
        assert!(subtitle.contains("馬勒的《第九交響曲》"));
    }

    #[test]
    fn feedback_define_returns_no_definitions_item_when_entry_is_missing() {
        let response = ScraperResponse {
            ok: true,
            stage: ScraperStage::Define,
            items: Vec::new(),
            entry: None,
            error: None,
        };

        let feedback = define_feedback(&response, "open", DictionaryMode::English);

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].title, DEFINE_EMPTY_TITLE);
        assert_eq!(feedback.items[0].valid, Some(false));
    }
}
