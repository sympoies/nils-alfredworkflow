# Docs Ownership Matrix (Sprint 4)

## Classification legend

- `canonical`: source of truth for current behavior and operator policy.
- `report`: point-in-time evidence; keep the latest release-relevant run.
- `stale candidate`: superseded document; remove and replace with canonical references.

## Active ownership matrix

| Document | Classification | Owner | Retention | Notes |
| --- | --- | --- | --- | --- |
| `README.md` | canonical | Repository maintainers | Keep indefinitely; update when workflow catalog or global navigation changes. | Entry point for workflow catalog and top-level routing. |
| `docs/ARCHITECTURE.md` | canonical | Repository maintainers | Keep indefinitely; update when runtime/layout boundaries change. | Architecture baseline and crate/workflow boundaries. |
| `docs/RELEASE.md` | canonical | Repository maintainers | Keep indefinitely; update when release gates or packaging flow changes. | Release/tagging source of truth. |
| `docs/specs/cli-standards-mapping.md` | canonical | CLI maintainers | Keep indefinitely; update with contract-affecting CLI changes. | CLI output/error policy mapping. |
| `DEVELOPMENT.md` | canonical | Repository maintainers | Keep indefinitely; update with local/CI gate changes. | Build/lint/test/pack contributor gate. |
| `ALFRED_WORKFLOW_DEVELOPMENT.md` | canonical | Workflow maintainers | Keep indefinitely; update with cross-workflow runtime policy changes. | Cross-workflow Alfred standards and troubleshooting routing. |
| `crates/workflow-common/README.md` | canonical | `workflow-common` maintainers | Keep indefinitely; update with exported API/contract changes. | Shared runtime crate contract surface. |
| `crates/workflow-cli/README.md` | canonical | `workflow-cli` maintainers | Keep indefinitely; update with command/output behavior changes. | Open-project workflow CLI command contract. |
| `crates/weather-cli/README.md` | canonical | `weather-cli` maintainers | Keep indefinitely; update with command/provider/output behavior changes. | Weather CLI contract and validation entrypoints. |
| `workflows/google-search/README.md` | canonical | `google-search` workflow maintainers | Keep indefinitely; update with keyword/runtime behavior changes. | Workflow runtime/query/config behavior. |
| `workflows/google-service/README.md` | canonical | `google-service` workflow maintainers | Keep indefinitely; update with auth/drive/gmail runtime behavior changes. | Active account semantics and command UX contract. |
| `workflows/steam-search/README.md` | canonical | `steam-search` workflow maintainers | Keep indefinitely; update with keyword/runtime behavior changes. | Region/query behavior and runtime parameters. |
| `workflows/memo-add/README.md` | canonical | `memo-add` workflow maintainers | Keep indefinitely; update with query/action contract changes. | Memo action/query contract and operator validation. |
| `docs/reports/google-cli-native-validation-report.md` | report | `google-cli` maintainers | Keep latest native validation report; refresh before release when native Google CLI surface changes. | Canonical validation evidence for native Google CLI runtime. |

## Stale candidate decisions (S4T2)

| Candidate file (removed) | Classification | Owner | Retention decision | Rationale | Canonical replacement |
| --- | --- | --- | --- | --- | --- |
| `google-service-auth-workflow-plan.md` | stale candidate | `google-service` maintainers | Delete from repo docs tree. | Planning artifact duplicated implementation scope now captured in workflow runtime docs and issue lane history. | `workflows/google-service/README.md` plus issue/plan snapshots under agent runtime output. |
| `google-cli-validation-report.md` | stale candidate | `google-cli` maintainers | Delete from repo docs tree. | Wrapper-era validation evidence is superseded by native implementation coverage. | `docs/reports/google-cli-native-validation-report.md` and `crates/google-cli/README.md`. |
| `steam-search-validation-report.md` | stale candidate | `steam-search` maintainers | Delete from repo docs tree. | One-off sprint report duplicated validation commands and runtime notes now kept in workflow README. | `workflows/steam-search/README.md` validation section. |
| `google-cli-wrapper-contract.md` | stale candidate | `google-cli` maintainers | Delete from repo docs tree. | Wrapper boundary/spec text was replaced by native Rust contract documents. | `docs/specs/google-cli-native-contract.md` and `docs/specs/cli-standards-mapping.md`. |

## Duplicate coverage resolved

- Google CLI contract duplication resolved by keeping only native contract/spec references.
- Google/Steam validation report duplication resolved by keeping canonical validation entrypoints in active README/report docs.
- Workflow planning duplication resolved by using workflow README + issue/plan runtime artifacts instead of frozen plan files in `docs/`.
