# Codex CLI - Alfred Workflow

Run core `nils-codex-cli@0.3.2` operations from Alfred.

## Screenshot

![Codex CLI workflow screenshot](./screenshot.png)

## Configuration

| Variable             | Required | Default | Description                                                     |
| -------------------- | -------- | ------- | --------------------------------------------------------------- |
| `CODEX_CLI_BIN`      | No       | empty   | Optional absolute path override for `codex-cli`.                |
| `CODEX_SAVE_CONFIRM` | No       | `1`     | Require confirmation for `save` without `--yes` (`0` disables). |

## Keywords

| Keyword | Behavior                                                  |
| ------- | --------------------------------------------------------- |
| `cx`    | Command palette for auth/save/diag actions.               |
| `cxda`  | Alias of `cx diag all-json ...` (all-accounts JSON view). |
