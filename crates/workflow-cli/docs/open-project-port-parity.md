# Open Project Port Parity Contract

## Target

- Reference workflow: `/Users/terry/Project/graysurf/alfred-open-project-in-vscode/src/info.plist`
- Port target in this repo:
  - `crates/alfred-core`
  - `crates/workflow-common`
  - `crates/workflow-cli`
  - `workflows/open-project/scripts/*`
  - `workflows/open-project/src/info.plist.template`
  - `workflows/open-project/workflow.toml`

## Required Parity

| Behavior | Contract | Target Files |
| --- | --- | --- |
| `PROJECT_DIRS` default/expansion | Default to `$HOME/Project,$HOME/.config`; support comma-separated roots; expand `$HOME` and `~`. | `crates/workflow-common/src/config.rs`, `workflows/open-project/workflow.toml` |
| `USAGE_FILE` default/expansion | Default to `$HOME/.config/zsh/cache/.alfred_project_usage.log`; support `$HOME` and `~`. | `crates/workflow-common/src/config.rs`, `crates/workflow-common/src/usage_log.rs`, `workflows/open-project/workflow.toml` |
| `VSCODE_PATH` default | Default to `/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code`; allow override by env var. | `workflows/open-project/workflow.toml`, `workflows/open-project/scripts/action_open.sh` |
| Project discovery depth | Scan each root up to depth 3 for Git repositories. Missing/unreadable roots are skipped without fatal error. | `crates/workflow-common/src/discovery.rs` |
| Query filter | Empty query returns all projects; non-empty query uses basename substring matching. | `crates/workflow-common/src/discovery.rs` |
| No-project fallback | Return Alfred JSON with one invalid item (`valid=false`) instead of process failure. | `crates/workflow-common/src/feedback.rs`, `crates/alfred-core/src/lib.rs` |
| Usage key resolution | Resolve usage timestamp by full path key first, then basename fallback for legacy entries. | `crates/workflow-common/src/usage_log.rs` |
| Usage sort order | Parse `%Y-%m-%d %H:%M:%S`, sort descending by timestamp; invalid/missing timestamp falls back predictably. | `crates/workflow-common/src/feedback.rs`, `crates/workflow-common/src/usage_log.rs` |
| Subtitle format | Emit `commit_text • last_used_text`; missing values render as `No recent commits` and `N/A`. | `crates/workflow-common/src/feedback.rs`, `crates/workflow-common/src/git.rs` |
| Alfred entrypoints | Support `c`, `code`, and `github` script-filter entrypoints in workflow object graph. | `workflows/open-project/src/info.plist.template` |
| Shift routing | Shift modifier route from project list opens GitHub action path. | `workflows/open-project/src/info.plist.template`, `crates/alfred-core/src/lib.rs` |
| GitHub remote behavior | Normalize `git@github.com:owner/repo(.git)` and `https://github.com/owner/repo(.git)` to canonical URL; unsupported/missing origin returns explicit error. | `crates/workflow-common/src/git.rs`, `crates/workflow-cli/src/main.rs`, `workflows/open-project/scripts/action_open_github.sh` |
| CLI command contract | `script-filter` prints Alfred JSON only; `record-usage` and `github-url` print plain output only. | `crates/workflow-cli/src/main.rs` |

## Optional Improvements (Not Required For Parity)

- Add richer Alfred fields (`mods`, `variables`) when useful, but avoid behavior changes for default Enter action.
- Add extra remote format support beyond GitHub canonical formats.
- Add extra workflow metadata fields not required by runtime behavior.

## Validation Checklist

- [x] `crates/workflow-cli/docs/open-project-port-parity.md` exists and maps parity rules to repository files.
- [x] `PROJECT_DIRS`, `USAGE_FILE`, `VSCODE_PATH` defaults documented and implemented.
- [x] Query filtering, subtitle formatting, and usage-based sort behavior covered by unit tests.
- [x] `c`, `code`, `github` and Shift route represented in `info.plist.template`.
- [x] `script-filter`, `record-usage`, `github-url` CLI contracts verified by tests.
- [x] Workflow smoke test verifies packaged plist graph and script presence.
