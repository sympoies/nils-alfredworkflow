use serde_json::{Value, json};

pub use workflow_common::{CliErrorKind as ErrorKind, redact_sensitive};

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
pub const ERROR_CODE_USER_CALENDAR_INVALID_INPUT: &str = "NILS_GOOGLE_015";
pub const ERROR_CODE_RUNTIME_CALENDAR_NOT_FOUND: &str = "NILS_GOOGLE_016";
pub const ERROR_CODE_RUNTIME_CALENDAR_FAILED: &str = "NILS_GOOGLE_017";

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
        self.kind.exit_code()
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

    pub fn invalid_calendar_input(message: impl Into<String>) -> Self {
        Self::user(
            ERROR_CODE_USER_CALENDAR_INVALID_INPUT,
            message,
            Some(json!({ "kind": "calendar_invalid_input" })),
        )
    }

    pub fn calendar_not_found(entity: &str, id: &str) -> Self {
        Self::runtime(
            ERROR_CODE_RUNTIME_CALENDAR_NOT_FOUND,
            format!("{entity} `{id}` not found"),
            Some(json!({
                "kind": "calendar_not_found",
                "entity": entity,
                "id": id,
            })),
        )
    }

    pub fn calendar_failure(message: impl Into<String>) -> Self {
        Self::runtime(
            ERROR_CODE_RUNTIME_CALENDAR_FAILED,
            message,
            Some(json!({ "kind": "calendar_runtime_failure" })),
        )
    }
}
