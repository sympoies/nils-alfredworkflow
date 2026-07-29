use alfred_core::{Feedback, Item};

use crate::convert::ConversionRow;

const NO_RESULTS_TITLE: &str = "No timezone results";
const NO_RESULTS_SUBTITLE: &str = "Provide at least one valid IANA timezone";

pub fn rows_to_feedback(rows: &[ConversionRow]) -> Feedback {
    if rows.is_empty() {
        return Feedback::new(vec![
            Item::new(NO_RESULTS_TITLE)
                .with_subtitle(NO_RESULTS_SUBTITLE)
                .with_valid(false),
        ]);
    }

    let items = rows
        .iter()
        .map(|row| {
            Item::new(row.title.clone())
                .with_uid(row.timezone_id.clone())
                .with_subtitle(row.subtitle.clone())
                .with_arg(row.arg.clone())
                .with_valid(true)
        })
        .collect();

    Feedback::new(items).with_skip_knowledge(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_to_feedback_maps_rows_to_items() {
        let rows = vec![ConversionRow::new(
            "Asia/Taipei",
            "2026-02-10 20:35:00",
            "Asia/Taipei (UTC+08:00)",
            "Asia/Taipei 2026-02-10 20:35:00 UTC+08:00",
        )];

        let feedback = rows_to_feedback(&rows);

        assert_eq!(feedback.skip_knowledge, Some(true));
        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].uid.as_deref(), Some("Asia/Taipei"));
        assert_eq!(feedback.items[0].valid, Some(true));
        assert_eq!(
            feedback.items[0].subtitle.as_deref(),
            Some("Asia/Taipei (UTC+08:00)")
        );
    }

    #[test]
    fn rows_to_feedback_returns_invalid_item_for_empty_rows() {
        let feedback = rows_to_feedback(&[]);

        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].title, NO_RESULTS_TITLE);
        assert_eq!(feedback.items[0].valid, Some(false));
    }
}
