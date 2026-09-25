use std::collections::BTreeMap;

use reqwest::Url;

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

    // Decoded, because a callback copied from an address bar carries the code
    // percent-encoded (`4%2F0A...`) and Google only accepts it decoded.
    let parsed = Url::parse(&format!("http://callback.invalid/?{query}"))
        .map_err(|_| AppError::invalid_auth_input("callback payload is not a valid query"))?;
    let values = parsed
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<BTreeMap<_, _>>();

    if let Some(error) = values.get("error") {
        return Err(AppError::invalid_auth_input(format!(
            "authorization was not granted: {}",
            error.chars().take(64).collect::<String>()
        )));
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
    fn parse_callback_decodes_an_address_bar_code() {
        let payload =
            parse_callback_url("http://localhost/?state=xyz&code=4%2F0Aabc-_&scope=email%20openid")
                .expect("payload");
        assert_eq!(payload.code, "4/0Aabc-_");
        assert_eq!(payload.state, "xyz");
    }

    #[test]
    fn parse_callback_reports_a_denied_consent() {
        let error = parse_callback_url("http://localhost/?error=access_denied&state=xyz")
            .expect_err("denied");
        assert!(error.message().contains("access_denied"));
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
