# CLI Standards Mapping (Repo Policy)

## Scope

- Applies to all CLI crates under `crates/*-cli`.
- Maps external standards into local migration rules for this repository.
- Policy baseline references:
  - `new-cli-crate-development-standard.md`
  - `cli-service-json-contract-guideline-v1.md`
  - `docs/specs/cli-shared-runtime-contract.md` (shared runtime contract)
  - `docs/specs/google-cli-native-contract.md` (native Google CLI contract)
  - `docs/reports/crate-legacy-removal-matrix.md` (compatibility debt tracking)

## Normative Decisions

| Topic | Local policy decision | Migration note |
| --- | --- | --- |
| Shared runtime contract | The shared runtime contract in `docs/specs/cli-shared-runtime-contract.md` is the canonical implementation contract for output mode behavior and envelope/error semantics. | Contract-affecting changes must update both this file and the shared runtime contract. |
| Output mode selector | Canonical output mode selector is `--output <human\|json\|alfred-json>`. | Documentation and scripts must prefer explicit `--output` mode selection. |
| Default output | Default output should be `human-readable` for direct terminal usage. | Script-filter callers should pass explicit `--output alfred-json` when Alfred JSON is required. |
| Machine output | Service-oriented output must be opt-in via explicit JSON output mode. | JSON mode uses one shared envelope (`schema_version`, `command`, `ok`, payload). |
| Alfred compatibility | Alfred consumers must use explicit `--output alfred-json` mode. | Avoid implicit/alias output-mode switching in new docs and scripts. |
| Envelope shape | Required top-level keys in JSON mode: `schema_version`, `command`, `ok`, and exactly one of `result`/`results`/`error`. | New commands must not introduce top-level `items`-only payloads. |
| Error contract | Failure payload must include `error.code`, `error.message`, optional `error.details`. | Runtime stderr remains human-oriented; machine clients must consume JSON envelope. |
| Exit code semantics | Keep current repo behavior: `0=success`, `1=runtime/dependency`, `2=user/input/config`. | Revisit only with explicit RFC and multi-crate rollout plan. |
| Secret safety | Never include secrets/tokens in `result`, `error.message`, or `error.details`. | Contract tests must include secret-redaction assertions for JSON paths. |
| Forbidden output aliases | New code must not add output aliases such as `text`, `alfred`, `alfred_json`, or `--mode service-json` forms. | Existing non-canonical aliases remain tracked in `docs/reports/crate-legacy-removal-matrix.md` until removed. |

## Native google-cli note

- `google-cli` is a native Rust crate with package `nils-google-cli` and scoped support for `auth/gmail/drive`.
- Native contract owner: `docs/specs/google-cli-native-contract.md`.
- Native validation evidence: `docs/reports/google-cli-native-validation-report.md`.
- Local policy for this crate:
  - default output remains human-readable native text
  - `--plain` emits stable native text unchanged
  - `--json` emits the repo envelope around native payloads
  - native-owned errors must use stable `NILS_GOOGLE_*` codes (`NILS_GOOGLE_002`-`004` remain reserved)

## Compatibility Debt Tracking

- Any temporary or historical output-mode exception must be tracked in
  `docs/reports/crate-legacy-removal-matrix.md`.
- New docs and scripts must use canonical output flags and envelope rules from this document and
  `docs/specs/cli-shared-runtime-contract.md`.

## Ownership And Change Control

- Policy owner: repository maintainers responsible for CLI crates and workflow scripts.
- Required change set for policy updates:
  - Update this file.
  - Update `docs/specs/cli-shared-runtime-contract.md`.
  - Update `docs/reports/crate-legacy-removal-matrix.md`.
  - Update `docs/reports/cli-command-inventory.md` if command/consumer mapping changes.
  - Update `docs/specs/cli-json-envelope-v1.md` and/or `docs/specs/cli-error-code-registry.md` if contract changes.
  - Keep `scripts/cli-standards-audit.sh` checks aligned with policy changes.
- Review control:
  - At least one maintainer approval is required for any contract-affecting PR.
  - PR description must include backward-compatibility impact and rollback plan.

## Compliance Checklist

- Every CLI command has documented output mode behavior (`human-readable`, `json`, `alfred-json`, compatibility mode where needed).
- Every service JSON response conforms to envelope v1.
- Every machine error has stable `error.code` from the registry.
- Every temporary compatibility path is tracked in the legacy removal matrix with owner + removal task.
- Every migration PR updates tests and docs together.
