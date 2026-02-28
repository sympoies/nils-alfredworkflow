use std::collections::BTreeMap;

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackPayload {
    pub code: String,
    pub state: String,
}

pub fn parse_callback_url(input: &str) -> Result<CallbackPayload, AppError> {
    let query = if let Some((_, query)) = input.split_once('?') {
        query
    } else {
        input
    };

    let mut values = BTreeMap::new();
    for pair in query.split('&') {
        if pair.trim().is_empty() {
            continue;
        }
        if let Some((key, value)) = pair.split_once('=') {
            values.insert(key.to_string(), value.to_string());
        }
    }

    let code = values
        .get("code")
        .cloned()
        .ok_or_else(|| AppError::invalid_auth_input("callback payload is missing `code`"))?;
    let state = values
        .get("state")
        .cloned()
        .ok_or_else(|| AppError::invalid_auth_input("callback payload is missing `state`"))?;

    Ok(CallbackPayload { code, state })
}

#[cfg(test)]
mod tests {
    use super::parse_callback_url;

    #[test]
    fn parse_callback_accepts_full_url() {
        let payload =
            parse_callback_url("http://127.0.0.1/callback?code=abc&state=xyz").expect("payload");
        assert_eq!(payload.code, "abc");
        assert_eq!(payload.state, "xyz");
    }

    #[test]
    fn parse_callback_rejects_missing_code() {
        let error = parse_callback_url("state=xyz").expect_err("missing code");
        assert_eq!(
            error.code(),
            crate::error::ERROR_CODE_USER_AUTH_INVALID_INPUT
        );
    }
}
