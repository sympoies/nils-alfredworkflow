# Google CLI native crate survey

## Decision summary

This survey evaluates the Sprint 1 native dependency stack for `google-cli` and records exact pins used for the
compile-only probe.

## Primary stack (selected)

| Crate | Exact pin | Role | Decision notes |
| --- | --- | --- | --- |
| `google-gmail1` | `=7.0.0+20251215` | Generated Gmail API client | Selected as the primary Gmail surface. |
| `google-drive3` | `=7.0.0+20251218` | Generated Drive API client | Selected as the primary Drive surface. |
| `google-apis-common` | `=8.0.0` | Shared auth/http glue used by generated clients | Selected to align with generated client internals. |
| `yup-oauth2` | `=12.1.2` | OAuth installed/manual exchange support | Selected for loopback/manual/remote auth modes. |
| `keyring` | `=3.6.2` | Token persistence in system keychain | Selected; stable line avoids 4.x release-candidate risk. |
| `directories` | `=6.0.0` | App config/data path discovery | Selected for deterministic local config locations. |
| `open` | `=5.3.3` | Browser launch for interactive auth | Selected for cross-platform browser handoff. |
| `mail-builder` | `=0.4.4` | MIME assembly for Gmail send payloads | Selected for native message construction. |
| `mime_guess` | `=2.0.5` | Attachment content-type inference | Selected for upload/send UX parity. |
| `wiremock` | `=0.6.5` | HTTP-level tests for fallback/native adapters | Selected for deterministic native test doubles. |

## Fallback stack (documented)

| Crate | Exact pin | When to use | Notes |
| --- | --- | --- | --- |
| `reqwest` | `=0.12.28` | Fallback when generated clients cannot cover required behavior | Enables hand-written REST calls while preserving auth/account semantics. |

## Rejected or deferred options

- `keyring` `4.0.0-rc.*` line: deferred because it is release-candidate software with higher toolchain requirements.
- Full handwritten Gmail/Drive clients from day one: rejected for Sprint 1 because generated clients plus selective
  fallback provide better delivery speed.
- Rebuilding OAuth and token-exchange primitives from scratch: rejected in favor of `yup-oauth2` + targeted fallback.

## Final choice (Sprint 5 freeze)

- Final choice: keep the primary generated-client stack (`google-gmail1`, `google-drive3`, `google-apis-common`,
  `yup-oauth2`) with native auth/account modules and command-local adapters.
- Final choice: keep `reqwest` as a documented fallback for blocked operations, but do not make it the default path.
- Final choice: keep `keyring` `=3.6.2` as the default secure token backend with deterministic file-mode test fallback.

## Rollback notes

- Rollback target: last wrapper-era release tag before native completion (`v1.1.9`) for emergency restore.
- Rollback method: revert `crates/google-cli` to wrapper-era runtime files, restore wrapper-era docs/specs, and remove
  native-only modules/reports in one revert change set.
- Rollback verification: run `cargo test -p google-cli`, `scripts/workflow-lint.sh`, and `scripts/workflow-test.sh`
  after the revert to confirm release readiness.

## Compile probe intent

`crates/google-cli/examples/native_probe.rs` is compile-only. It imports and type-checks the selected OAuth + Gmail +
Drive stack without runtime behavior.
