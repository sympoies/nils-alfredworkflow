use alfred_core::{Feedback, Item};

use crate::steam_store_api::{SteamPlatforms, SteamSearchResult};

const NO_RESULTS_TITLE: &str = "No games found";
const NO_RESULTS_SUBTITLE: &str = "Try broader keywords or switch STEAM_REGION.";
const REGION_CURRENT_TITLE_PREFIX: &str = "Current region:";
const REGION_SWITCH_TITLE_PREFIX: &str = "Search in";
const REGION_SWITCH_ARG_PREFIX: &str = "steam-requery:";
#[cfg_attr(not(test), allow(dead_code))]
const ERROR_TITLE: &str = "Steam search failed";
const UNKNOWN_PRICE_LABEL: &str = "Price unavailable";
const FREE_TO_PLAY_LABEL: &str = "Free to play";
const UNKNOWN_PLATFORM_LABEL: &str = "Platforms: Unknown";
const SUBTITLE_MAX_CHARS: usize = 120;

pub fn search_results_to_feedback(
    region: &str,
    query: &str,
    region_options: &[String],
    language: &str,
    results: &[SteamSearchResult],
) -> Feedback {
    let mut items = region_switch_items(region, query, region_options, language);

    if results.is_empty() {
        items.push(no_results_item());
        return Feedback::new(items);
    }

    items.extend(
        results
            .iter()
            .map(|result| result_to_item(region, language, result)),
    );
    Feedback::new(items)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn error_feedback(message: &str) -> Feedback {
    Feedback::new(vec![
        Item::new(ERROR_TITLE)
            .with_subtitle(single_line_subtitle(message, SUBTITLE_MAX_CHARS))
            .with_valid(false),
    ])
}

fn result_to_item(region: &str, language: &str, result: &SteamSearchResult) -> Item {
    let title = result.name.trim();
    let normalized_title = if title.is_empty() {
        "(untitled app)"
    } else {
        title
    };

    let price = format_price(result);
    let platforms = format_platforms(&result.platforms);
    let subtitle = single_line_subtitle(&format!("{price} | {platforms}"), SUBTITLE_MAX_CHARS);

    Item::new(normalized_title)
        .with_subtitle(subtitle)
        .with_arg(canonical_app_url(result.app_id, region, language))
}

#[cfg_attr(not(test), allow(dead_code))]
fn canonical_app_url(app_id: u32, region: &str, language: &str) -> String {
    format!("https://store.steampowered.com/app/{app_id}/?cc={region}&l={language}")
}

fn format_price(result: &SteamSearchResult) -> String {
    match result.price.as_ref() {
        Some(price) => {
            if let Some(formatted) = price.final_formatted.as_deref().map(str::trim)
                && !formatted.is_empty()
            {
                return formatted.to_string();
            }

            match price.final_price_cents {
                Some(0) => FREE_TO_PLAY_LABEL.to_string(),
                Some(value) => {
                    let major = value / 100;
                    let minor = value % 100;
                    format!("${major}.{minor:02}")
                }
                None => UNKNOWN_PRICE_LABEL.to_string(),
            }
        }
        None => UNKNOWN_PRICE_LABEL.to_string(),
    }
}

fn format_platforms(platforms: &SteamPlatforms) -> String {
    let mut labels = Vec::with_capacity(3);
    if platforms.windows {
        labels.push("Windows");
    }
    if platforms.mac {
        labels.push("macOS");
    }
    if platforms.linux {
        labels.push("Linux");
    }

    if labels.is_empty() {
        UNKNOWN_PLATFORM_LABEL.to_string()
    } else {
        format!("Platforms: {}", labels.join(", "))
    }
}

fn no_results_item() -> Item {
    Item::new(NO_RESULTS_TITLE)
        .with_subtitle(NO_RESULTS_SUBTITLE)
        .with_valid(false)
}

fn region_switch_items(
    current_region: &str,
    query: &str,
    options: &[String],
    language: &str,
) -> Vec<Item> {
    let current_region_upper = current_region.to_ascii_uppercase();
    let mut items = Vec::with_capacity(options.len() + 1);
    items.push(
        Item::new(format!(
            "{REGION_CURRENT_TITLE_PREFIX} {current_region_upper}"
        ))
        .with_subtitle(format!(
            "Searching Steam Store in {current_region_upper} ({language})."
        ))
        .with_valid(false),
    );

    items.extend(options.iter().map(|candidate| {
        let candidate_upper = candidate.to_ascii_uppercase();
        let subtitle = single_line_subtitle(
            &format!("Press Enter to requery \"{query}\" in {candidate_upper}."),
            SUBTITLE_MAX_CHARS,
        );

        Item::new(format!(
            "{REGION_SWITCH_TITLE_PREFIX} {candidate_upper} region"
        ))
        .with_subtitle(subtitle)
        .with_arg(switch_region_arg(candidate, query))
        .with_valid(true)
    }));

    items
}

fn switch_region_arg(region: &str, query: &str) -> String {
    let compact_query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{REGION_SWITCH_ARG_PREFIX}{region}:{compact_query}")
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
    use super::*;
    use crate::steam_store_api::{SteamPlatforms, SteamPrice};

    fn fixture_result(
        price: Option<SteamPrice>,
        subtitle_platforms: SteamPlatforms,
    ) -> SteamSearchResult {
        SteamSearchResult {
            app_id: 730,
            name: "Counter-Strike 2".to_string(),
            price,
            platforms: subtitle_platforms,
        }
    }

    #[test]
    fn feedback_maps_result_to_alfred_item_with_canonical_url() {
        let feedback = search_results_to_feedback(
            "us",
            "counter strike",
            &[],
            "english",
            &[fixture_result(
                Some(SteamPrice {
                    final_price_cents: Some(0),
                    final_formatted: Some("Free".to_string()),
                }),
                SteamPlatforms {
                    windows: true,
                    mac: false,
                    linux: true,
                },
            )],
        );

        let item = feedback
            .items
            .get(1)
            .expect("expected current-region row and one result");

        assert_eq!(item.title, "Counter-Strike 2");
        assert_eq!(
            item.subtitle.as_deref(),
            Some("Free | Platforms: Windows, Linux")
        );
        assert_eq!(
            item.arg.as_deref(),
            Some("https://store.steampowered.com/app/730/?cc=us&l=english")
        );
    }

    #[test]
    fn feedback_switch_rows_follow_configured_order() {
        let options = vec!["jp".to_string(), "us".to_string(), "kr".to_string()];
        let feedback = search_results_to_feedback("us", "dota", &options, "english", &[]);

        assert_eq!(feedback.items[0].title, "Current region: US");
        assert_eq!(feedback.items[1].title, "Search in JP region");
        assert_eq!(feedback.items[2].title, "Search in US region");
        assert_eq!(feedback.items[3].title, "Search in KR region");
        assert_eq!(feedback.items[0].valid, Some(false));
        assert_eq!(feedback.items[1].valid, Some(true));
        assert_eq!(feedback.items[2].valid, Some(true));
        assert_eq!(feedback.items[3].valid, Some(true));
    }

    #[test]
    fn feedback_switch_rows_use_requery_arg_contract() {
        let options = vec!["jp".to_string(), "us".to_string()];
        let feedback = search_results_to_feedback("us", "dota 2", &options, "english", &[]);

        assert_eq!(
            feedback.items[1].arg.as_deref(),
            Some("steam-requery:jp:dota 2")
        );
        assert_eq!(
            feedback.items[2].arg.as_deref(),
            Some("steam-requery:us:dota 2")
        );
    }

    #[test]
    fn feedback_subtitle_truncation_is_deterministic_and_single_line() {
        let long_price = " very long price segment\n\t".repeat(30);
        let feedback = search_results_to_feedback(
            "us",
            "rust",
            &[],
            "english",
            &[fixture_result(
                Some(SteamPrice {
                    final_price_cents: None,
                    final_formatted: Some(long_price.clone()),
                }),
                SteamPlatforms {
                    windows: true,
                    mac: true,
                    linux: true,
                },
            )],
        );
        let subtitle = feedback.items[1]
            .subtitle
            .as_deref()
            .expect("subtitle should exist")
            .to_string();

        let feedback_again = search_results_to_feedback(
            "us",
            "rust",
            &[],
            "english",
            &[fixture_result(
                Some(SteamPrice {
                    final_price_cents: None,
                    final_formatted: Some(long_price),
                }),
                SteamPlatforms {
                    windows: true,
                    mac: true,
                    linux: true,
                },
            )],
        );
        let subtitle_again = feedback_again.items[1]
            .subtitle
            .as_deref()
            .expect("subtitle should exist")
            .to_string();

        assert_eq!(subtitle, subtitle_again, "subtitle should be deterministic");
        assert!(!subtitle.contains('\n'));
        assert!(!subtitle.contains('\t'));
        assert!(subtitle.chars().count() <= SUBTITLE_MAX_CHARS);
    }

    #[test]
    fn feedback_no_results_item_is_invalid_and_has_expected_title() {
        let feedback = search_results_to_feedback("us", "dota", &[], "english", &[]);
        let item = feedback
            .items
            .get(1)
            .expect("fallback item should exist after current-region row");

        assert_eq!(item.title, NO_RESULTS_TITLE);
        assert_eq!(item.subtitle.as_deref(), Some(NO_RESULTS_SUBTITLE));
        assert_eq!(item.valid, Some(false));
        assert!(item.arg.is_none());
    }

    #[test]
    fn feedback_uses_fallback_labels_for_missing_price_and_platforms() {
        let feedback = search_results_to_feedback(
            "us",
            "dota",
            &[],
            "english",
            &[fixture_result(None, SteamPlatforms::default())],
        );

        assert_eq!(
            feedback.items[1].subtitle.as_deref(),
            Some("Price unavailable | Platforms: Unknown")
        );
    }

    #[test]
    fn error_feedback_returns_single_invalid_item() {
        let feedback = error_feedback("request timed out\nplease retry");

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].title, ERROR_TITLE);
        assert_eq!(feedback.items[0].valid, Some(false));
        assert!(
            feedback.items[0]
                .subtitle
                .as_deref()
                .is_some_and(|value| !value.contains('\n'))
        );
    }

    #[test]
    fn url_builds_canonical_store_url() {
        assert_eq!(
            canonical_app_url(570, "jp", "schinese"),
            "https://store.steampowered.com/app/570/?cc=jp&l=schinese"
        );
    }
}
