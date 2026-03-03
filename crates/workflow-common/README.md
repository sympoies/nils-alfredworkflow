# nils-workflow-common

Shared open-project domain and output-contract utilities reused by workflow crates.

## Public API Summary

- Configuration: `RuntimeConfig` plus defaults/helpers (`parse_project_dirs`, `expand_home_tokens`).
- Ordered list parsing: `split_ordered_list` and `parse_ordered_list_with` for deterministic comma/newline config lists.
- Project discovery: `Project`, `discover_projects`, `filter_projects`.
- Alfred feedback assembly: `build_feedback`, `build_script_filter_feedback`, and `Feedback` re-export.
- Git + errors: `github_url_for_project`, `normalize_github_remote`, `WorkflowError`.
- Output contract: `OutputMode`, `select_output_mode`, envelope builders, and `redact_sensitive`.
- Usage log: `record_usage` and `parse_usage_timestamp`.

## Contract References

- Shared runtime contract: [`docs/specs/cli-shared-runtime-contract.md`](../../docs/specs/cli-shared-runtime-contract.md)
- Compatibility debt matrix: [`docs/reports/crate-legacy-removal-matrix.md`](../../docs/reports/crate-legacy-removal-matrix.md)

## Documentation

- `docs/README.md`

## Validation

- `cargo check -p nils-workflow-common`
- `cargo test -p nils-workflow-common`
