use super::config::AccountMetadata;
use super::defaults::resolve_default_account;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionSource {
    Explicit,
    Alias,
    Default,
    Single,
}

impl ResolutionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolutionSource::Explicit => "explicit",
            ResolutionSource::Alias => "alias",
            ResolutionSource::Default => "default",
            ResolutionSource::Single => "single",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAccount {
    pub account: String,
    pub source: ResolutionSource,
}

pub fn resolve_account(
    requested: Option<&str>,
    metadata: &AccountMetadata,
) -> Result<ResolvedAccount, AppError> {
    if let Some(requested) = requested {
        if let Some(mapped) = metadata.aliases.get(requested) {
            return Ok(ResolvedAccount {
                account: mapped.clone(),
                source: ResolutionSource::Alias,
            });
        }

        if metadata.accounts.iter().any(|account| account == requested) {
            return Ok(ResolvedAccount {
                account: requested.to_string(),
                source: ResolutionSource::Explicit,
            });
        }

        return Err(AppError::invalid_auth_input(format!(
            "unknown account or alias `{requested}`"
        )));
    }

    if metadata.accounts.is_empty() {
        return Err(AppError::invalid_auth_input(
            "no accounts are configured; run `auth add <email>` first",
        ));
    }

    if let Some(default_account) = resolve_default_account(metadata) {
        return Ok(ResolvedAccount {
            account: default_account,
            source: ResolutionSource::Default,
        });
    }

    if metadata.accounts.len() == 1 {
        return Ok(ResolvedAccount {
            account: metadata.accounts[0].clone(),
            source: ResolutionSource::Single,
        });
    }

    Err(AppError::ambiguous_account(&metadata.accounts))
}
