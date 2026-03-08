use chrono::{
    DateTime, Datelike, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeZone, Utc, Weekday,
};
use chrono_tz::Tz;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;
use workflow_common::{
    EnvelopePayloadKind, OutputMode, build_alfred_error_feedback, build_error_details_json,
    build_error_envelope, build_success_envelope, redact_sensitive, select_output_mode,
};

use weather_cli::{
    batch_service,
    config::RuntimeConfig,
    error::AppError,
    hourly_service::{self, DEFAULT_HOURLY_COUNT},
    model::{
        ForecastBatchOutput, ForecastOutput, ForecastPeriod, ForecastRequest, HourlyForecastOutput,
        LocationQuery, OutputMode as RequestOutputMode,
    },
    providers::{HttpProviders, ProviderApi},
    service,
};

#[cfg(test)]
use weather_cli::{
    geocoding::ResolvedLocation,
    providers::{
        ProviderError, ProviderForecast, ProviderForecastDay, ProviderForecastHour,
        ProviderHourlyForecast,
    },
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Weather forecast CLI (free no-token APIs)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Today weather forecast.
    Today {
        #[arg(long = "city")]
        city: Vec<String>,
        #[arg(long, allow_hyphen_values = true)]
        lat: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        lon: Option<f64>,
        #[arg(long, value_enum)]
        output: Option<OutputModeArg>,
        #[arg(long)]
        json: bool,
        #[arg(long, value_enum)]
        lang: Option<LanguageArg>,
    },
    /// 7-day weather forecast.
    Week {
        #[arg(long = "city")]
        city: Vec<String>,
        #[arg(long, allow_hyphen_values = true)]
        lat: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        lon: Option<f64>,
        #[arg(long, value_enum)]
        output: Option<OutputModeArg>,
        #[arg(long)]
        json: bool,
        #[arg(long, value_enum)]
        lang: Option<LanguageArg>,
    },
    /// Hourly weather forecast (next 24h by default).
    Hourly {
        #[arg(long)]
        city: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        lat: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        lon: Option<f64>,
        #[arg(long, value_enum)]
        output: Option<OutputModeArg>,
        #[arg(long)]
        json: bool,
        #[arg(long, value_enum)]
        lang: Option<LanguageArg>,
        #[arg(long, default_value_t = DEFAULT_HOURLY_COUNT)]
        hours: usize,
    },
}

const ERROR_CODE_USER_INVALID_INPUT: &str = "user.invalid_input";
const ERROR_CODE_USER_OUTPUT_MODE_CONFLICT: &str = "user.output_mode_conflict";
const ERROR_CODE_RUNTIME_PROVIDER_INIT: &str = "runtime.provider_init_failed";
const ERROR_CODE_RUNTIME_SERIALIZE: &str = "runtime.serialize_failed";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputModeArg {
    Human,
    Json,
    AlfredJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LanguageArg {
    En,
    Zh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputLanguage {
    En,
    Zh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliError {
    kind: weather_cli::error::ErrorKind,
    code: &'static str,
    message: String,
}

impl CliError {
    fn user(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: weather_cli::error::ErrorKind::User,
            code,
            message: message.into(),
        }
    }

    fn runtime(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: weather_cli::error::ErrorKind::Runtime,
            code,
            message: message.into(),
        }
    }

    fn exit_code(&self) -> i32 {
        match self.kind {
            weather_cli::error::ErrorKind::User => 2,
            weather_cli::error::ErrorKind::Runtime => 1,
        }
    }
}

impl From<OutputModeArg> for OutputMode {
    fn from(value: OutputModeArg) -> Self {
        match value {
            OutputModeArg::Human => OutputMode::Human,
            OutputModeArg::Json => OutputMode::Json,
            OutputModeArg::AlfredJson => OutputMode::AlfredJson,
        }
    }
}

impl From<LanguageArg> for OutputLanguage {
    fn from(value: LanguageArg) -> Self {
        match value {
            LanguageArg::En => OutputLanguage::En,
            LanguageArg::Zh => OutputLanguage::Zh,
        }
    }
}

impl Cli {
    fn command_name(&self) -> &'static str {
        match &self.command {
            Commands::Today { .. } => "weather.today",
            Commands::Week { .. } => "weather.week",
            Commands::Hourly { .. } => "weather.hourly",
        }
    }

    fn output_mode_hint(&self) -> OutputMode {
        match &self.command {
            Commands::Today { output, json, .. }
            | Commands::Week { output, json, .. }
            | Commands::Hourly { output, json, .. } => {
                if *json {
                    OutputMode::Json
                } else if let Some(explicit) = output {
                    (*explicit).into()
                } else {
                    OutputMode::Human
                }
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let command = cli.command_name();
    let output_mode = cli.output_mode_hint();
    match run(cli) {
        Ok(output) => println!("{output}"),
        Err(error) => {
            emit_error(command, output_mode, &error);
            std::process::exit(error.exit_code());
        }
    }
}

fn run(cli: Cli) -> Result<String, CliError> {
    let config = RuntimeConfig::from_env();
    let providers = HttpProviders::new()
        .map_err(|error| runtime_error(ERROR_CODE_RUNTIME_PROVIDER_INIT, error.to_string()))?;
    run_with(cli, &config, &providers, Utc::now)
}

fn run_with<P, N>(
    cli: Cli,
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
) -> Result<String, CliError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    match cli.command {
        Commands::Today {
            city,
            lat,
            lon,
            output,
            json,
            lang,
        } => run_command(
            config,
            providers,
            now_fn,
            CommandArgs {
                command: "weather.today",
                period: ForecastPeriod::Today,
                cities: &city,
                lat,
                lon,
                output,
                json,
                lang,
            },
        ),
        Commands::Week {
            city,
            lat,
            lon,
            output,
            json,
            lang,
        } => run_command(
            config,
            providers,
            now_fn,
            CommandArgs {
                command: "weather.week",
                period: ForecastPeriod::Week,
                cities: &city,
                lat,
                lon,
                output,
                json,
                lang,
            },
        ),
        Commands::Hourly {
            city,
            lat,
            lon,
            output,
            json,
            lang,
            hours,
        } => run_hourly_command(
            config,
            providers,
            now_fn,
            HourlyCommandArgs {
                command: "weather.hourly",
                city: city.as_deref(),
                lat,
                lon,
                output,
                json,
                lang,
                hours,
            },
        ),
    }
}

#[derive(Debug, Clone, Copy)]
struct CommandArgs<'a> {
    command: &'static str,
    period: ForecastPeriod,
    cities: &'a [String],
    lat: Option<f64>,
    lon: Option<f64>,
    output: Option<OutputModeArg>,
    json: bool,
    lang: Option<LanguageArg>,
}

#[derive(Debug, Clone, Copy)]
struct HourlyCommandArgs<'a> {
    command: &'static str,
    city: Option<&'a str>,
    lat: Option<f64>,
    lon: Option<f64>,
    output: Option<OutputModeArg>,
    json: bool,
    lang: Option<LanguageArg>,
    hours: usize,
}

fn run_command<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    args: CommandArgs<'_>,
) -> Result<String, CliError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    let output_mode = select_output_mode(args.output.map(Into::into), args.json, OutputMode::Human)
        .map_err(|error| user_error(ERROR_CODE_USER_OUTPUT_MODE_CONFLICT, error.to_string()))?;
    let output_language = args.lang.map(Into::into).unwrap_or(OutputLanguage::En);

    if args.cities.len() > 1 {
        if args.lat.is_some() || args.lon.is_some() {
            return Err(user_error(
                ERROR_CODE_USER_INVALID_INPUT,
                "conflicting location input: use either repeated --city or --lat/--lon",
            ));
        }

        let output = batch_service::resolve_forecast_batch(
            config,
            providers,
            now_fn,
            args.period,
            args.cities,
        )
        .map_err(map_app_error)?;

        return match output_mode {
            OutputMode::Json => render_batch_json_envelope(args.command, &output),
            OutputMode::Human => Ok(format_batch_text_output(&output, output_language)),
            OutputMode::AlfredJson => render_batch_alfred_json(&output, output_language, now_fn()),
        };
    }

    let request_mode = match output_mode {
        OutputMode::Json => RequestOutputMode::Json,
        OutputMode::Human | OutputMode::AlfredJson => RequestOutputMode::Text,
    };
    let request = ForecastRequest::new(
        args.period,
        args.cities.first().map(String::as_str),
        args.lat,
        args.lon,
        request_mode,
    )
    .map_err(user_invalid_input)?;
    let output =
        service::resolve_forecast(config, providers, now_fn, &request).map_err(map_app_error)?;

    match output_mode {
        OutputMode::Json => render_service_json_envelope(args.command, &output),
        OutputMode::Human => Ok(format_text_output(&output, output_language)),
        OutputMode::AlfredJson => render_alfred_json(&output, output_language, now_fn()),
    }
}

fn run_hourly_command<P, N>(
    config: &RuntimeConfig,
    providers: &P,
    now_fn: N,
    args: HourlyCommandArgs<'_>,
) -> Result<String, CliError>
where
    P: ProviderApi,
    N: Fn() -> DateTime<Utc> + Copy,
{
    let output_mode = select_output_mode(args.output.map(Into::into), args.json, OutputMode::Human)
        .map_err(|error| user_error(ERROR_CODE_USER_OUTPUT_MODE_CONFLICT, error.to_string()))?;
    let output_language = args.lang.map(Into::into).unwrap_or(OutputLanguage::En);
    let location = resolve_location_query(args.city, args.lat, args.lon)?;
    let output =
        hourly_service::resolve_hourly_forecast(config, providers, now_fn, &location, args.hours)
            .map_err(map_app_error)?;

    match output_mode {
        OutputMode::Json => render_hourly_json_envelope(args.command, &output),
        OutputMode::Human => Ok(format_hourly_text_output(&output, output_language)),
        OutputMode::AlfredJson => render_hourly_alfred_json(&output, output_language),
    }
}

fn resolve_location_query(
    city: Option<&str>,
    lat: Option<f64>,
    lon: Option<f64>,
) -> Result<LocationQuery, CliError> {
    let request = ForecastRequest::new(
        ForecastPeriod::Hourly,
        city,
        lat,
        lon,
        RequestOutputMode::Json,
    )
    .map_err(user_invalid_input)?;
    Ok(request.location)
}

fn render_service_json_envelope(
    command: &str,
    output: &ForecastOutput,
) -> Result<String, CliError> {
    let result = serde_json::to_string(output).map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to serialize output: {error}"),
        )
    })?;
    Ok(build_success_envelope(
        command,
        EnvelopePayloadKind::Result,
        &result,
    ))
}

fn render_hourly_json_envelope(
    command: &str,
    output: &HourlyForecastOutput,
) -> Result<String, CliError> {
    let result = serde_json::to_string(output).map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to serialize output: {error}"),
        )
    })?;
    Ok(build_success_envelope(
        command,
        EnvelopePayloadKind::Result,
        &result,
    ))
}

fn render_batch_json_envelope(
    command: &str,
    output: &ForecastBatchOutput,
) -> Result<String, CliError> {
    let result = serde_json::to_string(output).map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to serialize batch output: {error}"),
        )
    })?;
    Ok(build_success_envelope(
        command,
        EnvelopePayloadKind::Result,
        &result,
    ))
}

fn render_alfred_json(
    output: &ForecastOutput,
    language: OutputLanguage,
    now: DateTime<Utc>,
) -> Result<String, CliError> {
    let mut items = Vec::with_capacity(output.forecast.len() + 1);
    items.push(alfred_header_item(
        &output.location.name,
        &output.timezone,
        output.location.latitude,
        output.location.longitude,
        &output.source,
        output.freshness.status,
    ));
    for day in &output.forecast {
        let summary = localized_summary(day, language);
        let date_with_weekday = format_date_with_weekday(&day.date, language);
        let weekday_label = weekday_label_for_date(&day.date, language);
        let utc_offset_label = timezone_offset_label_for_date(&output.timezone, &day.date);
        let timezone_display =
            timezone_display_label(&output.timezone, utc_offset_label.as_deref());
        let icon_key = if output.period == ForecastPeriod::Today {
            weather_cli::weather_icon::current_conditions_icon_key(
                day.weather_code,
                &output.timezone,
                now,
            )
        } else {
            weather_cli::weather_icon::daily_forecast_icon_key(day.weather_code)
        };

        items.push(json!({
            "title": format!(
                "{} {} {:.1}~{:.1}°C",
                date_with_weekday, summary, day.temp_min_c, day.temp_max_c
            ),
            "subtitle": format!("{}:{}%", precip_label(language), day.precip_prob_max_pct),
            "arg": day.date,
            "valid": false,
            "icon": {
                "path": icon_path(icon_key),
            },
            "weather_meta": {
                "item_kind": "daily",
                "date": day.date,
                "date_with_weekday": date_with_weekday,
                "weekday_label": weekday_label,
                "utc_offset_label": utc_offset_label,
                "timezone_display": timezone_display,
                "summary": summary,
                "weather_code": day.weather_code,
                "icon_key": icon_key,
                "is_night": weather_cli::weather_icon::is_night_icon_key(icon_key),
                "temp_min_c": day.temp_min_c,
                "temp_max_c": day.temp_max_c,
                "temp_min_c_label": format!("{:.1}", day.temp_min_c),
                "temp_max_c_label": format!("{:.1}", day.temp_max_c),
                "precip_prob_max_pct": day.precip_prob_max_pct,
                "precip_prob_max_pct_label": day.precip_prob_max_pct.to_string(),
            },
        }));
    }

    serde_json::to_string(&json!({ "items": items })).map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to serialize Alfred output: {error}"),
        )
    })
}

fn render_batch_alfred_json(
    output: &ForecastBatchOutput,
    language: OutputLanguage,
    now: DateTime<Utc>,
) -> Result<String, CliError> {
    let mut items = Vec::new();

    for entry in &output.entries {
        if let Some(result) = &entry.result {
            for day in &result.forecast {
                items.push(alfred_daily_city_item(
                    result,
                    day,
                    language,
                    now,
                    output.period == ForecastPeriod::Today,
                ));
            }
            continue;
        }

        items.push(json!({
            "title": format!("{}: forecast error", entry.city),
            "subtitle": entry.error.as_deref().unwrap_or("failed to fetch forecast"),
            "valid": false,
        }));
    }

    serde_json::to_string(&json!({ "items": items })).map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to serialize Alfred batch output: {error}"),
        )
    })
}

fn render_hourly_alfred_json(
    output: &HourlyForecastOutput,
    language: OutputLanguage,
) -> Result<String, CliError> {
    let mut items = Vec::with_capacity(output.hourly.len() + 1);
    items.push(alfred_header_item(
        &output.location.name,
        &output.timezone,
        output.location.latitude,
        output.location.longitude,
        &output.source,
        output.freshness.status,
    ));

    for hour in &output.hourly {
        let summary = localized_summary_by_code(hour.weather_code, language);
        let icon_key =
            weather_cli::weather_icon::hourly_forecast_icon_key(hour.weather_code, &hour.datetime);
        let (date, time) = split_datetime_label(&hour.datetime);
        let date_with_weekday = format_date_with_weekday(date, language);
        let weekday_label = weekday_label_for_datetime(&hour.datetime, language);
        let utc_offset_label = timezone_offset_label_for_datetime(&output.timezone, &hour.datetime);
        let timezone_display =
            timezone_display_label(&output.timezone, utc_offset_label.as_deref());
        items.push(json!({
            "title": format!(
                "{} {} {:.1}°C",
                display_hour_label(&hour.datetime, language),
                summary,
                hour.temp_c
            ),
            "subtitle": format!("{}:{}%", precip_label(language), hour.precip_prob_pct),
            "arg": hour.datetime,
            "valid": false,
            "icon": {
                "path": icon_path(icon_key),
            },
            "weather_meta": {
                "item_kind": "hourly",
                "date": date,
                "date_with_weekday": date_with_weekday,
                "weekday_label": weekday_label,
                "utc_offset_label": utc_offset_label,
                "timezone_display": timezone_display,
                "time": time,
                "datetime": hour.datetime,
                "summary": summary,
                "weather_code": hour.weather_code,
                "icon_key": icon_key,
                "is_night": weather_cli::weather_icon::is_night_icon_key(icon_key),
                "temp_c": hour.temp_c,
                "temp_c_label": format!("{:.1}", hour.temp_c),
                "precip_prob_pct": hour.precip_prob_pct,
                "precip_prob_pct_label": hour.precip_prob_pct.to_string(),
            },
        }));
    }

    serde_json::to_string(&json!({ "items": items })).map_err(|error| {
        runtime_error(
            ERROR_CODE_RUNTIME_SERIALIZE,
            format!("failed to serialize Alfred output: {error}"),
        )
    })
}

fn emit_error(command: &str, output_mode: OutputMode, error: &CliError) {
    match output_mode {
        OutputMode::Json => {
            let details = build_error_details_json(error_kind_label(error.kind), error.exit_code());
            println!(
                "{}",
                build_error_envelope(command, error.code, &error.message, Some(&details))
            );
        }
        OutputMode::AlfredJson => {
            println!(
                "{}",
                build_alfred_error_feedback(error.code, &error.message)
            );
        }
        OutputMode::Human => {
            eprintln!(
                "error[{}]: {}",
                error.code,
                redact_sensitive(&error.message)
            );
        }
    }
}

fn user_invalid_input(error: weather_cli::model::ValidationError) -> CliError {
    user_error(ERROR_CODE_USER_INVALID_INPUT, error.to_string())
}

fn user_error(code: &'static str, message: impl Into<String>) -> CliError {
    CliError::user(code, message)
}

fn runtime_error(code: &'static str, message: impl Into<String>) -> CliError {
    CliError::runtime(code, message)
}

fn map_app_error(error: AppError) -> CliError {
    match error.kind {
        weather_cli::error::ErrorKind::User => {
            user_error(ERROR_CODE_USER_INVALID_INPUT, error.message)
        }
        weather_cli::error::ErrorKind::Runtime => {
            runtime_error("runtime.provider_failed", error.message)
        }
    }
}

fn error_kind_label(kind: weather_cli::error::ErrorKind) -> &'static str {
    match kind {
        weather_cli::error::ErrorKind::User => "user",
        weather_cli::error::ErrorKind::Runtime => "runtime",
    }
}

fn format_text_output(output: &ForecastOutput, language: OutputLanguage) -> String {
    let mut lines = vec![format!(
        "{} ({}) | source={} | freshness={}",
        output.location.name,
        output.timezone,
        output.source,
        freshness_label(output.freshness.status)
    )];

    for day in &output.forecast {
        let summary = localized_summary(day, language);
        lines.push(format!(
            "{} {} {:.1}~{:.1}°C {}:{}%",
            format_date_with_weekday(&day.date, language),
            summary,
            day.temp_min_c,
            day.temp_max_c,
            precip_label(language),
            day.precip_prob_max_pct
        ));
    }

    lines.join("\n")
}

fn format_batch_text_output(output: &ForecastBatchOutput, language: OutputLanguage) -> String {
    let mut sections = Vec::new();

    for entry in &output.entries {
        if let Some(result) = &entry.result {
            sections.push(format_text_output(result, language));
        } else {
            sections.push(format!(
                "{} | error={}",
                entry.city,
                entry.error.as_deref().unwrap_or("failed to fetch forecast"),
            ));
        }
    }

    sections.join("\n\n")
}

fn format_hourly_text_output(output: &HourlyForecastOutput, language: OutputLanguage) -> String {
    let mut lines = vec![format!(
        "{} ({}) | source={} | freshness={}",
        output.location.name,
        output.timezone,
        output.source,
        freshness_label(output.freshness.status)
    )];

    for hour in &output.hourly {
        let summary = localized_summary_by_code(hour.weather_code, language);
        lines.push(format!(
            "{} {} {:.1}°C {}:{}%",
            display_hour_label(&hour.datetime, language),
            summary,
            hour.temp_c,
            precip_label(language),
            hour.precip_prob_pct
        ));
    }

    lines.join("\n")
}

fn localized_summary(day: &weather_cli::model::ForecastDay, language: OutputLanguage) -> String {
    localized_summary_by_code(day.weather_code, language)
}

fn localized_summary_by_code(weather_code: i32, language: OutputLanguage) -> String {
    match language {
        OutputLanguage::En => weather_cli::weather_code::summary_en(weather_code).to_string(),
        OutputLanguage::Zh => weather_cli::weather_code::summary_zh(weather_code).to_string(),
    }
}

fn localized_weekday_label(weekday: Weekday, language: OutputLanguage) -> &'static str {
    match language {
        OutputLanguage::En => match weekday {
            Weekday::Mon => "Mon",
            Weekday::Tue => "Tue",
            Weekday::Wed => "Wed",
            Weekday::Thu => "Thu",
            Weekday::Fri => "Fri",
            Weekday::Sat => "Sat",
            Weekday::Sun => "Sun",
        },
        OutputLanguage::Zh => match weekday {
            Weekday::Mon => "週一",
            Weekday::Tue => "週二",
            Weekday::Wed => "週三",
            Weekday::Thu => "週四",
            Weekday::Fri => "週五",
            Weekday::Sat => "週六",
            Weekday::Sun => "週日",
        },
    }
}

fn weekday_label_for_date(date: &str, language: OutputLanguage) -> Option<&'static str> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()
        .map(|parsed| localized_weekday_label(parsed.weekday(), language))
}

fn parse_local_datetime(datetime: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(datetime, "%Y-%m-%dT%H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(datetime, "%Y-%m-%d %H:%M"))
        .ok()
}

fn parse_timezone(timezone: &str) -> Option<Tz> {
    timezone.parse::<Tz>().ok()
}

fn resolve_localized_datetime(
    timezone: &str,
    local_datetime: NaiveDateTime,
) -> Option<DateTime<Tz>> {
    let tz = parse_timezone(timezone)?;
    match tz.from_local_datetime(&local_datetime) {
        LocalResult::Single(datetime) => Some(datetime),
        LocalResult::Ambiguous(datetime, _) => Some(datetime),
        LocalResult::None => None,
    }
}

fn timezone_offset_seconds_for_date(timezone: &str, date: &str) -> Option<i32> {
    let local_date = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let local_datetime = local_date.and_hms_opt(12, 0, 0)?;
    resolve_localized_datetime(timezone, local_datetime)
        .map(|datetime| datetime.offset().fix().local_minus_utc())
}

fn timezone_offset_seconds_for_datetime(timezone: &str, datetime: &str) -> Option<i32> {
    let local_datetime = parse_local_datetime(datetime)?;
    resolve_localized_datetime(timezone, local_datetime)
        .map(|datetime| datetime.offset().fix().local_minus_utc())
}

fn format_utc_offset_label(offset_seconds: i32) -> String {
    let sign = if offset_seconds >= 0 { '+' } else { '-' };
    let absolute_seconds = offset_seconds.unsigned_abs();
    let hours = absolute_seconds / 3600;
    let minutes = (absolute_seconds % 3600) / 60;
    let seconds = absolute_seconds % 60;

    if seconds > 0 {
        return format!("UTC{sign}{hours}:{minutes:02}:{seconds:02}");
    }

    if minutes > 0 {
        return format!("UTC{sign}{hours}:{minutes:02}");
    }

    format!("UTC{sign}{hours}")
}

fn timezone_offset_label_for_date(timezone: &str, date: &str) -> Option<String> {
    timezone_offset_seconds_for_date(timezone, date).map(format_utc_offset_label)
}

fn timezone_offset_label_for_datetime(timezone: &str, datetime: &str) -> Option<String> {
    timezone_offset_seconds_for_datetime(timezone, datetime).map(format_utc_offset_label)
}

fn timezone_display_label(timezone: &str, offset_label: Option<&str>) -> String {
    match offset_label {
        Some(offset_label) => format!("{timezone} ({offset_label})"),
        None => timezone.to_string(),
    }
}

fn weekday_label_for_datetime(datetime: &str, language: OutputLanguage) -> Option<&'static str> {
    parse_local_datetime(datetime).map(|parsed| localized_weekday_label(parsed.weekday(), language))
}

fn format_date_with_weekday(date: &str, language: OutputLanguage) -> String {
    match weekday_label_for_date(date, language) {
        Some(label) => format!("{date} {label}"),
        None => date.to_string(),
    }
}

fn display_hour_label(datetime: &str, language: OutputLanguage) -> String {
    if let Some(parsed) = parse_local_datetime(datetime) {
        return format!(
            "{} {} {}",
            parsed.format("%Y-%m-%d"),
            localized_weekday_label(parsed.weekday(), language),
            parsed.format("%H:%M")
        );
    }

    datetime.replace('T', " ")
}

fn split_datetime_label(datetime: &str) -> (&str, &str) {
    datetime
        .split_once('T')
        .or_else(|| datetime.split_once(' '))
        .unwrap_or((datetime, ""))
}

fn icon_path(icon_key: &str) -> String {
    format!("assets/icons/weather/{icon_key}.png")
}

fn alfred_daily_city_item(
    output: &ForecastOutput,
    day: &weather_cli::model::ForecastDay,
    language: OutputLanguage,
    now: DateTime<Utc>,
    use_current_conditions_icon: bool,
) -> serde_json::Value {
    let summary = localized_summary(day, language);
    let rendered_summary = if summary
        .chars()
        .all(|ch| ch.is_ascii_alphabetic() || ch == ' ')
    {
        summary.to_ascii_lowercase()
    } else {
        summary.clone()
    };
    let weekday_label = weekday_label_for_date(&day.date, language);
    let weekday_segment = weekday_label
        .map(|label| format!(" {label}"))
        .unwrap_or_default();
    let date_with_weekday = format_date_with_weekday(&day.date, language);
    let utc_offset_label = timezone_offset_label_for_date(&output.timezone, &day.date);
    let timezone_display = timezone_display_label(&output.timezone, utc_offset_label.as_deref());
    let icon_key = if use_current_conditions_icon {
        weather_cli::weather_icon::current_conditions_icon_key(
            day.weather_code,
            &output.timezone,
            now,
        )
    } else {
        weather_cli::weather_icon::daily_forecast_icon_key(day.weather_code)
    };

    json!({
        "title": format!(
            "{}{} {:.1}~{:.1}°C {} {}%",
            output.location.name,
            weekday_segment,
            day.temp_min_c,
            day.temp_max_c,
            rendered_summary,
            day.precip_prob_max_pct
        ),
        "subtitle": format!(
            "{} {} {:.4},{:.4}",
            date_with_weekday,
            output.timezone,
            output.location.latitude,
            output.location.longitude
        ),
        "arg": day.date,
        "valid": true,
        "icon": {
            "path": icon_path(icon_key),
        },
        "weather_meta": {
            "item_kind": "daily",
            "date": day.date,
            "date_with_weekday": date_with_weekday,
            "weekday_label": weekday_label,
            "utc_offset_label": utc_offset_label,
            "timezone_display": timezone_display,
            "summary": summary,
            "weather_code": day.weather_code,
            "icon_key": icon_key,
            "is_night": weather_cli::weather_icon::is_night_icon_key(icon_key),
            "temp_min_c": day.temp_min_c,
            "temp_max_c": day.temp_max_c,
            "temp_min_c_label": format!("{:.1}", day.temp_min_c),
            "temp_max_c_label": format!("{:.1}", day.temp_max_c),
            "precip_prob_max_pct": day.precip_prob_max_pct,
            "precip_prob_max_pct_label": day.precip_prob_max_pct.to_string(),
            "location_name": output.location.name,
            "timezone": output.timezone,
            "latitude_label": format!("{:.4}", output.location.latitude),
            "longitude_label": format!("{:.4}", output.location.longitude),
        },
    })
}

fn alfred_header_item(
    location_name: &str,
    timezone: &str,
    latitude: f64,
    longitude: f64,
    source: &str,
    freshness_status: weather_cli::model::FreshnessStatus,
) -> serde_json::Value {
    json!({
        "title": format!("{location_name} ({timezone})"),
        "subtitle": format!(
            "source={} freshness={} lat={:.4} lon={:.4}",
            source,
            freshness_label(freshness_status),
            latitude,
            longitude
        ),
        "arg": location_name,
        "valid": false,
        "weather_meta": {
            "item_kind": "header",
            "location_name": location_name,
            "timezone": timezone,
            "latitude": latitude,
            "longitude": longitude,
            "latitude_label": format!("{latitude:.4}"),
            "longitude_label": format!("{longitude:.4}"),
        },
    })
}

fn precip_label(language: OutputLanguage) -> &'static str {
    match language {
        OutputLanguage::En => "rain",
        OutputLanguage::Zh => "降雨",
    }
}

fn freshness_label(status: weather_cli::model::FreshnessStatus) -> &'static str {
    match status {
        weather_cli::model::FreshnessStatus::Live => "live",
        weather_cli::model::FreshnessStatus::CacheFresh => "cache_fresh",
        weather_cli::model::FreshnessStatus::CacheStaleFallback => "cache_stale_fallback",
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::Value;

    use super::*;

    struct FakeProviders {
        geocode_result: Result<ResolvedLocation, ProviderError>,
        open_meteo_result: Result<ProviderForecast, ProviderError>,
        open_meteo_hourly_result: Result<ProviderHourlyForecast, ProviderError>,
        met_no_result: Result<ProviderForecast, ProviderError>,
    }

    impl FakeProviders {
        fn ok() -> Self {
            let now = Utc
                .with_ymd_and_hms(2026, 2, 11, 0, 0, 0)
                .single()
                .expect("time");

            Self {
                geocode_result: Ok(ResolvedLocation {
                    name: "Taipei City".to_string(),
                    latitude: 25.05,
                    longitude: 121.52,
                    timezone: "Asia/Taipei".to_string(),
                }),
                open_meteo_result: Ok(ProviderForecast {
                    timezone: "Asia/Taipei".to_string(),
                    fetched_at: now,
                    days: vec![ProviderForecastDay {
                        date: "2026-02-11".to_string(),
                        weather_code: 3,
                        temp_min_c: 14.5,
                        temp_max_c: 20.1,
                        precip_prob_max_pct: 20,
                    }],
                }),
                open_meteo_hourly_result: Ok(ProviderHourlyForecast {
                    timezone: "Asia/Taipei".to_string(),
                    utc_offset_seconds: 0,
                    fetched_at: now,
                    hours: vec![ProviderForecastHour {
                        datetime: "2026-02-11T00:00".to_string(),
                        weather_code: 3,
                        temp_c: 16.1,
                        precip_prob_pct: 20,
                    }],
                }),
                met_no_result: Ok(ProviderForecast {
                    timezone: "UTC".to_string(),
                    fetched_at: now,
                    days: vec![ProviderForecastDay {
                        date: "2026-02-11".to_string(),
                        weather_code: 61,
                        temp_min_c: 11.0,
                        temp_max_c: 15.0,
                        precip_prob_max_pct: 70,
                    }],
                }),
            }
        }
    }

    struct MultiCityProviders {
        batch_calls: std::cell::Cell<usize>,
    }

    impl MultiCityProviders {
        fn new() -> Self {
            Self {
                batch_calls: std::cell::Cell::new(0),
            }
        }

        fn location_for(city: &str) -> Result<ResolvedLocation, ProviderError> {
            match city {
                "Taipei" => Ok(ResolvedLocation {
                    name: "Taipei".to_string(),
                    latitude: 25.033,
                    longitude: 121.5654,
                    timezone: "Asia/Taipei".to_string(),
                }),
                "Tokyo" => Ok(ResolvedLocation {
                    name: "Tokyo".to_string(),
                    latitude: 35.6762,
                    longitude: 139.6503,
                    timezone: "Asia/Tokyo".to_string(),
                }),
                _ => Err(ProviderError::NotFound(city.to_string())),
            }
        }

        fn forecast_for(city: &str) -> Result<ProviderForecast, ProviderError> {
            let now = Utc
                .with_ymd_and_hms(2026, 2, 11, 0, 0, 0)
                .single()
                .expect("time");

            match city {
                "Taipei" => Ok(ProviderForecast {
                    timezone: "Asia/Taipei".to_string(),
                    fetched_at: now,
                    days: vec![ProviderForecastDay {
                        date: "2026-02-11".to_string(),
                        weather_code: 3,
                        temp_min_c: 14.5,
                        temp_max_c: 20.1,
                        precip_prob_max_pct: 20,
                    }],
                }),
                "Tokyo" => Ok(ProviderForecast {
                    timezone: "Asia/Tokyo".to_string(),
                    fetched_at: now,
                    days: vec![ProviderForecastDay {
                        date: "2026-02-11".to_string(),
                        weather_code: 2,
                        temp_min_c: 5.2,
                        temp_max_c: 12.6,
                        precip_prob_max_pct: 10,
                    }],
                }),
                _ => Err(ProviderError::NotFound(city.to_string())),
            }
        }
    }

    impl ProviderApi for FakeProviders {
        fn geocode_city(&self, _city: &str) -> Result<ResolvedLocation, ProviderError> {
            self.geocode_result.clone()
        }

        fn fetch_open_meteo_forecast(
            &self,
            _lat: f64,
            _lon: f64,
            _forecast_days: usize,
        ) -> Result<ProviderForecast, ProviderError> {
            self.open_meteo_result.clone()
        }

        fn fetch_open_meteo_hourly_forecast(
            &self,
            _lat: f64,
            _lon: f64,
            _forecast_days: usize,
        ) -> Result<ProviderHourlyForecast, ProviderError> {
            self.open_meteo_hourly_result.clone()
        }

        fn fetch_met_no_forecast(
            &self,
            _lat: f64,
            _lon: f64,
            _forecast_days: usize,
        ) -> Result<ProviderForecast, ProviderError> {
            self.met_no_result.clone()
        }
    }

    impl ProviderApi for MultiCityProviders {
        fn geocode_city(&self, city: &str) -> Result<ResolvedLocation, ProviderError> {
            Self::location_for(city)
        }

        fn fetch_open_meteo_forecast(
            &self,
            _lat: f64,
            _lon: f64,
            _forecast_days: usize,
        ) -> Result<ProviderForecast, ProviderError> {
            Err(ProviderError::Transport("unused".to_string()))
        }

        fn fetch_open_meteo_forecasts_batch(
            &self,
            locations: &[ResolvedLocation],
            _forecast_days: usize,
        ) -> Result<Vec<ProviderForecast>, ProviderError> {
            self.batch_calls.set(self.batch_calls.get() + 1);
            locations
                .iter()
                .map(|location| Self::forecast_for(&location.name))
                .collect()
        }

        fn fetch_open_meteo_hourly_forecast(
            &self,
            _lat: f64,
            _lon: f64,
            _forecast_days: usize,
        ) -> Result<ProviderHourlyForecast, ProviderError> {
            Err(ProviderError::Transport("unused".to_string()))
        }

        fn fetch_met_no_forecast(
            &self,
            _lat: f64,
            _lon: f64,
            _forecast_days: usize,
        ) -> Result<ProviderForecast, ProviderError> {
            Err(ProviderError::Transport("unused".to_string()))
        }
    }

    fn config_in_tempdir() -> RuntimeConfig {
        RuntimeConfig {
            cache_dir: tempfile::tempdir().expect("tempdir").path().to_path_buf(),
            cache_ttl_secs: weather_cli::config::WEATHER_CACHE_TTL_SECS,
        }
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 11, 0, 5, 0)
            .single()
            .expect("time")
    }

    #[test]
    fn main_outputs_today_json_contract() {
        let cli = Cli::parse_from(["weather-cli", "today", "--city", "Taipei", "--json"]);

        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("today should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("v1")
        );
        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("weather.today")
        );
        assert_eq!(json.get("ok").and_then(Value::as_bool), Some(true));
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("period"))
                .and_then(Value::as_str),
            Some("today")
        );
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("location"))
                .and_then(|x| x.get("name"))
                .and_then(Value::as_str),
            Some("Taipei City")
        );
        assert!(
            json.get("result")
                .and_then(|result| result.get("forecast"))
                .is_some()
        );
    }

    #[test]
    fn main_outputs_week_json_contract() {
        let mut providers = FakeProviders::ok();
        providers.open_meteo_result = Ok(ProviderForecast {
            timezone: "Asia/Taipei".to_string(),
            fetched_at: Utc
                .with_ymd_and_hms(2026, 2, 11, 0, 0, 0)
                .single()
                .expect("time"),
            days: (0..7)
                .map(|i| ProviderForecastDay {
                    date: format!("2026-02-1{}", i + 1),
                    weather_code: 2,
                    temp_min_c: 14.0 + i as f64,
                    temp_max_c: 20.0 + i as f64,
                    precip_prob_max_pct: 10 + i as u8,
                })
                .collect(),
        });

        let cli = Cli::parse_from(["weather-cli", "week", "--city", "Taipei", "--json"]);

        let output =
            run_with(cli, &config_in_tempdir(), &providers, fixed_now).expect("week should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("v1")
        );
        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("weather.week")
        );
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("period"))
                .and_then(Value::as_str),
            Some("week")
        );
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("forecast"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(7)
        );
    }

    #[test]
    fn main_outputs_hourly_json_contract() {
        let cli = Cli::parse_from(["weather-cli", "hourly", "--city", "Tokyo", "--json"]);

        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("hourly should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("schema_version").and_then(Value::as_str),
            Some("v1")
        );
        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("weather.hourly")
        );
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("hourly"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn main_outputs_batch_json_contract_for_repeated_city_flags() {
        let cli = Cli::parse_from([
            "weather-cli",
            "today",
            "--city",
            "Taipei",
            "--city",
            "Tokyo",
            "--json",
        ]);
        let providers = MultiCityProviders::new();

        let output = run_with(cli, &config_in_tempdir(), &providers, fixed_now)
            .expect("batch json should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("weather.today")
        );
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("period"))
                .and_then(Value::as_str),
            Some("today")
        );
        assert_eq!(
            json.get("result")
                .and_then(|result| result.get("entries"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(providers.batch_calls.get(), 1);
    }

    #[test]
    fn main_accepts_negative_longitude_values() {
        let cli = Cli::try_parse_from([
            "weather-cli",
            "hourly",
            "--lat",
            "34.0522",
            "--lon",
            "-118.2437",
            "--json",
        ])
        .expect("negative longitude should parse");

        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("hourly should pass");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("command").and_then(Value::as_str),
            Some("weather.hourly")
        );
    }

    #[test]
    fn main_maps_invalid_input_to_user_error() {
        let cli = Cli::parse_from([
            "weather-cli",
            "today",
            "--city",
            "Taipei",
            "--lat",
            "25.0",
            "--lon",
            "121.5",
        ]);

        let err = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect_err("must fail");

        assert_eq!(err.kind, weather_cli::error::ErrorKind::User);
        assert_eq!(err.code, ERROR_CODE_USER_INVALID_INPUT);
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn main_maps_runtime_provider_failure() {
        let cli = Cli::parse_from(["weather-cli", "today", "--city", "Taipei", "--json"]);

        let providers = FakeProviders {
            open_meteo_result: Err(ProviderError::Transport("timeout".to_string())),
            met_no_result: Err(ProviderError::Http {
                status: 503,
                message: "down".to_string(),
            }),
            ..FakeProviders::ok()
        };

        let err =
            run_with(cli, &config_in_tempdir(), &providers, fixed_now).expect_err("must fail");
        assert_eq!(err.kind, weather_cli::error::ErrorKind::Runtime);
        assert_eq!(err.code, "runtime.provider_failed");
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn exit_code_mapping_user_and_runtime_are_stable() {
        assert_eq!(weather_cli::error::AppError::user("x").exit_code(), 2);
        assert_eq!(weather_cli::error::AppError::runtime("x").exit_code(), 1);
    }

    #[test]
    fn main_outputs_text_mode_when_json_flag_not_set() {
        let cli = Cli::parse_from(["weather-cli", "today", "--city", "Taipei"]);
        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("text mode");

        assert!(output.contains("Taipei City"));
        assert!(output.contains("source=open_meteo"));
        assert!(output.contains("2026-02-11 Wed"));
        assert!(output.contains("Cloudy"));
        assert!(output.contains("rain:20%"));
    }

    #[test]
    fn main_outputs_text_mode_in_zh_when_requested() {
        let cli = Cli::parse_from(["weather-cli", "today", "--city", "Taipei", "--lang", "zh"]);
        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("zh text mode");

        assert!(output.contains("陰天"));
        assert!(output.contains("2026-02-11 週三"));
        assert!(output.contains("降雨:20%"));
    }

    #[test]
    fn main_outputs_alfred_json_mode_when_requested() {
        let cli = Cli::parse_from([
            "weather-cli",
            "today",
            "--city",
            "Taipei",
            "--output",
            "alfred-json",
        ]);

        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("alfred mode");
        let json: Value = serde_json::from_str(&output).expect("json");

        let first_item = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .expect("first item");
        assert!(first_item.get("title").is_some());

        let second_item_title = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("title"))
            .and_then(Value::as_str);
        assert_eq!(second_item_title, Some("2026-02-11 Wed Cloudy 14.5~20.1°C"));

        let second_item_icon = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("icon"))
            .and_then(|icon| icon.get("path"))
            .and_then(Value::as_str);
        assert_eq!(second_item_icon, Some("assets/icons/weather/cloudy.png"));

        let second_item_icon_key = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("weather_meta"))
            .and_then(|meta| meta.get("icon_key"))
            .and_then(Value::as_str);
        assert_eq!(second_item_icon_key, Some("cloudy"));

        let second_item_weekday = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("weather_meta"))
            .and_then(|meta| meta.get("weekday_label"))
            .and_then(Value::as_str);
        assert_eq!(second_item_weekday, Some("Wed"));

        let second_item_timezone_display = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("weather_meta"))
            .and_then(|meta| meta.get("timezone_display"))
            .and_then(Value::as_str);
        assert_eq!(second_item_timezone_display, Some("Asia/Taipei (UTC+8)"));

        let second_item_utc_offset = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("weather_meta"))
            .and_then(|meta| meta.get("utc_offset_label"))
            .and_then(Value::as_str);
        assert_eq!(second_item_utc_offset, Some("UTC+8"));
    }

    #[test]
    fn main_outputs_alfred_json_mode_in_zh_when_requested() {
        let cli = Cli::parse_from([
            "weather-cli",
            "today",
            "--city",
            "Taipei",
            "--output",
            "alfred-json",
            "--lang",
            "zh",
        ]);

        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("alfred zh mode");
        let json: Value = serde_json::from_str(&output).expect("json");

        let second_item_title = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("title"))
            .and_then(Value::as_str);
        assert_eq!(second_item_title, Some("2026-02-11 週三 陰天 14.5~20.1°C"));

        let second_item_weekday = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("weather_meta"))
            .and_then(|meta| meta.get("weekday_label"))
            .and_then(Value::as_str);
        assert_eq!(second_item_weekday, Some("週三"));

        let second_item_timezone_display = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("weather_meta"))
            .and_then(|meta| meta.get("timezone_display"))
            .and_then(Value::as_str);
        assert_eq!(second_item_timezone_display, Some("Asia/Taipei (UTC+8)"));
    }

    #[test]
    fn main_outputs_batch_alfred_json_for_repeated_city_flags() {
        let cli = Cli::parse_from([
            "weather-cli",
            "today",
            "--city",
            "Taipei",
            "--city",
            "Tokyo",
            "--output",
            "alfred-json",
        ]);
        let providers = MultiCityProviders::new();

        let output =
            run_with(cli, &config_in_tempdir(), &providers, fixed_now).expect("batch alfred mode");
        let json: Value = serde_json::from_str(&output).expect("json");

        assert_eq!(
            json.get("items").and_then(Value::as_array).map(Vec::len),
            Some(2)
        );
        assert_eq!(
            json.get("items")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("title"))
                .and_then(Value::as_str),
            Some("Taipei Wed 14.5~20.1°C cloudy 20%")
        );
        assert_eq!(
            json.get("items")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("weather_meta"))
                .and_then(|meta| meta.get("timezone_display"))
                .and_then(Value::as_str),
            Some("Asia/Taipei (UTC+8)")
        );
        assert_eq!(
            json.get("items")
                .and_then(Value::as_array)
                .and_then(|items| items.get(1))
                .and_then(|item| item.get("title"))
                .and_then(Value::as_str),
            Some("Tokyo Wed 5.2~12.6°C partly cloudy 10%")
        );
        assert_eq!(providers.batch_calls.get(), 1);
    }

    #[test]
    fn main_outputs_hourly_alfred_json_mode_when_requested() {
        let cli = Cli::parse_from([
            "weather-cli",
            "hourly",
            "--city",
            "Tokyo",
            "--output",
            "alfred-json",
            "--hours",
            "1",
        ]);

        let output = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect("hourly alfred mode");
        let json: Value = serde_json::from_str(&output).expect("json");

        let second_item_title = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("title"))
            .and_then(Value::as_str);
        assert_eq!(
            second_item_title,
            Some("2026-02-11 Wed 00:00 Cloudy 16.1°C")
        );

        let second_item_icon = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("icon"))
            .and_then(|icon| icon.get("path"))
            .and_then(Value::as_str);
        assert_eq!(
            second_item_icon,
            Some("assets/icons/weather/cloudy-night.png")
        );

        let second_item_time = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("weather_meta"))
            .and_then(|meta| meta.get("time"))
            .and_then(Value::as_str);
        assert_eq!(second_item_time, Some("00:00"));

        let second_item_timezone_display = json
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("weather_meta"))
            .and_then(|meta| meta.get("timezone_display"))
            .and_then(Value::as_str);
        assert_eq!(second_item_timezone_display, Some("Asia/Taipei (UTC+8)"));
    }

    #[test]
    fn main_rejects_conflicting_json_flags() {
        let cli = Cli::parse_from([
            "weather-cli",
            "today",
            "--city",
            "Taipei",
            "--json",
            "--output",
            "human",
        ]);

        let err = run_with(cli, &config_in_tempdir(), &FakeProviders::ok(), fixed_now)
            .expect_err("must fail");
        assert_eq!(err.kind, weather_cli::error::ErrorKind::User);
        assert_eq!(err.code, ERROR_CODE_USER_OUTPUT_MODE_CONFLICT);
    }

    #[test]
    fn main_redacts_sensitive_error_fragments() {
        let redacted = redact_sensitive(
            "authorization: Bearer abc token=xyz secret=hidden client_secret:demo",
        );
        assert!(!redacted.contains("abc"));
        assert!(!redacted.contains("xyz"));
        assert!(!redacted.contains("hidden"));
        assert!(!redacted.contains("demo"));
        assert!(redacted.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn main_help_flag_is_supported() {
        let help = Cli::try_parse_from(["weather-cli", "--help"]).expect_err("help");
        assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
    }
}
