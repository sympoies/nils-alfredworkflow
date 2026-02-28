use std::env;

use crate::error::AppError;

pub const GOOGLE_CLI_AUTH_DISABLE_BROWSER_ENV: &str = "GOOGLE_CLI_AUTH_DISABLE_BROWSER";

pub fn maybe_launch(url: &str) -> Result<(), AppError> {
    let disabled = env::var(GOOGLE_CLI_AUTH_DISABLE_BROWSER_ENV)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);

    if disabled {
        return Ok(());
    }

    open::that_detached(url)
        .map_err(|error| AppError::auth_store_failure(format!("failed to open browser: {error}")))
}
