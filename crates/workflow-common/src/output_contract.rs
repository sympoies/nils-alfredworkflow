pub const ENVELOPE_SCHEMA_VERSION: &str = "cli-envelope@v1";

#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[cfg_attr(feature = "clap", value(rename_all = "kebab-case"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    AlfredJson,
}

impl OutputMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::AlfredJson => "alfred-json",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "human" => Some(Self::Human),
            "json" => Some(Self::Json),
            "alfred-json" => Some(Self::AlfredJson),
            _ => None,
        }
    }
}

/// Clap value-enum mirror of the `--output` flag offered by Alfred Script
/// Filter CLIs that expose only `json` and `alfred-json`.
///
/// This centralizes what used to be a byte-identical `OutputModeArg` copy in
/// every Script Filter CLI crate. Consumers enable the `clap` feature and
/// import it, conventionally as
/// `use workflow_common::ScriptFilterOutputModeArg as OutputModeArg;`, then
/// convert into [`OutputMode`] via `From`.
#[cfg(feature = "clap")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ScriptFilterOutputModeArg {
    /// Service envelope JSON (`json`).
    Json,
    /// Alfred Script Filter JSON (`alfred-json`); the Script Filter default.
    #[default]
    AlfredJson,
}

#[cfg(feature = "clap")]
impl From<ScriptFilterOutputModeArg> for OutputMode {
    fn from(value: ScriptFilterOutputModeArg) -> Self {
        match value {
            ScriptFilterOutputModeArg::Json => Self::Json,
            ScriptFilterOutputModeArg::AlfredJson => Self::AlfredJson,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopePayloadKind {
    Result,
    Results,
}

pub fn build_feedback_result_envelope(
    command: &str,
    payload: &alfred_core::Feedback,
) -> Result<String, String> {
    let payload_json = payload.to_json().map_err(|error| error.to_string())?;
    Ok(build_success_envelope(
        command,
        EnvelopePayloadKind::Result,
        &payload_json,
    ))
}

pub fn build_success_envelope(
    command: &str,
    payload_kind: EnvelopePayloadKind,
    payload_json: &str,
) -> String {
    let payload_key = match payload_kind {
        EnvelopePayloadKind::Result => "result",
        EnvelopePayloadKind::Results => "results",
    };

    format!(
        "{{\"schema_version\":\"{}\",\"command\":\"{}\",\"ok\":true,\"{}\":{}}}",
        ENVELOPE_SCHEMA_VERSION,
        escape_json_string(command),
        payload_key,
        payload_json
    )
}

pub fn build_error_envelope(
    command: &str,
    code: &str,
    message: &str,
    details_json: Option<&str>,
) -> String {
    let safe_message = escape_json_string(&redact_sensitive(message));
    let mut output = format!(
        "{{\"schema_version\":\"{}\",\"command\":\"{}\",\"ok\":false,\"error\":{{\"code\":\"{}\",\"message\":\"{}\"",
        ENVELOPE_SCHEMA_VERSION,
        escape_json_string(command),
        escape_json_string(code),
        safe_message
    );

    if let Some(details) = details_json {
        output.push_str(",\"details\":");
        output.push_str(details);
    }

    output.push_str("}}");
    output
}

pub fn build_error_details_json(kind: &str, exit_code: i32) -> String {
    format!(
        "{{\"kind\":\"{}\",\"exit_code\":{}}}",
        escape_json_string(kind),
        exit_code
    )
}

pub fn build_alfred_error_feedback(code: &str, message: &str) -> String {
    let safe_message = redact_sensitive(message);
    alfred_core::Feedback::single_error(code, safe_message)
        .to_json()
        .unwrap_or_else(|_| {
            "{\"items\":[{\"title\":\"Error\",\"subtitle\":\"failed to serialize error output\",\"valid\":false}]}".to_string()
        })
}

pub fn redact_sensitive(input: &str) -> String {
    let mut output = input.to_string();

    for pattern in [
        "token=",
        "token:",
        "secret=",
        "secret:",
        "client_secret=",
        "client_secret:",
        "password=",
        "password:",
        "apikey=",
        "apikey:",
        "api_key=",
        "api_key:",
        "authorization=",
        "authorization:",
    ] {
        output = redact_after_pattern(&output, pattern);
    }

    redact_bearer_token(&output)
}

fn redact_after_pattern(input: &str, pattern: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let pattern_lower = pattern.to_ascii_lowercase();
    let is_authorization_pattern = pattern_lower.starts_with("authorization");
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(found) = lower[cursor..].find(&pattern_lower) {
        let start = cursor + found;
        let value_start = start + pattern.len();
        let value_content_start = skip_whitespace(input, value_start);
        let (redaction_start, value_end) = if is_authorization_pattern {
            // Authorization headers are `<scheme> <credential>` (e.g.
            // `Bearer xyz`, `Basic dXNlcjpwYXNz`, `Negotiate YII...`). Redact the
            // credential regardless of the scheme; when no scheme word is present
            // the whole value is the credential. The previous code special-cased
            // only `bearer `, so `Basic`/`Negotiate`/etc. leaked their credential.
            let credential_start = authorization_credential_start(input, value_content_start);
            (credential_start, find_value_end(input, credential_start))
        } else {
            (
                value_content_start,
                find_value_end(input, value_content_start),
            )
        };

        output.push_str(&input[cursor..redaction_start]);
        if redaction_start < value_end {
            output.push_str("[REDACTED]");
        }

        cursor = value_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn redact_bearer_token(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let pattern = "bearer ";
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(found) = lower[cursor..].find(pattern) {
        let start = cursor + found;
        let value_start = start + pattern.len();
        let value_end = find_value_end(input, value_start);

        output.push_str(&input[cursor..value_start]);
        if value_start < value_end {
            output.push_str("[REDACTED]");
        }

        cursor = value_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn skip_whitespace(input: &str, mut index: usize) -> usize {
    let bytes = input.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

/// Returns the byte index at which the credential begins inside an
/// `Authorization` value. Authorization headers are `<scheme> <credential>`
/// (e.g. `Bearer xyz`, `Basic dXNlcjpwYXNz`, `Negotiate YII...`). When a scheme
/// word (a run of ASCII letters followed by whitespace and a credential token)
/// is present, the credential is everything after it; otherwise the whole value
/// is treated as the credential. Indices stay on ASCII boundaries so the result
/// is always a valid UTF-8 slice point.
fn authorization_credential_start(input: &str, value_start: usize) -> usize {
    let bytes = input.as_bytes();
    let mut index = value_start;
    while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
        index += 1;
    }

    if index > value_start && index < bytes.len() && bytes[index].is_ascii_whitespace() {
        let credential_start = skip_whitespace(input, index);
        if credential_start < bytes.len() {
            return credential_start;
        }
    }

    value_start
}

fn find_value_end(input: &str, mut index: usize) -> usize {
    let bytes = input.as_bytes();
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() || matches!(byte, b'&' | b',' | b';' | b')' | b']' | b'}') {
            break;
        }
        index += 1;
    }
    index
}

fn escape_json_string(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0C}' => escaped.push_str("\\f"),
            c if c < '\u{20}' => {
                let code = c as u32;
                escaped.push_str(&format!("\\u{code:04x}"));
            }
            c => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_mode_parse_accepts_canonical_values_only() {
        assert_eq!(OutputMode::parse("human"), Some(OutputMode::Human));
        assert_eq!(OutputMode::parse("json"), Some(OutputMode::Json));
        assert_eq!(
            OutputMode::parse("alfred-json"),
            Some(OutputMode::AlfredJson)
        );
        assert_eq!(OutputMode::parse("text"), None);
        assert_eq!(OutputMode::parse("alfred"), None);
        assert_eq!(OutputMode::parse("alfred_json"), None);
        assert_eq!(OutputMode::parse("invalid"), None);
    }

    #[cfg(feature = "clap")]
    #[test]
    fn output_mode_clap_value_enum_exposes_full_contract() {
        let variants = <OutputMode as clap::ValueEnum>::value_variants();
        assert_eq!(
            variants,
            &[OutputMode::Human, OutputMode::Json, OutputMode::AlfredJson]
        );
        assert_eq!(
            <OutputMode as clap::ValueEnum>::to_possible_value(&OutputMode::Human)
                .expect("human value")
                .get_name(),
            "human"
        );
        assert_eq!(
            <OutputMode as clap::ValueEnum>::to_possible_value(&OutputMode::Json)
                .expect("json value")
                .get_name(),
            "json"
        );
        assert_eq!(
            <OutputMode as clap::ValueEnum>::to_possible_value(&OutputMode::AlfredJson)
                .expect("alfred-json value")
                .get_name(),
            "alfred-json"
        );
    }

    #[test]
    fn envelope_builders_emit_required_keys() {
        let success =
            build_success_envelope("weather.today", EnvelopePayloadKind::Result, "{\"foo\":1}");
        assert!(success.contains("\"schema_version\":\"cli-envelope@v1\""));
        assert!(success.contains("\"command\":\"weather.today\""));
        assert!(success.contains("\"ok\":true"));
        assert!(success.contains("\"result\":{\"foo\":1}"));

        let details = build_error_details_json("runtime", 1);
        let failure = build_error_envelope(
            "weather.today",
            "NILS_WEATHER_002",
            "token=abc",
            Some(&details),
        );
        assert!(failure.contains("\"ok\":false"));
        assert!(failure.contains("\"code\":\"NILS_WEATHER_002\""));
        assert!(failure.contains("\"details\":{\"kind\":\"runtime\",\"exit_code\":1}"));
        assert!(failure.contains("token=[REDACTED]"));
    }

    #[test]
    fn feedback_envelope_builder_wraps_feedback_payload() {
        let feedback = alfred_core::Feedback::new(vec![alfred_core::Item::new("Alpha")]);
        let envelope =
            build_feedback_result_envelope("workflow.script-filter", &feedback).expect("envelope");
        assert!(envelope.contains("\"ok\":true"));
        assert!(envelope.contains("\"result\":{\"items\":[{\"title\":\"Alpha\""));
    }

    #[test]
    fn alfred_error_feedback_builder_redacts_sensitive_message() {
        let payload = build_alfred_error_feedback("NILS_COMMON_005", "token=abc123");
        assert!(payload.contains("Error [NILS_COMMON_005]"));
        assert!(payload.contains("token=[REDACTED]"));
        assert!(!payload.contains("abc123"));
    }

    #[test]
    fn redaction_masks_sensitive_patterns() {
        let raw = "authorization: Bearer top.secret token=abc123 client_secret:zzz";
        let redacted = redact_sensitive(raw);

        assert!(!redacted.contains("top.secret"));
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("zzz"));
        assert!(redacted.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn redaction_masks_non_bearer_authorization_schemes() {
        // Regression: only `Bearer` was special-cased, so `Basic`/`Negotiate`
        // (and any other scheme) leaked the credential after the scheme word.
        let basic = redact_sensitive("authorization: Basic dXNlcjpwYXNzd29yZA==");
        assert!(
            !basic.contains("dXNlcjpwYXNzd29yZA=="),
            "Basic credential leaked: {basic}"
        );
        assert!(basic.contains("Basic [REDACTED]"), "got: {basic}");

        let negotiate = redact_sensitive("Authorization: Negotiate YIIZsecrettoken");
        assert!(
            !negotiate.contains("YIIZsecrettoken"),
            "Negotiate credential leaked: {negotiate}"
        );
        assert!(
            negotiate.contains("Negotiate [REDACTED]"),
            "got: {negotiate}"
        );

        // A scheme-less authorization value must redact the whole credential.
        let schemeless = redact_sensitive("authorization: rawsecretvalue");
        assert!(
            !schemeless.contains("rawsecretvalue"),
            "scheme-less credential leaked: {schemeless}"
        );
    }
}
