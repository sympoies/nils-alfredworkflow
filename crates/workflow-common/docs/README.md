# nils-workflow-common docs

Crate-local documentation index for `nils-workflow-common`.

## Intended Readers

- Maintainers integrating shared open-project logic across CLI crates.
- Contributors changing output contract, discovery behavior, or shared runtime configuration.

## Canonical Documents

- `../README.md`: crate purpose, public API summary, and validation commands.
- [`preference-projection-contract.md`](preference-projection-contract.md): external preference projection
  file format, validation, freshness, precedence, status-row wording, and privacy rules.

## Why no `workflow-contract.md`

`nils-workflow-common` is a library-only crate — it has no binary, no clap subcommand surface, and no JSON
service envelope of its own. There is no per-CLI contract to freeze; the public API documented in
[`../README.md`](../README.md) is the canonical surface and is exercised through `cargo test -p
nils-workflow-common`. A dedicated `workflow-contract.md` would only restate the README without adding
information.

## Notable behavior surfaces

- **Output contract helpers** (`OutputMode`, `select_output_mode`, `build_success_envelope`,
  `build_error_envelope`, `redact_sensitive`) — implement the shared runtime contract from
  [`docs/specs/cli-shared-runtime-contract.md`](../../../docs/specs/cli-shared-runtime-contract.md). Every
  CLI crate that emits the JSON envelope routes through these helpers; the canonical envelope schema
  version constant (`ENVELOPE_SCHEMA_VERSION`) lives in `src/output_contract.rs`.
- **Host-agnostic git remote helpers** (`web_url_for_project`, `normalize_remote`) — `github.com` is the
  single strict case (path must be exactly `<owner>/<repo>`); any other host accepts paths with two or more
  segments. This widening unblocks GitLab subgroups, Gitea organizations, Bitbucket workspaces, and
  self-hosted instances without per-host configuration. See `crates/workflow-cli/docs/workflow-contract.md`
  for the consumer-side `github-url` policy that surfaces this behavior.
- **Ordered list parser** (`split_ordered_list`, `parse_ordered_list_with`) — canonical comma/newline
  tokenizer used by workflows that accept config/query lists (e.g., timezone IDs, wiki language options).
  Tokenization rules are normative per `ALFRED_WORKFLOW_DEVELOPMENT.md` (`Ordered config list parsing
  standard`); domain validation stays local to each consumer crate.
