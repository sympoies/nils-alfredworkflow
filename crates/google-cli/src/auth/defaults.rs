use super::config::AccountMetadata;

pub fn resolve_default_account(metadata: &AccountMetadata) -> Option<String> {
    metadata.default_account.as_ref().and_then(|candidate| {
        metadata
            .accounts
            .iter()
            .find(|account| *account == candidate)
            .cloned()
    })
}

pub fn ensure_default_account(metadata: &mut AccountMetadata, account: &str) {
    if metadata.default_account.is_none() {
        metadata.default_account = Some(account.to_string());
    }
}
