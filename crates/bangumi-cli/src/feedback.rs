use alfred_core::{Feedback, Item, ItemIcon, ItemModifier};

use crate::bangumi_api::{BangumiSubject, canonical_subject_url};
use crate::image_cache::{ImageCacheError, ImageCacheManager};
use crate::input::SubjectType;

const NO_RESULTS_TITLE: &str = "No Bangumi subjects found";
const NO_RESULTS_SUBTITLE: &str =
    "Try another keyword or type prefix (all/book/anime/music/game/real)";
const SUBTITLE_MAX_CHARS: usize = 120;
const SUMMARY_PREVIEW_MAX_CHARS: usize = 90;

pub fn subjects_to_feedback(subjects: &[BangumiSubject], requested_type: SubjectType) -> Feedback {
    subjects_to_feedback_with_icons(subjects, requested_type, None, |_url| Ok(Vec::new()))
}

pub fn subjects_to_feedback_with_icons<F>(
    subjects: &[BangumiSubject],
    requested_type: SubjectType,
    image_cache: Option<&ImageCacheManager>,
    mut fetch_image: F,
) -> Feedback
where
    F: FnMut(&str) -> Result<Vec<u8>, ImageCacheError>,
{
    if subjects.is_empty() {
        return no_results_feedback();
    }

    let mut items = Vec::with_capacity(subjects.len());

    for subject in subjects {
        let mut item = subject_to_item(subject, requested_type);

        if let Some(cache) = image_cache
            && let Ok(Some(icon_path)) =
                cache.resolve_subject_icon_with(subject, |url| fetch_image(url))
        {
            item = item.with_icon(ItemIcon::new(icon_path.to_string_lossy().into_owned()));
        }

        items.push(item);
    }

    Feedback::new(items).with_skip_knowledge(true)
}

fn subject_to_item(subject: &BangumiSubject, requested_type: SubjectType) -> Item {
    let title = normalized_title(subject);
    let url = normalized_url(subject);
    let subtitle = build_subtitle(subject, requested_type);

    let mut item = Item::new(title.clone())
        .with_uid(format!("subject-{}", subject.id))
        .with_subtitle(subtitle)
        .with_arg(url.clone())
        .with_autocomplete(build_autocomplete(subject, requested_type, &title))
        .with_valid(true);

    if let Some(summary) = normalized_summary(subject) {
        item = item.with_mod(
            "cmd",
            ItemModifier::new()
                .with_subtitle(format!(
                    "Summary: {}",
                    single_line_subtitle(summary, SUMMARY_PREVIEW_MAX_CHARS)
                ))
                .with_arg(url.clone())
                .with_valid(true),
        );
    }

    if let Some(rank_score_subtitle) = rank_score_subtitle(subject) {
        item = item.with_mod(
            "ctrl",
            ItemModifier::new()
                .with_subtitle(rank_score_subtitle)
                .with_arg(url)
                .with_valid(true),
        );
    }

    item
}

fn build_autocomplete(
    subject: &BangumiSubject,
    requested_type: SubjectType,
    title: &str,
) -> String {
    if requested_type == SubjectType::All {
        let prefix = subject
            .subject_type
            .map(SubjectType::as_str)
            .unwrap_or("all");
        return format!("{prefix} {title}");
    }

    title.to_string()
}

fn build_subtitle(subject: &BangumiSubject, requested_type: SubjectType) -> String {
    let mut parts = Vec::new();

    if requested_type == SubjectType::All {
        let type_tag = subject
            .subject_type
            .map(SubjectType::as_str)
            .unwrap_or("unknown");
        parts.push(format!("[{type_tag}]"));
    }

    if let Some(name_cn) = subject
        .name_cn
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != subject.name.trim())
    {
        parts.push(name_cn.to_string());
    }

    if let Some(summary) = normalized_summary(subject) {
        parts.push(single_line_subtitle(summary, SUBTITLE_MAX_CHARS));
    }

    if parts.is_empty() {
        format!("subject #{}", subject.id)
    } else {
        parts.join(" · ")
    }
}

fn normalized_title(subject: &BangumiSubject) -> String {
    let name = subject.name.trim();
    if name.is_empty() {
        format!("subject #{}", subject.id)
    } else {
        name.to_string()
    }
}

fn normalized_url(subject: &BangumiSubject) -> String {
    let url = subject.url.trim();
    if url.is_empty() {
        canonical_subject_url(subject.id)
    } else {
        url.to_string()
    }
}

fn normalized_summary(subject: &BangumiSubject) -> Option<&str> {
    subject
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn rank_score_subtitle(subject: &BangumiSubject) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(rank) = subject.rank {
        parts.push(format!("Rank #{rank}"));
    }

    if let Some(score) = subject.score {
        parts.push(format!("Score {score:.1}"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

fn no_results_feedback() -> Feedback {
    Feedback::new(vec![
        Item::new(NO_RESULTS_TITLE)
            .with_subtitle(NO_RESULTS_SUBTITLE)
            .with_valid(false),
    ])
}

fn single_line_subtitle(input: &str, max_chars: usize) -> String {
    let compact = input.split_whitespace().collect::<Vec<_>>().join(" ");

    if compact.chars().count() <= max_chars {
        return compact;
    }

    if max_chars <= 3 {
        return "...".chars().take(max_chars).collect();
    }

    let truncated: String = compact.chars().take(max_chars - 3).collect();
    format!("{truncated}...")
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use tempfile::tempdir;

    use super::*;
    use crate::bangumi_api::SubjectImages;
    use crate::config::{ApiFallbackPolicy, DEFAULT_USER_AGENT, RuntimeConfig};

    fn fixture_subject() -> BangumiSubject {
        BangumiSubject {
            id: 2782,
            subject_type: Some(SubjectType::Anime),
            name: "Cowboy Bebop".to_string(),
            name_cn: Some("星際牛仔".to_string()),
            summary: Some("  Space western classic with bounty hunters. ".to_string()),
            url: "https://bgm.tv/subject/2782".to_string(),
            rank: Some(1),
            score: Some(9.1),
            images: SubjectImages {
                small: Some("https://img.example.com/2782-small.jpg".to_string()),
                grid: None,
                common: None,
                large: None,
            },
        }
    }

    fn fixture_config(cache_dir: &std::path::Path) -> RuntimeConfig {
        RuntimeConfig {
            api_key: None,
            max_results: 10,
            timeout_ms: 8_000,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            cache_dir: cache_dir.to_path_buf(),
            image_cache_ttl_seconds: 120,
            image_cache_max_bytes: 8 * 1024 * 1024,
            api_fallback: ApiFallbackPolicy::Auto,
        }
    }

    #[test]
    fn feedback_maps_subjects_to_alfred_item_fields() {
        let feedback = subjects_to_feedback(&[fixture_subject()], SubjectType::Anime);
        let json = feedback.to_json().expect("feedback should serialize");
        let payload: serde_json::Value =
            serde_json::from_str(&json).expect("feedback should be valid JSON");
        let item = feedback.items.first().expect("item should exist");

        assert_eq!(
            payload
                .get("skipknowledge")
                .and_then(serde_json::Value::as_bool),
            Some(true),
            "subject uid rows must preserve API match order"
        );
        assert_eq!(item.title, "Cowboy Bebop");
        assert_eq!(item.arg.as_deref(), Some("https://bgm.tv/subject/2782"));
        assert_eq!(item.valid, Some(true));
        assert_eq!(item.uid.as_deref(), Some("subject-2782"));
    }

    #[test]
    fn modifier_adds_ctrl_metadata_only_when_rank_or_score_exists() {
        let with_meta = subjects_to_feedback(&[fixture_subject()], SubjectType::Anime);
        let with_ctrl = with_meta.items[0]
            .mods
            .as_ref()
            .and_then(|mods| mods.get("ctrl"));
        assert!(
            with_ctrl.is_some(),
            "ctrl modifier should be present with metadata"
        );

        let no_meta_subject = BangumiSubject {
            rank: None,
            score: None,
            ..fixture_subject()
        };
        let without_meta = subjects_to_feedback(&[no_meta_subject], SubjectType::Anime);
        let without_ctrl = without_meta.items[0]
            .mods
            .as_ref()
            .and_then(|mods| mods.get("ctrl"));
        assert!(
            without_ctrl.is_none(),
            "ctrl modifier should be omitted when metadata is absent"
        );
    }

    #[test]
    fn subtitle_includes_type_tag_and_localized_name_in_all_mode() {
        let feedback = subjects_to_feedback(&[fixture_subject()], SubjectType::All);
        let subtitle = feedback.items[0]
            .subtitle
            .as_deref()
            .expect("subtitle should exist");

        assert!(subtitle.contains("[anime]"));
        assert!(subtitle.contains("星際牛仔"));
    }

    #[test]
    fn cmd_modifier_present_when_summary_exists() {
        let feedback = subjects_to_feedback(&[fixture_subject()], SubjectType::Anime);
        let cmd_mod = feedback.items[0]
            .mods
            .as_ref()
            .and_then(|mods| mods.get("cmd"));

        assert!(
            cmd_mod.is_some(),
            "cmd modifier should exist when summary exists"
        );
        assert!(
            cmd_mod
                .and_then(|modifier| modifier.subtitle.as_deref())
                .is_some_and(|subtitle| subtitle.starts_with("Summary:"))
        );
    }

    #[test]
    fn cmd_modifier_absent_when_summary_missing() {
        let subject = BangumiSubject {
            summary: None,
            ..fixture_subject()
        };
        let feedback = subjects_to_feedback(&[subject], SubjectType::Anime);

        let cmd_mod = feedback.items[0]
            .mods
            .as_ref()
            .and_then(|mods| mods.get("cmd"));
        assert!(
            cmd_mod.is_none(),
            "cmd modifier should be omitted without summary"
        );
    }

    #[test]
    fn feedback_applies_local_icon_when_cache_manager_succeeds() {
        let dir = tempdir().expect("temp dir");
        let config = fixture_config(dir.path());
        let cache = ImageCacheManager::new(&config);

        let subject = fixture_subject();
        let feedback =
            subjects_to_feedback_with_icons(&[subject], SubjectType::Anime, Some(&cache), |_url| {
                Ok(vec![1, 2, 3, 4])
            });

        let icon_path = feedback.items[0]
            .icon
            .as_ref()
            .map(|icon| icon.path.clone())
            .expect("icon path should exist");

        assert!(
            icon_path.contains("images"),
            "icon path should point to the image cache directory"
        );
    }

    #[test]
    fn feedback_keeps_rendering_when_icon_download_fails() {
        let dir = tempdir().expect("temp dir");
        let config = fixture_config(dir.path());
        let cache = ImageCacheManager::new(&config);

        let subject = fixture_subject();
        let feedback =
            subjects_to_feedback_with_icons(&[subject], SubjectType::Anime, Some(&cache), |_url| {
                Err(ImageCacheError::Http { status: 404 })
            });

        assert_eq!(feedback.items.len(), 1);
        assert!(
            feedback.items[0].icon.is_none(),
            "icon should be omitted when fetch fails"
        );
    }

    #[test]
    fn feedback_no_results_returns_non_actionable_guidance_row() {
        let feedback = subjects_to_feedback(&[], SubjectType::All);
        let item = feedback.items.first().expect("guidance item should exist");

        assert_eq!(item.title, NO_RESULTS_TITLE);
        assert_eq!(item.valid, Some(false));
    }

    #[test]
    fn subtitle_truncation_is_single_line_and_deterministic() {
        let long = " line1\nline2\tline3".repeat(40);
        let subject = BangumiSubject {
            summary: Some(long.clone()),
            ..fixture_subject()
        };

        let first = subjects_to_feedback(std::slice::from_ref(&subject), SubjectType::Anime);
        let second = subjects_to_feedback(&[subject], SubjectType::Anime);

        let first_subtitle = first.items[0]
            .subtitle
            .as_deref()
            .expect("subtitle should exist");
        let second_subtitle = second.items[0]
            .subtitle
            .as_deref()
            .expect("subtitle should exist");

        assert_eq!(first_subtitle, second_subtitle);
        assert!(!first_subtitle.contains('\n'));
        assert!(!first_subtitle.contains('\t'));
    }

    #[test]
    fn feedback_cache_ttl_integration_prefers_cached_file_on_repeated_mapping() {
        let dir = tempdir().expect("temp dir");
        let config = fixture_config(dir.path());
        let cache = ImageCacheManager::new(&config);
        let subject = fixture_subject();

        let mut download_count = 0;
        let _ = cache
            .resolve_subject_icon_with(&subject, |_url| {
                download_count += 1;
                Ok(vec![1, 2, 3])
            })
            .expect("first resolve should succeed");

        let _ = cache
            .resolve_subject_icon_with(&subject, |_url| {
                download_count += 1;
                Ok(vec![4, 5, 6])
            })
            .expect("second resolve should succeed");

        assert_eq!(download_count, 1);

        let _ = cache
            .resolve_subject_icon_with(&subject, |_url| {
                download_count += 1;
                Ok(vec![7, 8, 9])
            })
            .expect("third resolve should succeed");

        let _ = SystemTime::now() + Duration::from_secs(1);
        assert_eq!(
            download_count, 1,
            "ttl should keep cache fresh within interval"
        );
    }
}
