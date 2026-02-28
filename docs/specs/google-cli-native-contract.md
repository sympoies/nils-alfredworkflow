# Google CLI native contract

## Purpose

Define the native Rust command contract for `google-cli` over the repo-scoped Google surface: `auth`, `gmail`, and
`drive`.

- Package: `nils-google-cli`
- Binary: `google-cli`

## Scope

- Native command ownership includes:
  - `auth credentials set|list`
  - `auth add|list|status|remove|alias|manage`
  - `gmail search|get|send|thread get|thread modify`
  - `drive ls|search|get|download|upload`
- Out of scope:
  - browser account-manager UI rebuild
  - non-scoped domains (`calendar`, `chat`, `docs`, `forms`, `people`, and similar)
  - service-account flows in this phase unless explicitly added later

## Output and error envelope

- Native responses keep repository CLI envelope behavior (`schema_version`, `command`, `ok`, and `result`/`error`).
- Native runtime error taxonomy continues to separate user errors from runtime failures.
- Native command IDs remain stable and service-scoped (`google.auth.*`, `google.gmail.*`, `google.drive.*`).

## OAuth modes

`auth add` supports three native modes:

- `loopback`: launch browser, receive callback on loopback listener, exchange code.
- `manual`: display auth URL and accept pasted code for exchange.
- `remote`: run explicit step-based exchange where state is generated, persisted, and validated by native runtime.

Required behavior:

- Remote/manual state tracking must prevent wrapper-era state mismatch failures.
- Browser launch is an auth helper concern only; account-manager UI is not opened.

## Account and default resolution semantics

Native account targeting order for auth-adjacent commands:

1. explicit `--account`
2. alias mapping
3. configured default account
4. single stored account when unambiguous
5. deterministic error when none of the above resolve

`auth status` contract:

- `auth status` without `--account` must apply the same default account resolution order.
- `auth status` must never return an empty account payload when multiple accounts exist without a default account.
- Ambiguous-account failures must include explicit corrective guidance.

## `auth manage` contract

- `auth manage` is terminal-native only.
- No browser account-manager page is launched.
- The command returns account summary/help output and, when appropriate, guidance to use `auth alias` and default-account
  configuration.

## Service behavior contract

- `gmail` and `drive` commands execute through native client modules owned by this crate.
- Generated API clients are the primary transport path.
- `reqwest` is an allowed fallback path when generated coverage is incomplete for a command edge case.

## Compatibility notes

- This contract replaces wrapper pass-through ownership language for future implementation work.
- Sprint 1 freezes behavior definitions; implementation arrives incrementally in later sprints.
