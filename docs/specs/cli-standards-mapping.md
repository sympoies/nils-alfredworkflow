# CLI Standards Mapping (Repo Policy)

## Scope

- Applies to all CLI crates under `crates/*-cli`.
- Maps external standards into local migration rules for this repository.
- Policy baseline references:
  - `new-cli-crate-development-standard.md`
  - `cli-service-json-contract-guideline-v1.md`

## Normative Decisions

| Topic | Local policy decision | Migration note |
| --- | --- | --- |
| Default output | Default output should be `human-readable` for direct terminal usage. | Existing Alfred integrations may keep explicit legacy mode during migration. |
| Machine output | Service-oriented output must be opt-in via `--json`. | JSON mode uses one shared envelope (`schema_version`, `command`, `ok`, payload). |
| Alfred compatibility | Alfred consumers must use explicit compatibility mode (`--alfred-json` or equivalent crate-specific mode flag). | No workflow should rely on implicit default JSON once migrated. |
| Envelope shape | Required top-level keys in JSON mode: `schema_version`, `command`, `ok`, and exactly one of `result`/`results`/`error`. | Legacy top-level `items`-only payloads are transitional and compatibility-only. |
| Error contract | Failure payload must include `error.code`, `error.message`, optional `error.details`. | Runtime stderr remains human-oriented; machine clients must consume JSON envelope. |
| Exit code semantics | Keep current repo behavior: `0=success`, `1=runtime/dependency`, `2=user/input/config`. | Revisit only with explicit RFC and multi-crate rollout plan. |
| Secret safety | Never include secrets/tokens in `result`, `error.message`, or `error.details`. | Contract tests must include secret-redaction assertions for JSON paths. |

## Native google-cli note

- `google-cli` is a native Rust crate with scoped support for `auth/gmail/drive`.
- Native contract owner: `docs/specs/google-cli-native-contract.md`.
- Local policy for this crate:
  - default output remains human-readable native text
  - `--plain` emits stable native text unchanged
  - `--json` emits the repo envelope around native payloads
  - native-owned errors must use stable `NILS_GOOGLE_*` codes (`NILS_GOOGLE_002`-`004` remain reserved)

## Legacy Exceptions (Time-bounded)

| Exception | Allowed until | Owner | Sunset action |
| --- | --- | --- | --- |
| Legacy Alfred JSON shape (top-level `items`) for existing workflow `script_filter` callers. | 2026-09-30 | Workflow maintainers (`crates/*-cli` + `workflows/*`) | Migrate all script calls to explicit compatibility flags and remove implicit JSON defaults. |
| Mixed output commands in `workflow-cli` (`script-filter` JSON + action commands plain text). | 2026-12-31 | `workflow-cli` maintainers | Keep mixed behavior documented; add explicit JSON envelope mode for service consumption only. |

Any exception extension after its date requires a dedicated PR updating this document and the migration plan.

## Ownership And Change Control

- Policy owner: repository maintainers responsible for CLI crates and workflow scripts.
- Required change set for policy updates:
  - Update this file.
  - Update `docs/reports/cli-command-inventory.md` if command/consumer mapping changes.
  - Update `docs/specs/cli-json-envelope-v1.md` and/or `docs/specs/cli-error-code-registry.md` if contract changes.
  - Keep `scripts/cli-standards-audit.sh` checks aligned with policy changes.
- Review control:
  - At least one maintainer approval is required for any contract-affecting PR.
  - PR description must include backward-compatibility impact and rollback plan.

## Compliance Checklist

- Every CLI command has documented mode behavior (`human-readable`, `--json`, compatibility mode).
- Every service JSON response conforms to envelope v1.
- Every machine error has stable `error.code` from the registry.
- Every migration PR updates tests and docs together.
