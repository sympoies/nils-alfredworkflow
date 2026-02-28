# Google CLI native gap analysis

## Goal

Capture the wrapper-era baseline for `google-cli` and convert it into a native implementation inventory for
`auth`, `gmail`, and `drive`.

## Repo-scoped command inventory to preserve

### Auth

- `auth credentials set`
- `auth credentials list`
- `auth add`
- `auth list`
- `auth status`
- `auth remove`
- `auth alias`
- `auth manage`

### Gmail

- `gmail search`
- `gmail get`
- `gmail send`
- `gmail thread get`
- `gmail thread modify`

### Drive

- `drive ls`
- `drive search`
- `drive get`
- `drive download`
- `drive upload`

## Live E2E findings from wrapper baseline

- `auth add --remote --step 2` can fail with a state mismatch when the callback state emitted by step 1 does not match
  what step 2 expects.
- `auth status` is ambiguous when `--account` is omitted and multiple accounts exist. The current behavior can return an
  empty account payload instead of a deterministic resolution/error path.
- `auth manage` currently depends on an upstream browser account-manager UI that is outside this repository's control.
- `gmail` and `drive` commands are currently pass-through wrappers and depend on upstream `gog` CLI behavior.

## Native migration gaps

| Area | Current state | Native requirement |
| --- | --- | --- |
| Auth flow state | Wrapper relays remote/manual flow behavior from `gog`. | Own state lifecycle in Rust to remove the state mismatch class of bugs. |
| Default account resolution | `auth status` can be ambiguous without explicit account input. | Resolve default account deterministically or fail with explicit guidance. |
| Account management UX | `auth manage` relies on browser page owned by upstream tooling. | Provide terminal-native behavior with no browser manager page. |
| Gmail/Drive behavior | Wrapper forwards command-specific flags directly to `gog`. | Keep command scope but execute through native Rust clients. |

## Sprint 2 auth closure notes

- Native auth now owns credentials, token persistence, alias/default metadata, and account-resolution semantics.
- Loopback mode is the primary interactive path; `manual` and two-step `remote` exchange are also supported.
- `auth status` now resolves default account deterministically or returns explicit ambiguity guidance.
- `auth manage` remains summary-only by design; browser account-manager UI is still a non-goal.
- `gog` is no longer a runtime dependency for `auth` commands (still required for wrapper-backed `gmail`/`drive`).

## Manual auth smoke checklist

1. `google-cli auth credentials set --client-id <id> --client-secret <secret>`
2. `google-cli auth add <email>` (loopback)
3. `google-cli auth list` and `google-cli auth status`
4. `google-cli auth manage` (summary-only)

## Explicit non-goals for this migration

- non-goal: rebuilding the browser account-manager page.
- non-goal: expanding beyond repo-scoped Google surfaces (`auth`, `gmail`, `drive`).
- non-goal: adding unrelated Google domains such as `calendar`, `chat`, `docs`, `forms`, or `people`.

## Source references

- Wrapper baseline report: `docs/reports/google-cli-validation-report.md`
- Existing crate command docs: `crates/google-cli/README.md`
