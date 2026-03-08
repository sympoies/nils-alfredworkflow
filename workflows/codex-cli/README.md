# Codex CLI - Alfred Workflow

Run core `nils-codex-cli@0.6.5` operations from Alfred.

## Screenshot

![Codex CLI workflow screenshot](./screenshot.png)

## Scope

This workflow currently supports:

- `auth login` (browser, `--api-key`, `--device-code`)
- `auth use <secret>` (supports direct query and picker list)
- `auth save [--yes] <secret.json>`
- `auth remove [--yes] <secret.json>`
- `auth current --json` quick inspection (`cxac`)
- `diag rate-limits` presets:
  - default
  - `--cached`
  - `--one-line`
  - `--all`
  - `--all --json` (parsed in Alfred)
  - `--all --async --jobs 4`

Input policy:

- All `cx*` Script Filters use a 1 second queue delay and disable immediate first-character execution.
- `cxac` and `diag` branches gate short partial queries (`<2` chars) with `Keep typing (2+ chars)` and skip expensive
  refresh/current calls.

Diag result behavior:

- `cxau` / `cxd` / `cxda` auto-refresh diag cache by TTL before rendering cache-based rows.
- If cache is missing/expired, list rendering blocks until refresh finishes (no stale cache render).
- Refresh TTL is controlled by `CODEX_DIAG_CACHE_TTL_SECONDS` (default `300` = 5 minutes).
- Wait timeout while another refresh is running is controlled by `CODEX_DIAG_CACHE_BLOCK_WAIT_SECONDS` (default `15`
  seconds).
- `cxd` default refresh/action uses `diag rate-limits --json` and parses single-account rows.
- `cxda result` parses JSON and renders one account per row.
- `cxda result` rows are sorted by `weekly_reset_epoch` ascending (earliest reset first).
- Parsed subtitle format: `<email> | reset <weekly_reset_local>`.

Auth use behavior:

- `cxau` first row shows current secret JSON from `codex-cli auth current` (when parsable).
- Following rows list all `*.json` files in `CODEX_SECRET_DIR` (or fallback config dir).
- When no saved `*.json` exists, `cxau` still shows current `auth.json` info (for example email).
- Press Enter on a row to run `codex-cli auth use <secret>`.

No `CODEX_SECRET_DIR` saved secrets behavior:

- `cxda` falls back from `diag rate-limits --all --json` to `diag rate-limits --json` (current auth).
- `cxd` / `cxda` menu still shows current auth hint row even before saved-secret setup.

## Runtime Requirements

- End users: no extra install when using release artifact.
- `.alfredworkflow` bundles `codex-cli@0.6.5` (release-coupled runtime version).
- Pinned runtime metadata is centralized in `scripts/lib/codex_cli_runtime.sh`.
- Bundled target: macOS `arm64`.

Fallback runtime sources (when bundled binary is unavailable):

1. `CODEX_CLI_BIN` (absolute path)
2. `PATH` lookup (`codex-cli`)

Manual fallback install:

```bash
cargo install nils-codex-cli --version 0.6.5
```

## Configuration

| Variable                              | Required | Default | Description                                                                                                                                                                   |
| ------------------------------------- | -------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CODEX_CLI_BIN`                       | No       | empty   | Optional absolute path override for `codex-cli`.                                                                                                                              |
| `CODEX_AUTH_FILE`                     | No       | empty   | Auth file path used by `codex-cli` auth/diag commands. Resolution order: configured value -> inherited env `CODEX_AUTH_FILE` -> `~/.codex/auth.json`. Supports `~` expansion. |
| `CODEX_SECRET_DIR`                    | No       | empty   | Optional secret directory override. If empty, runtime fallback is `$XDG_CONFIG_HOME/codex_secrets` or `~/.config/codex_secrets`.                                              |
| `CODEX_DIAG_CACHE_TTL_SECONDS`        | No       | `300`   | Diag cache TTL for `cxau`/`cxd`/`cxda` (`0` means always refresh before render).                                                                                              |
| `CODEX_DIAG_CACHE_BLOCK_WAIT_SECONDS` | No       | `15`    | Max wait seconds while another process is refreshing the same diag cache mode.                                                                                                |
| `CODEX_LOGIN_TIMEOUT_SECONDS`         | No       | `60`    | Login timeout in seconds (`1..3600`).                                                                                                                                         |
| `CODEX_API_KEY`                       | No       | empty   | API key source for `auth login --api-key` (otherwise prompt on macOS).                                                                                                        |
| `CODEX_SAVE_CONFIRM`                  | No       | `1`     | Require confirmation for `save` without `--yes` (`0` disables).                                                                                                               |
| `CODEX_REMOVE_CONFIRM`                | No       | `1`     | Require confirmation for `remove` without `--yes` (`0` disables).                                                                                                             |

## Keywords

| Keyword | Behavior                                                  |
| ------- | --------------------------------------------------------- |
| `cx`    | Command palette for auth/save/remove/diag actions.        |
| `cxa`   | Alias of `cx auth ...`.                                   |
| `cxau`  | Alias of `cx auth use ...` (current + all JSON picker).   |
| `cxac`  | Run `codex-cli auth current --json` and show raw result.  |
| `cxd`   | Alias of `cx diag ...`.                                   |
| `cxda`  | Alias of `cx diag all-json ...` (all-accounts JSON view). |
| `cxs`   | Alias of `cx save ...`.                                   |
| `cxr`   | Alias of `cx remove ...`.                                 |

## Query Examples

| Query                             | Result                                                                           |
| --------------------------------- | -------------------------------------------------------------------------------- |
| `cx login`                        | Run `codex-cli auth login`                                                       |
| `cx login --api-key`              | Run `codex-cli auth login --api-key`                                             |
| `cx login --device-code`          | Run `codex-cli auth login --device-code`                                         |
| `cx save team-alpha.json`         | Run `codex-cli auth save team-alpha.json` (with confirmation)                    |
| `cx save --yes team-alpha.json`   | Run `codex-cli auth save --yes team-alpha.json`                                  |
| `cxs --yes team-alpha.json`       | Alias of `cx save --yes team-alpha.json`                                         |
| `cx remove team-alpha.json`       | Show confirmation dialog, then run `codex-cli auth remove --yes team-alpha.json` |
| `cx remove --yes team-alpha.json` | Run `codex-cli auth remove --yes team-alpha.json`                                |
| `cxr team-alpha.json`             | Alias of `cx remove team-alpha.json`                                             |
| `cxr --yes team-alpha.json`       | Alias of `cx remove --yes team-alpha.json`                                       |
| `cx use alpha`                    | Run `codex-cli auth use alpha`                                                   |
| `cxau`                            | Show current JSON + all JSON secrets, then select to use                         |
| `cxau alpha`                      | Run `codex-cli auth use alpha` directly                                          |
| `cxac`                            | Show `codex-cli auth current --json` parsed/raw output                           |
| `cx diag`                         | Run `codex-cli diag rate-limits --json`                                          |
| `cx diag cached`                  | Run `codex-cli diag rate-limits --cached`                                        |
| `cx diag one-line`                | Run `codex-cli diag rate-limits --one-line`                                      |
| `cx diag all`                     | Run `codex-cli diag rate-limits --all`                                           |
| `cx diag async`                   | Run `codex-cli diag rate-limits --all --async --jobs 4`                          |
| `cxd`                             | Run `codex-cli diag rate-limits --json`                                          |
| `cxda`                            | Run `codex-cli diag rate-limits --all --json`                                    |
| `cxd result`                      | Show latest cached default JSON-parsed result rows                               |
| `cxda result`                     | Show latest cached all-json parsed rows                                          |
| `cxda result raw`                 | Same as above with higher row limit                                              |

## Maintainer Packaging Notes

- Official package should bundle exactly `codex-cli@0.6.5`.
- `scripts/workflow-pack.sh --id codex-cli` runs `workflows/codex-cli/scripts/prepare_package.sh`.
- Packaging binary resolution order:
  1. `CODEX_CLI_PACK_BIN` (if set)
  2. local `PATH` `codex-cli`
  3. auto-install pinned `nils-codex-cli@0.6.5` from crates.io via `cargo install --locked --root <cache-root>`
- Useful overrides:
  - `CODEX_CLI_PACK_BIN=/absolute/path/to/codex-cli`
  - `CODEX_CLI_PACK_INSTALL_ROOT=/absolute/path/to/install-root` (default is cache under `$XDG_CACHE_HOME` or
    `~/.cache`)

## Validation

Run before packaging/release:

- `bash workflows/codex-cli/tests/smoke.sh`
- `scripts/workflow-test.sh --id codex-cli`
- `scripts/workflow-pack.sh --id codex-cli`

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).
