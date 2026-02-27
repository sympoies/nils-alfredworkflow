# YouTube Search - Alfred Workflow

Search YouTube videos from Alfred and open selected videos in your browser.

## Screenshot

![YouTube Search workflow screenshot](./screenshot.png)

## Features

- Trigger YouTube search with `yt <query>`.
- Show video title and description in Alfred results.
- Open selected YouTube watch URL in your default browser with `Enter`.
- Short query guard: `<2` characters shows `Keep typing (2+ chars)` and skips API calls.
- Script Filter queue policy: 1 second delay with initial immediate run disabled.
- Script-level guardrails: async query coalescing (final query priority) and short TTL cache reduce duplicate API calls
  while typing.
- Runtime orchestration is shared via `scripts/lib/script_filter_search_driver.sh`; YouTube-specific fetch/error mapping
  remains local.
- Map common failures (missing API key, quota exceeded, API unavailable, invalid config) to actionable Alfred messages.
- Tune result count and region targeting through workflow variables.

## Configuration

Set these via Alfred's "Configure Workflow..." UI:

| Variable              | Required | Default | Description                                                             |
| --------------------- | -------- | ------- | ----------------------------------------------------------------------- |
| `YOUTUBE_API_KEY`     | Yes      | (empty) | YouTube Data API v3 key.                                                |
| `YOUTUBE_MAX_RESULTS` | No       | `10`    | Max results per query. Effective range is clamped to `1..25`.           |
| `YOUTUBE_REGION_CODE` | No       | (empty) | Optional ISO 3166-1 alpha-2 region code (for example `US`, `TW`, `JP`). |

## Keyword

| Keyword      | Behavior                                                |
| ------------ | ------------------------------------------------------- |
| `yt <query>` | Search and list videos, then open selected YouTube URL. |

## Advanced Runtime Parameters

| Parameter                               | Description                                                                                     |
| --------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `YOUTUBE_CLI_BIN`                       | Optional override path for `youtube-cli` (useful for local debugging).                          |
| `YOUTUBE_QUERY_CACHE_TTL_SECONDS`       | Optional same-query cache TTL (seconds). Default `0` (disabled to avoid stale mid-typing hits). |
| `YOUTUBE_QUERY_COALESCE_SETTLE_SECONDS` | Optional coalesce settle window (seconds). Default `2`.                                         |
| `YOUTUBE_QUERY_COALESCE_RERUN_SECONDS`  | Optional Alfred rerun interval while waiting for coalesced result. Default `0.4`.               |

## macOS Gatekeeper acceptance (optional manual)

For one-time quarantine cleanup and smoke validation after install:

```bash
WORKFLOW_DIR="$(for p in "$HOME"/Library/Application\ Support/Alfred/Alfred.alfredpreferences/workflows/*/info.plist; do
  [ -f "$p" ] || continue
  bid="$(plutil -extract bundleid raw -o - "$p" 2>/dev/null || true)"
  [ "$bid" = "com.graysurf.youtube-search" ] && dirname "$p"
done | head -n1)"

[ -n "$WORKFLOW_DIR" ] || { echo "youtube-search workflow not found"; exit 1; }
xattr -dr com.apple.quarantine "$WORKFLOW_DIR"
"$WORKFLOW_DIR/scripts/script_filter.sh" "rust tutorial" | jq -e '.items | type == "array"'
```

## Troubleshooting

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).
