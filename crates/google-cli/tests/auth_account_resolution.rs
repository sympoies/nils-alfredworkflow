use std::collections::BTreeMap;

use google_cli::auth::account::{ResolutionSource, resolve_account};
use google_cli::auth::config::AccountMetadata;

fn mk_metadata(accounts: &[&str], default_account: Option<&str>) -> AccountMetadata {
    AccountMetadata {
        version: 1,
        default_account: default_account.map(ToOwned::to_owned),
        aliases: BTreeMap::new(),
        accounts: accounts.iter().map(|value| (*value).to_string()).collect(),
    }
}

#[test]
fn resolves_explicit_account() {
    let metadata = mk_metadata(&["a@example.com", "b@example.com"], None);
    let resolved = resolve_account(Some("b@example.com"), &metadata).expect("resolved");
    assert_eq!(resolved.account, "b@example.com");
    assert_eq!(resolved.source, ResolutionSource::Explicit);
}

#[test]
fn resolves_alias_then_default_then_single() {
    let mut metadata = mk_metadata(&["a@example.com", "b@example.com"], Some("a@example.com"));
    metadata
        .aliases
        .insert("work".to_string(), "b@example.com".to_string());

    let alias = resolve_account(Some("work"), &metadata).expect("alias");
    assert_eq!(alias.account, "b@example.com");
    assert_eq!(alias.source, ResolutionSource::Alias);

    let default_account = resolve_account(None, &metadata).expect("default");
    assert_eq!(default_account.account, "a@example.com");
    assert_eq!(default_account.source, ResolutionSource::Default);

    let single = resolve_account(None, &mk_metadata(&["solo@example.com"], None)).expect("single");
    assert_eq!(single.account, "solo@example.com");
    assert_eq!(single.source, ResolutionSource::Single);
}

#[test]
fn returns_ambiguous_error_without_default() {
    let metadata = mk_metadata(&["a@example.com", "b@example.com"], None);
    let error = resolve_account(None, &metadata).expect_err("ambiguous");
    assert_eq!(
        error.code(),
        google_cli::error::ERROR_CODE_USER_AUTH_AMBIGUOUS_ACCOUNT
    );
}

#[test]
fn returns_invalid_input_for_unknown_account_or_empty_store() {
    let metadata = mk_metadata(&["a@example.com"], None);
    let unknown = resolve_account(Some("missing@example.com"), &metadata).expect_err("unknown");
    assert_eq!(
        unknown.code(),
        google_cli::error::ERROR_CODE_USER_AUTH_INVALID_INPUT
    );

    let empty = resolve_account(None, &mk_metadata(&[], None)).expect_err("empty");
    assert_eq!(
        empty.code(),
        google_cli::error::ERROR_CODE_USER_AUTH_INVALID_INPUT
    );
}
