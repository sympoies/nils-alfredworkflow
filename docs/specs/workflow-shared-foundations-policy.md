# Workflow Shared Foundations Policy

## Scope

- This document is the canonical policy for shared foundation extraction across Alfred workflows in this repository.
- Shared foundation means cross-workflow runtime mechanics and smoke-test scaffolding that should behave consistently in every workflow.
- This policy governs extraction boundary decisions, migration constraints, and rollback constraints.

## Shared foundation extraction boundary

### Allowed shared foundation domains (normative)

- helper loading
- binary resolution
- error-row emission
- CLI invocation guard
- smoke scaffolding

### Domains that must stay local

- Domain mappings must stay local to each workflow.
- Provider-specific query semantics must stay local to each workflow.
- Workflow-specific ranking, wording, business rules, and provider error interpretation must stay local.

### Explicit over-sharing prohibition

- Do not move domain mappings into shared helpers.
- Do not move provider-specific query semantics into shared helpers.
- If a helper requires workflow/provider branching to function, keep that logic local and expose only a narrow shared primitive.

## Steam Search Adoption Contract

### Shared helper requirements (`steam-search`)

| Area | Requirement |
| --- | --- |
| helper loader | Must use `workflow_helper_loader.sh` and `wfhl_source_helper` in Script Filter and action wrappers. |
| search flow | Must use `script_filter_search_driver.sh` (`sfsd_run_search_flow`) for async/coalesce/cache mechanics. |
| query policy | Must use `script_filter_query_policy.sh` for Script Filter input normalization and short-query guardrails. |
| action requery | Must use `workflow_action_requery.sh` for requery payload parsing, state persistence, and Alfred requery trigger fallback. |
| action open | Must use `workflow_action_open_url.sh` for normal URL-open execution path. |

### Steam domain mechanics that must stay local

- Steam `storesearch` and `appdetails` endpoint selection/parameter mapping.
- Steam region and language semantics (for example `cc` behavior) and user-facing domain copy.
- Steam-specific ranking, subtitle wording, and provider error interpretation.

## Migration constraints

1. Extract only the allowed shared foundation domains listed above; do not broaden scope during migration.
2. Preserve workflow-local behavior contracts while migrating runtime mechanics.
3. Keep workflow adapters thin and local so workflow-specific semantics remain local after extraction.
4. Every migration PR must include targeted validation for affected workflows (lint/test/smoke as applicable).
5. If a migration cannot meet this extraction boundary, stop and keep the logic local.

## Rollback constraints

1. Roll back shared foundation migrations as a coherent unit for affected workflows and shared helper changes.
2. Do not use rollback to introduce shared domain mappings or shared provider-specific query semantics.
3. After rollback, rerun repository workflow validation gates before republishing artifacts.

## References

- Global workflow development guide: `ALFRED_WORKFLOW_DEVELOPMENT.md`
