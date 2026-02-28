use super::config::OAuthClientCredentials;
use crate::error::AppError;

pub fn parse_set_args(args: &[String]) -> Result<OAuthClientCredentials, AppError> {
    let mut client_id = None;
    let mut client_secret = None;
    let mut auth_uri = None;
    let mut token_uri = None;
    let mut redirect_uri = None;

    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if index + 1 >= args.len() {
            return Err(AppError::invalid_auth_input(format!(
                "missing value for `{flag}`"
            )));
        }
        let value = args[index + 1].clone();

        match flag {
            "--client-id" => client_id = Some(value),
            "--client-secret" => client_secret = Some(value),
            "--auth-uri" => auth_uri = Some(value),
            "--token-uri" => token_uri = Some(value),
            "--redirect-uri" => redirect_uri = Some(value),
            unknown => {
                return Err(AppError::invalid_auth_input(format!(
                    "unknown credentials option `{unknown}`"
                )));
            }
        }
        index += 2;
    }

    let client_id = client_id.ok_or_else(|| {
        AppError::invalid_auth_input(
            "`auth credentials set` requires --client-id and --client-secret",
        )
    })?;
    let client_secret = client_secret.ok_or_else(|| {
        AppError::invalid_auth_input(
            "`auth credentials set` requires --client-id and --client-secret",
        )
    })?;

    let mut credentials = OAuthClientCredentials::with_defaults(client_id, client_secret);
    if let Some(value) = auth_uri {
        credentials.auth_uri = value;
    }
    if let Some(value) = token_uri {
        credentials.token_uri = value;
    }
    if let Some(value) = redirect_uri {
        credentials.redirect_uri = value;
    }

    Ok(credentials)
}
