# Bangumi Playwright Bridge Design (Future Path)

> Status: frozen — pre-implementation design doc; the bridge is scaffolded behind
> `BANGUMI_SCRAPER_ENABLE` and the `scraper-bridge` cargo gate, both default-off. Live runtime stays
> API-first per [`workflow-contract.md`](workflow-contract.md).

This design describes a future Rust-to-Node Playwright bridge for Bangumi scraping.
Current runtime remains API-first; this bridge is scaffolded and disabled by default.

## Goals

- Define a concrete handoff-ready contract for a future Playwright scraping path.
- Keep current production path API-first with zero behavior change.
- Specify bridge I/O, errors, and rollout safety gates before implementation.

## Non-goals

- Do not switch current workflow runtime to scraper path.
- Do not require Node/Playwright for default packaging or smoke tests.
- Do not add login-required Bangumi user operations in this phase.

## Bridge Boundaries

- Rust owner (`nils-bangumi-cli`):
  - Parse workflow input and config.
  - Decide whether bridge feature flag is enabled.
  - Execute Node bridge process and validate JSON envelope.
  - Map bridge errors to Alfred fallback items.
- Node owner (`workflows/bangumi-search/scripts/bangumi_scraper.mjs`):
  - Receive typed request.
  - Run Playwright/browser extraction logic (future implementation).
  - Return stable JSON schema and typed error taxonomy.

## CLI Contract

Node entrypoint command surface:

```text
bangumi_scraper.mjs search --input "[type] query" [--max-results <n>] [--timeout-ms <ms>] [--fixture-html <path>]
```

Runtime switches:

- Feature flag (Rust + workflow): `BANGUMI_SCRAPER_ENABLE=true`
- Cargo gate (Rust compile): `scraper-bridge`

Default behavior:

- `BANGUMI_SCRAPER_ENABLE` absent/false => return `not_implemented` envelope.
- `script_filter.sh` must not invoke scraper directly while bridge is disabled by default.

## JSON Schema

Success envelope target (future):

```json
{
  "ok": true,
  "schema_version": "0.1.0",
  "stage": "search",
  "request": {
    "type": "anime",
    "query": "naruto",
    "max_results": 10
  },
  "items": [
    {
      "id": 2782,
      "name": "Naruto",
      "name_cn": "火影忍者",
      "url": "https://bgm.tv/subject/2782"
    }
  ]
}
```

Failure envelope target:

```json
{
  "ok": false,
  "schema_version": "0.1.0",
  "stage": "search",
  "error": {
    "code": "anti_bot",
    "message": "challenge page detected",
    "hint": "retry later",
    "retriable": true
  }
}
```

## Error Taxonomy

Required bridge error codes:

- `anti_bot`
- `cookie_wall`
- `timeout`
- `parse_error`
- `invalid_args`
- `not_implemented`
- `unknown`

Mapping requirements:

- Node returns one canonical code per failure.
- Rust maps code to stable Alfred titles/subtitles.
- Unknown/malformed envelopes must degrade safely to non-actionable error item.

## Runtime Bootstrap

Bootstrap steps for future enablement:

1. Ensure Node runtime is present (`node >= 24`).
2. Ensure workflow-local or repo-managed Playwright dependencies are installed.
3. Verify bridge script path (`BANGUMI_SCRAPER_SCRIPT`) resolves in package/repo layouts.
4. Run deterministic contract tests before live-network checks.

Current status:

- Runtime bootstrap is intentionally not required for default `bangumi-search` usage.
- Bridge remains disabled by default until rollout gates are met.

## Rollout Gates

Enablement checklist (all required):

1. Contract test suite green (`node --test ...bangumi_scraper_contract.test.mjs`).
2. Rust bridge feature builds in CI (`cargo check -p nils-bangumi-cli --features scraper-bridge`).
3. Observability baseline defined (error-code counts, timeout rate, fallback rate).
4. Abort conditions documented and rehearsed:
   - sustained `anti_bot`/`cookie_wall` spike,
   - parse failure ratio above threshold,
   - response latency budget breach.
5. Rollback switch validated:
   - set `BANGUMI_SCRAPER_ENABLE=false`,
   - keep API-first path active.

## Handoff Checklist

- Confirm this design doc is linked from crate docs index.
- Confirm scaffold files exist and are test-covered.
- Confirm workflow troubleshooting explicitly states scraper is disabled by default.
- Confirm script_filter runtime path has no direct scraper reference.
- Provide implementation handoff notes for next sprint with owner and acceptance tests.
