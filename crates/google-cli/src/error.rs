use serde_json::{Value, json};

pub const ERROR_CODE_USER_INVALID_OUTPUT_FLAGS: &str = "NILS_GOOGLE_001";
// Reserved after native migration removed wrapper runtime errors.
pub const ERROR_CODE_RUNTIME_MISSING_GOG: &str = "NILS_GOOGLE_002";
// Reserved after native migration removed wrapper runtime errors.
pub const ERROR_CODE_RUNTIME_GOG_FAILED: &str = "NILS_GOOGLE_003";
// Reserved after native migration removed wrapper runtime errors.
pub const ERROR_CODE_RUNTIME_INVALID_JSON: &str = "NILS_GOOGLE_004";
pub const ERROR_CODE_USER_AUTH_INVALID_INPUT: &str = "NILS_GOOGLE_005";
pub const ERROR_CODE_USER_AUTH_AMBIGUOUS_ACCOUNT: &str = "NILS_GOOGLE_006";
pub const ERROR_CODE_RUNTIME_AUTH_STORE_FAILED: &str = "NILS_GOOGLE_007";
pub const ERROR_CODE_RUNTIME_AUTH_STATE_MISMATCH: &str = "NILS_GOOGLE_008";
pub const ERROR_CODE_USER_GMAIL_INVALID_INPUT: &str = "NILS_GOOGLE_009";
pub const ERROR_CODE_RUNTIME_GMAIL_NOT_FOUND: &str = "NILS_GOOGLE_010";
pub const ERROR_CODE_RUNTIME_GMAIL_FAILED: &str = "NILS_GOOGLE_011";
pub const ERROR_CODE_USER_DRIVE_INVALID_INPUT: &str = "NILS_GOOGLE_012";
pub const ERROR_CODE_RUNTIME_DRIVE_NOT_FOUND: &str = "NILS_GOOGLE_013";
pub const ERROR_CODE_RUNTIME_DRIVE_FAILED: &str = "NILS_GOOGLE_014";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    User,
    Runtime,
}

#[derive(Debug, Clone)]
pub struct AppError {
    kind: ErrorKind,
    code: &'static str,
    message: String,
    details: Option<Value>,
}

impl AppError {
    pub fn new(
        kind: ErrorKind,
        code: &'static str,
        message: impl Into<String>,
        details: Option<Value>,
    ) -> Self {
        Self {
            kind,
            code,
            message: message.into(),
            details,
        }
    }

    pub fn user(code: &'static str, message: impl Into<String>, details: Option<Value>) -> Self {
        Self::new(ErrorKind::User, code, message, details)
    }

    pub fn runtime(code: &'static str, message: impl Into<String>, details: Option<Value>) -> Self {
        Self::new(ErrorKind::Runtime, code, message, details)
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn exit_code(&self) -> i32 {
        match self.kind {
            ErrorKind::User => 2,
            ErrorKind::Runtime => 1,
        }
    }

    pub fn invalid_output_flags(message: impl Into<String>) -> Self {
        Self::user(
            ERROR_CODE_USER_INVALID_OUTPUT_FLAGS,
            message,
            Some(json!({ "kind": "user" })),
        )
    }

    pub fn invalid_auth_input(message: impl Into<String>) -> Self {
        Self::user(
            ERROR_CODE_USER_AUTH_INVALID_INPUT,
            message,
            Some(json!({ "kind": "auth_invalid_input" })),
        )
    }

    pub fn ambiguous_account(accounts: &[String]) -> Self {
        Self::user(
            ERROR_CODE_USER_AUTH_AMBIGUOUS_ACCOUNT,
            "multiple accounts exist; pass --account, set a default account, or remove ambiguity",
            Some(json!({
                "kind": "auth_ambiguous_account",
                "accounts": accounts,
            })),
        )
    }

    pub fn auth_store_failure(message: impl Into<String>) -> Self {
        Self::runtime(
            ERROR_CODE_RUNTIME_AUTH_STORE_FAILED,
            message,
            Some(json!({ "kind": "auth_store_failure" })),
        )
    }

    pub fn auth_state_mismatch(expected: &str, received: &str) -> Self {
        Self::runtime(
            ERROR_CODE_RUNTIME_AUTH_STATE_MISMATCH,
            "remote auth state mismatch; restart with --remote --step 1",
            Some(json!({
                "kind": "auth_state_mismatch",
                "expected": expected,
                "received": received,
            })),
        )
    }

    pub fn invalid_gmail_input(message: impl Into<String>) -> Self {
        Self::user(
            ERROR_CODE_USER_GMAIL_INVALID_INPUT,
            message,
            Some(json!({ "kind": "gmail_invalid_input" })),
        )
    }

    pub fn gmail_not_found(entity: &str, id: &str) -> Self {
        Self::runtime(
            ERROR_CODE_RUNTIME_GMAIL_NOT_FOUND,
            format!("{entity} `{id}` not found"),
            Some(json!({
                "kind": "gmail_not_found",
                "entity": entity,
                "id": id,
            })),
        )
    }

    pub fn gmail_failure(message: impl Into<String>) -> Self {
        Self::runtime(
            ERROR_CODE_RUNTIME_GMAIL_FAILED,
            message,
            Some(json!({ "kind": "gmail_runtime_failure" })),
        )
    }

    pub fn invalid_drive_input(message: impl Into<String>) -> Self {
        Self::user(
            ERROR_CODE_USER_DRIVE_INVALID_INPUT,
            message,
            Some(json!({ "kind": "drive_invalid_input" })),
        )
    }

    pub fn drive_not_found(entity: &str, id: &str) -> Self {
        Self::runtime(
            ERROR_CODE_RUNTIME_DRIVE_NOT_FOUND,
            format!("{entity} `{id}` not found"),
            Some(json!({
                "kind": "drive_not_found",
                "entity": entity,
                "id": id,
            })),
        )
    }

    pub fn drive_failure(message: impl Into<String>) -> Self {
        Self::runtime(
            ERROR_CODE_RUNTIME_DRIVE_FAILED,
            message,
            Some(json!({ "kind": "drive_runtime_failure" })),
        )
    }
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
        "authorization=",
        "authorization:",
        "bearer ",
    ] {
        output = redact_after_pattern(&output, pattern);
    }

    output
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
        let (redaction_start, value_end) = if is_authorization_pattern
            && input[value_content_start..]
                .to_ascii_lowercase()
                .starts_with("bearer ")
        {
            let bearer_start = value_content_start + "bearer ".len();
            (bearer_start, find_value_end(input, bearer_start))
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

fn skip_whitespace(input: &str, mut index: usize) -> usize {
    let bytes = input.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
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

#[cfg(test)]
mod tests {
    use super::redact_sensitive;

    #[test]
    fn redact_sensitive_masks_common_tokens() {
        let text = "secret=abcd token:1234 authorization=Bearer zzzz";
        let redacted = redact_sensitive(text);
        assert!(!redacted.contains("abcd"));
        assert!(!redacted.contains("1234"));
        assert!(!redacted.contains("zzzz"));
        assert!(redacted.contains("[REDACTED]"));
    }
}
