# Google CLI native validation report

## Automated native matrix

| Command | Status |
| --- | --- |
| `cargo test -p google-cli` | PASS |
| `cargo test -p google-cli --test native_no_gog` | PASS |
| `cargo test -p google-cli --lib` | PASS |
| `scripts/workflow-lint.sh` | PASS |
| `scripts/workflow-test.sh` | PASS |

## Live smoke checklist

### Live smoke: auth add + account restore

1. Run `cargo run -p google-cli -- auth add <email> --manual` and complete the loopback/manual flow.
2. Verify `cargo run -p google-cli -- auth status --json` shows the expected default account.
3. If temporary auth state was created, restore previous aliases/default account from saved metadata backup.

### Live smoke: gmail send + cleanup

1. Run `cargo run -p google-cli -- gmail send --to <target> --subject "<subject>" --body "<body>"`.
2. Verify `gmail search` and `gmail get` return the sent message and expected metadata.
3. Perform cleanup by deleting/archiving the smoke message and restoring mailbox state.

### Live smoke: drive upload/download + cleanup

1. Run `cargo run -p google-cli -- drive upload <local-path> --name <smoke-name>`.
2. Verify `drive search`, `drive get`, and `drive download` for the uploaded file id.
3. Perform cleanup by deleting the uploaded smoke file and verifying no stray Drive artifacts remain.

## Multi-account coverage notes

- Coverage includes no-default-account and default-present paths for both Gmail and Drive command families.
- Account ambiguity behavior is validated via `auth_account_resolution` and `account_resolution_shared` test suites.

## Release readiness note

- Native `google-cli` no longer requires wrapper runtime binaries.
- Keep this report updated when rerunning live smoke before release tagging.
