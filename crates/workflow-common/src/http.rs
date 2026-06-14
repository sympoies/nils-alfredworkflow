use std::time::Duration;

use reqwest::blocking::Client;

/// Builds a blocking reqwest client while preserving caller-owned HTTP policy.
///
/// Pass `None` for values that remain local to each request, such as
/// per-request user-agent headers.
pub fn build_blocking_client(
    user_agent: Option<&str>,
    timeout: Option<Duration>,
) -> Result<Client, reqwest::Error> {
    let mut builder = Client::builder();
    if let Some(user_agent) = user_agent {
        builder = builder.user_agent(user_agent);
    }
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_blocking_client_accepts_user_agent_and_timeout() {
        let client = build_blocking_client(
            Some("nils-alfredworkflow-test/1.0"),
            Some(Duration::from_millis(100)),
        );

        assert!(client.is_ok());
    }

    #[test]
    fn build_blocking_client_rejects_invalid_user_agent() {
        let client = build_blocking_client(Some("invalid\nagent"), None);

        assert!(client.is_err());
    }
}
