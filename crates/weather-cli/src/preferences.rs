//! Default-location resolution for the Weather workflow.
//!
//! Precedence (the caller handles explicit queries first):
//! valid fresh external preference projection > `fallback` list
//! (`WEATHER_DEFAULT_CITIES`). Projection labels are used verbatim and are
//! never split on commas; only the fallback list uses comma/newline splitting.

use std::collections::HashSet;
use std::path::Path;

use chrono::{DateTime, Utc};
use workflow_common::{
    preference_projection::{ProjectionStatus, load_preference_projection},
    split_ordered_list,
};

use crate::geocoding::{coordinate_label, read_cached_city_location};

/// Status-row subtitle when the projection supplied the default locations.
pub const PROJECTION_USED_HINT: &str =
    "Default locations from the external preference projection. Type a city to override.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultLocations {
    pub locations: Vec<String>,
    /// Present only when a projection path is configured.
    pub status: Option<ProjectionStatus>,
}

impl DefaultLocations {
    /// `projection` when the projection supplied the list, otherwise `settings`.
    pub fn source(&self) -> &'static str {
        match self.status {
            Some(ProjectionStatus::Used { .. }) => "projection",
            _ => "settings",
        }
    }
}

pub fn resolve_default_locations(
    fallback: &str,
    projection_path: Option<&Path>,
    now: DateTime<Utc>,
) -> DefaultLocations {
    let Some(path) = projection_path else {
        return DefaultLocations {
            locations: split_ordered_list(fallback),
            status: None,
        };
    };

    let status = match load_preference_projection(path, now) {
        Ok(projection) => {
            let locations = projection.weather_default_locations();
            if !locations.is_empty() {
                return DefaultLocations {
                    locations,
                    status: Some(ProjectionStatus::Used {
                        revision: projection.revision,
                        generated_at: projection.generated_at,
                        skipped: 0,
                    }),
                };
            }
            ProjectionStatus::Empty {
                revision: projection.revision,
                skipped: 0,
            }
        }
        Err(error) => ProjectionStatus::Failed(error),
    };

    DefaultLocations {
        locations: split_ordered_list(fallback),
        status: Some(status),
    }
}

/// Drops a label whose cached geocode resolves to the same coordinates as an
/// earlier label, so two spellings of one place are listed once. Reads only
/// the local geocode cache; a label without a cached geocode is always kept.
pub fn collapse_same_place(locations: Vec<String>, cache_dir: &Path) -> Vec<String> {
    let mut listed_places = HashSet::new();
    locations
        .into_iter()
        .filter(|label| match read_cached_city_location(cache_dir, label) {
            Ok(Some(location)) => {
                listed_places.insert(coordinate_label(location.latitude, location.longitude))
            }
            _ => true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use chrono::TimeZone;
    use serde_json::json;
    use workflow_common::preference_projection::ProjectionError;

    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
            .single()
            .expect("time")
    }

    fn write_projection(
        dir: &tempfile::TempDir,
        generated_at: &str,
        default_location: &str,
        saved_locations: serde_json::Value,
    ) -> PathBuf {
        let path = dir.path().join("preference-projection.json");
        let document = json!({
            "schema": "sympoies.alfred-preference-projection/v1",
            "generatedAt": generated_at,
            "revision": 5,
            "digest": format!("sha256:{}", "b".repeat(64)),
            "market": {"default_quote_currency": "EUR", "watchlist": []},
            "weather": {
                "default_location": default_location,
                "saved_locations": saved_locations
            },
            "sources": {
                "market.default_quote_currency": "profile",
                "market.watchlist": "profile",
                "weather.default_location": "owner_override",
                "weather.saved_locations": "profile"
            }
        });
        fs::write(&path, document.to_string()).expect("write projection");
        path
    }

    #[test]
    fn projection_locations_keep_commas_unicode_and_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_projection(
            &dir,
            "2026-03-01T11:00:00Z",
            "Springfield, Oregon",
            json!(["東京", "SPRINGFIELD, OREGON", "Zürich"]),
        );

        let resolved = resolve_default_locations("Tokyo,Osaka", Some(&path), now());
        assert_eq!(
            resolved.locations,
            vec!["Springfield, Oregon", "東京", "Zürich"]
        );
        assert_eq!(resolved.source(), "projection");
        assert!(matches!(
            resolved.status,
            Some(ProjectionStatus::Used {
                revision: Some(5),
                ..
            })
        ));
    }

    #[test]
    fn fallback_list_is_split_when_projection_unset() {
        let resolved = resolve_default_locations(" Tokyo, Osaka ,,", None, now());
        assert_eq!(resolved.locations, vec!["Tokyo", "Osaka"]);
        assert_eq!(resolved.status, None);
        assert_eq!(resolved.source(), "settings");
    }

    #[test]
    fn fallback_list_is_used_with_status_when_projection_unusable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.json");
        let resolved = resolve_default_locations("Tokyo,Osaka", Some(&missing), now());
        assert_eq!(resolved.locations, vec!["Tokyo", "Osaka"]);
        assert_eq!(
            resolved.status,
            Some(ProjectionStatus::Failed(ProjectionError::Unavailable))
        );

        let stale = write_projection(&dir, "2026-02-20T00:00:00Z", "Kyoto", json!([]));
        let resolved = resolve_default_locations("Tokyo", Some(&stale), now());
        assert_eq!(resolved.locations, vec!["Tokyo"]);
        assert_eq!(
            resolved.status.as_ref().map(ProjectionStatus::state),
            Some("stale")
        );

        let empty = write_projection(&dir, "2026-03-01T11:00:00Z", "", json!([]));
        let resolved = resolve_default_locations("Tokyo", Some(&empty), now());
        assert_eq!(resolved.locations, vec!["Tokyo"]);
        assert_eq!(
            resolved.status.as_ref().map(ProjectionStatus::state),
            Some("empty")
        );
    }
}
