# CLI Error Code Registry (v1)

> Status: active

## Purpose

Provides stable machine error codes shared by all CLI crates using JSON envelope v1.

## Code Format

- Format: `NILS_<DOMAIN>_<NNN>`
- Example: `NILS_WEATHER_002`
- Stability rules:
  - Codes are append-only after release.
  - Existing meanings cannot be repurposed.
  - Deprecated codes remain reserved and documented.

## Shared Codes

| Code | Meaning | Typical exit code |
| --- | --- | --- |
| `NILS_COMMON_001` | invalid user input | 2 |
| `NILS_COMMON_002` | missing required configuration | 2 |
| `NILS_COMMON_003` | upstream service unavailable | 1 |
| `NILS_COMMON_004` | invalid upstream response | 1 |
| `NILS_COMMON_005` | internal serialization/runtime failure | 1 |

## Domain Allocation (Unique Ranges)

| Domain / crate | Prefix | Reserved range |
| --- | --- | --- |
| `bangumi-cli` (`nils-bangumi-cli`) | `NILS_BANGUMI_` | `001-099` |
| `bilibili-cli` (`nils-bilibili-cli`) | `NILS_BILIBILI_` | `001-099` |
| `brave-cli` (`nils-brave-cli`) | `NILS_BRAVE_` | `001-099` |
| `cambridge-cli` (`nils-cambridge-cli`) | `NILS_CAMBRIDGE_` | `001-099` |
| `epoch-cli` (`nils-epoch-cli`) | `NILS_EPOCH_` | `001-099` |
| `google-cli` (`nils-google-cli`) | `NILS_GOOGLE_` | `001-099` |
| `market-cli` (`nils-market-cli`) | `NILS_MARKET_` | `001-099` |
| `memo-workflow-cli` (`nils-memo-workflow-cli`) | `NILS_MEMO_` | `001-099` |
| `quote-cli` (`nils-quote-cli`) | `NILS_QUOTE_` | `001-099` |
| `randomer-cli` (`nils-randomer-cli`) | `NILS_RANDOMER_` | `001-099` |
| `spotify-cli` (`nils-spotify-cli`) | `NILS_SPOTIFY_` | `001-099` |
| `steam-cli` (`nils-steam-cli`) | `NILS_STEAM_` | `001-099` |
| `timezone-cli` (`nils-timezone-cli`) | `NILS_TIMEZONE_` | `001-099` |
| `weather-cli` (`nils-weather-cli`) | `NILS_WEATHER_` | `001-099` |
| `wiki-cli` (`nils-wiki-cli`) | `NILS_WIKI_` | `001-099` |
| `workflow-cli` (`nils-workflow-cli`) | `NILS_WORKFLOW_` | `001-099` |
| `workflow-readme-cli` (`nils-workflow-readme-cli`) | `NILS_WORKFLOW_README_` | `001-099` |
| `youtube-cli` (`nils-youtube-cli`) | `NILS_YOUTUBE_` | `001-099` |

## Seed Registry (Initial Assignments)

Per-crate `_001` is the canonical user-input error and `_002` is the canonical
runtime/upstream failure. Higher numbers are reserved for finer-grained errors
that promote out of these generic buckets without breaking existing consumers.

| Code | Domain | Meaning |
| --- | --- | --- |
| `NILS_BANGUMI_001` | bangumi | invalid user input (empty query, malformed type token) |
| `NILS_BANGUMI_002` | bangumi | Bangumi API runtime failure (HTTP, transport, invalid response) |
| `NILS_BILIBILI_001` | bilibili | invalid user input (empty query) |
| `NILS_BILIBILI_002` | bilibili | Bilibili API runtime failure (HTTP, transport, invalid response) |
| `NILS_BRAVE_001` | brave | invalid user input (empty query, missing API key) |
| `NILS_BRAVE_002` | brave | Brave / suggestion API runtime failure |
| `NILS_CAMBRIDGE_001` | cambridge | invalid user input (token/stage parsing) |
| `NILS_CAMBRIDGE_002` | cambridge | scraper runtime failure (timeout, process error) |
| `NILS_EPOCH_001` | epoch | invalid user input (unsupported format, out-of-range value) |
| `NILS_EPOCH_002` | epoch | epoch conversion runtime failure |
| `NILS_GOOGLE_001` | google | invalid google-cli input / conflicting output flags |
| `NILS_GOOGLE_002` | google | reserved after native migration (legacy external-runtime missing binary) |
| `NILS_GOOGLE_003` | google | reserved after native migration (legacy external-runtime process failure) |
| `NILS_GOOGLE_004` | google | reserved after native migration (legacy external-runtime invalid JSON) |
| `NILS_GOOGLE_005` | google | auth invalid input |
| `NILS_GOOGLE_006` | google | auth ambiguous account selection |
| `NILS_GOOGLE_007` | google | auth store/runtime persistence failure |
| `NILS_GOOGLE_008` | google | auth remote/manual state mismatch |
| `NILS_GOOGLE_009` | google | Gmail invalid input |
| `NILS_GOOGLE_010` | google | Gmail resource not found |
| `NILS_GOOGLE_011` | google | Gmail runtime failure |
| `NILS_GOOGLE_012` | google | Drive invalid input |
| `NILS_GOOGLE_013` | google | Drive resource not found |
| `NILS_GOOGLE_014` | google | Drive runtime failure |
| `NILS_GOOGLE_015` | google | Calendar invalid input |
| `NILS_GOOGLE_016` | google | Calendar resource not found |
| `NILS_GOOGLE_017` | google | Calendar runtime failure |
| `NILS_MARKET_001` | market | invalid symbol/amount expression |
| `NILS_MARKET_002` | market | provider unavailable/rate-limited |
| `NILS_MEMO_001` | memo-workflow | invalid user input (parse/validation, missing config) |
| `NILS_MEMO_002` | memo-workflow | runtime/upstream failure (sqlite, serialization, IO) |
| `NILS_QUOTE_001` | quote | invalid user input / quote config value |
| `NILS_QUOTE_002` | quote | quote refresh/storage runtime failure |
| `NILS_RANDOMER_001` | randomer | invalid user input (unknown format, invalid count) |
| `NILS_RANDOMER_002` | randomer | randomer runtime failure |
| `NILS_SPOTIFY_001` | spotify | invalid user input (empty query, missing credentials) |
| `NILS_SPOTIFY_002` | spotify | Spotify API runtime failure (HTTP, rate-limit, transport) |
| `NILS_STEAM_001` | steam | invalid user input (empty query) |
| `NILS_STEAM_002` | steam | Steam storefront runtime failure (HTTP, transport, invalid response) |
| `NILS_TIMEZONE_001` | timezone | invalid timezone identifier / user input |
| `NILS_TIMEZONE_002` | timezone | timezone conversion runtime failure |
| `NILS_WEATHER_001` | weather | invalid location arguments |
| `NILS_WEATHER_002` | weather | weather provider unavailable |
| `NILS_WEATHER_003` | weather | geocoding failure |
| `NILS_WIKI_001` | wiki | invalid user input (empty query, invalid config) |
| `NILS_WIKI_002` | wiki | Wikipedia API runtime failure |
| `NILS_WORKFLOW_001` | workflow | project path not found/not directory |
| `NILS_WORKFLOW_002` | workflow | git origin/command failure |
| `NILS_WORKFLOW_003` | workflow | usage log persistence failure |
| `NILS_WORKFLOW_README_001` | workflow-readme | invalid Alfred workflow root path |
| `NILS_WORKFLOW_README_002` | workflow-readme | invalid README source path |
| `NILS_WORKFLOW_README_003` | workflow-readme | README source file not found |
| `NILS_WORKFLOW_README_004` | workflow-readme | workflow info.plist not found |
| `NILS_WORKFLOW_README_005` | workflow-readme | remote image URL not permitted |
| `NILS_WORKFLOW_README_006` | workflow-readme | invalid local image path or unsupported extension |
| `NILS_WORKFLOW_README_007` | workflow-readme | image asset file not found |
| `NILS_WORKFLOW_README_008` | workflow-readme | info.plist missing readme key |
| `NILS_WORKFLOW_README_009` | workflow-readme | filesystem read failure |
| `NILS_WORKFLOW_README_010` | workflow-readme | filesystem write failure |
| `NILS_WORKFLOW_README_011` | workflow-readme | directory creation failure |
| `NILS_WORKFLOW_README_012` | workflow-readme | file copy failure |
| `NILS_YOUTUBE_001` | youtube | invalid user input (empty query, missing API key) |
| `NILS_YOUTUBE_002` | youtube | YouTube API runtime failure (HTTP, quota, transport) |

## Change Control

- New code allocation requires:
  - registry update in this file,
  - contract test update in the affected crate,
  - migration note in PR summary.
- Removing legacy JSON shapes does not remove registered codes; it only changes call paths.
