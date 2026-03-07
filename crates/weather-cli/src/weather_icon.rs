use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;

const NIGHT_START_HOUR: u32 = 18;
const DAY_START_HOUR: u32 = 6;

pub fn current_conditions_icon_key(
    weather_code: i32,
    timezone: &str,
    now: DateTime<Utc>,
) -> &'static str {
    icon_key_for_local_hour(weather_code, current_local_hour(timezone, now))
}

pub fn daily_forecast_icon_key(weather_code: i32) -> &'static str {
    icon_key_for_local_hour(weather_code, None)
}

pub fn hourly_forecast_icon_key(weather_code: i32, datetime: &str) -> &'static str {
    icon_key_for_local_hour(weather_code, datetime_local_hour(datetime))
}

pub fn icon_key_for_local_hour(weather_code: i32, local_hour: Option<u32>) -> &'static str {
    if uses_day_night_variant(weather_code) && local_hour.is_some_and(is_night_hour) {
        return night_variant_icon_key(weather_code);
    }

    day_variant_icon_key(weather_code)
}

pub fn uses_day_night_variant(weather_code: i32) -> bool {
    night_variant_icon_key(weather_code) != day_variant_icon_key(weather_code)
}

pub fn is_night_icon_key(icon_key: &str) -> bool {
    icon_key.ends_with("-night")
}

fn current_local_hour(timezone: &str, now: DateTime<Utc>) -> Option<u32> {
    local_hour_override().or_else(|| {
        timezone
            .parse::<Tz>()
            .ok()
            .map(|tz| now.with_timezone(&tz).hour())
    })
}

fn local_hour_override() -> Option<u32> {
    std::env::var("WEATHER_ICON_LOCAL_HOUR_OVERRIDE")
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
        .filter(|hour| *hour < 24)
}

fn datetime_local_hour(datetime: &str) -> Option<u32> {
    datetime
        .split_once('T')
        .map(|(_, time)| time)
        .or_else(|| datetime.split_once(' ').map(|(_, time)| time))
        .and_then(|time| time.get(0..2))
        .and_then(|hour| hour.parse::<u32>().ok())
        .filter(|hour| *hour < 24)
}

fn is_night_hour(hour: u32) -> bool {
    !(DAY_START_HOUR..NIGHT_START_HOUR).contains(&hour)
}

fn day_variant_icon_key(weather_code: i32) -> &'static str {
    match weather_code {
        0 => "clear-day",
        1 => "mainly-clear-day",
        2 => "partly-cloudy-day",
        3 => "cloudy",
        45 | 48 => "fog",
        51 | 53 | 55 | 56 | 57 => "drizzle",
        61 | 63 | 65 | 66 | 67 => "rain",
        71 | 73 | 75 | 77 => "snow",
        80..=82 => "rain-showers",
        85 | 86 => "snow-showers",
        95 | 96 | 99 => "thunderstorm",
        _ => "unknown",
    }
}

fn night_variant_icon_key(weather_code: i32) -> &'static str {
    match weather_code {
        0 => "clear-night",
        1 => "mainly-clear-night",
        2 => "partly-cloudy-night",
        3 => "cloudy-night",
        45 | 48 => "fog-night",
        51 | 53 | 55 | 56 | 57 => "drizzle-night",
        61 | 63 | 65 | 66 | 67 => "rain-night",
        71 | 73 | 75 | 77 => "snow-night",
        80..=82 => "rain-showers-night",
        85 | 86 => "snow-showers-night",
        95 | 96 | 99 => "thunderstorm-night",
        _ => "unknown-night",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn clear_family_uses_explicit_day_keys() {
        assert_eq!(daily_forecast_icon_key(0), "clear-day");
        assert_eq!(daily_forecast_icon_key(1), "mainly-clear-day");
        assert_eq!(daily_forecast_icon_key(2), "partly-cloudy-day");
    }

    #[test]
    fn cloudy_family_uses_shared_keys() {
        assert_eq!(daily_forecast_icon_key(3), "cloudy");
        assert_eq!(daily_forecast_icon_key(61), "rain");
    }

    #[test]
    fn night_variants_cover_non_clear_weather_codes() {
        assert_eq!(
            hourly_forecast_icon_key(3, "2026-02-11T22:00"),
            "cloudy-night"
        );
        assert_eq!(
            hourly_forecast_icon_key(61, "2026-02-11T22:00"),
            "rain-night"
        );
        assert_eq!(
            hourly_forecast_icon_key(95, "2026-02-11T22:00"),
            "thunderstorm-night"
        );
    }

    #[test]
    fn hourly_icons_switch_to_night_variants_after_dark() {
        assert_eq!(
            hourly_forecast_icon_key(0, "2026-02-11T22:00"),
            "clear-night"
        );
        assert_eq!(
            hourly_forecast_icon_key(1, "2026-02-11T04:00"),
            "mainly-clear-night"
        );
        assert_eq!(
            hourly_forecast_icon_key(2, "2026-02-11T10:00"),
            "partly-cloudy-day"
        );
    }

    #[test]
    fn current_conditions_use_timezone_local_hour() {
        let now = Utc
            .with_ymd_and_hms(2026, 2, 11, 14, 0, 0)
            .single()
            .expect("time");

        assert_eq!(
            current_conditions_icon_key(0, "America/Los_Angeles", now),
            "clear-day"
        );
        assert_eq!(
            current_conditions_icon_key(0, "Asia/Tokyo", now),
            "clear-night"
        );
    }

    #[test]
    fn invalid_hour_inputs_fall_back_to_day_variant() {
        assert_eq!(hourly_forecast_icon_key(0, "bad"), "clear-day");
        assert_eq!(
            current_conditions_icon_key(0, "Bad/Timezone", Utc::now()),
            "clear-day"
        );
    }

    #[test]
    fn icon_key_reports_night_variant_status() {
        assert!(is_night_icon_key("clear-night"));
        assert!(!is_night_icon_key("clear-day"));
        assert!(is_night_icon_key("rain-night"));
        assert!(!is_night_icon_key("rain"));
    }
}
