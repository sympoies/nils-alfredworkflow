# CLI Error Code Registry (v1)

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
| `brave-cli` (`nils-brave-cli`) | `NILS_BRAVE_` | `001-099` |
| `cambridge-cli` (`nils-cambridge-cli`) | `NILS_CAMBRIDGE_` | `001-099` |
| `epoch-cli` (`nils-epoch-cli`) | `NILS_EPOCH_` | `001-099` |
| `google-cli` (`google-cli`) | `NILS_GOOGLE_` | `001-099` |
| `market-cli` (`nils-market-cli`) | `NILS_MARKET_` | `001-099` |
| `quote-cli` (`nils-quote-cli`) | `NILS_QUOTE_` | `001-099` |
| `randomer-cli` (`nils-randomer-cli`) | `NILS_RANDOMER_` | `001-099` |
| `spotify-cli` (`nils-spotify-cli`) | `NILS_SPOTIFY_` | `001-099` |
| `timezone-cli` (`nils-timezone-cli`) | `NILS_TIMEZONE_` | `001-099` |
| `weather-cli` (`nils-weather-cli`) | `NILS_WEATHER_` | `001-099` |
| `wiki-cli` (`nils-wiki-cli`) | `NILS_WIKI_` | `001-099` |
| `workflow-cli` (`nils-workflow-cli`) | `NILS_WORKFLOW_` | `001-099` |
| `youtube-cli` (`nils-youtube-cli`) | `NILS_YOUTUBE_` | `001-099` |

## Seed Registry (Initial Assignments)

| Code | Domain | Meaning |
| --- | --- | --- |
| `NILS_BRAVE_001` | brave | query empty |
| `NILS_BRAVE_002` | brave | missing `BRAVE_API_KEY` |
| `NILS_BRAVE_003` | brave | Brave API request failed |
| `NILS_CAMBRIDGE_001` | cambridge | invalid query token/stage |
| `NILS_CAMBRIDGE_002` | cambridge | scraper timeout |
| `NILS_CAMBRIDGE_003` | cambridge | scraper runtime process failure |
| `NILS_EPOCH_001` | epoch | unsupported query format |
| `NILS_EPOCH_002` | epoch | out-of-range datetime/epoch |
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
| `NILS_MARKET_001` | market | invalid symbol/amount expression |
| `NILS_MARKET_002` | market | provider unavailable/rate-limited |
| `NILS_QUOTE_001` | quote | invalid quote config value |
| `NILS_QUOTE_002` | quote | quote refresh/storage runtime failure |
| `NILS_RANDOMER_001` | randomer | unknown format |
| `NILS_RANDOMER_002` | randomer | invalid count |
| `NILS_SPOTIFY_001` | spotify | query empty |
| `NILS_SPOTIFY_002` | spotify | missing/invalid Spotify credentials |
| `NILS_SPOTIFY_003` | spotify | Spotify API unavailable/rate-limited |
| `NILS_TIMEZONE_001` | timezone | invalid timezone identifier |
| `NILS_TIMEZONE_002` | timezone | timezone conversion runtime failure |
| `NILS_WEATHER_001` | weather | invalid location arguments |
| `NILS_WEATHER_002` | weather | weather provider unavailable |
| `NILS_WEATHER_003` | weather | geocoding failure |
| `NILS_WIKI_001` | wiki | query empty |
| `NILS_WIKI_002` | wiki | invalid wiki config value |
| `NILS_WIKI_003` | wiki | Wikipedia API unavailable |
| `NILS_WORKFLOW_001` | workflow | project path not found/not directory |
| `NILS_WORKFLOW_002` | workflow | git origin/command failure |
| `NILS_YOUTUBE_001` | youtube | query empty |
| `NILS_YOUTUBE_002` | youtube | missing `YOUTUBE_API_KEY` |
| `NILS_YOUTUBE_003` | youtube | YouTube API unavailable/quota |

## Change Control

- New code allocation requires:
  - registry update in this file,
  - contract test update in the affected crate,
  - migration note in PR summary.
- Removing legacy JSON shapes does not remove registered codes; it only changes call paths.
