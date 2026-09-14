# Development log

A time-ordered narrative of notable work on `nils-alfredworkflow`: what shipped,
why it mattered, the evidence behind it, and the references worth keeping for
future debugging. It complements, rather than duplicates, the repository's other
records:

- Commit messages say what changed. The devlog preserves the non-obvious
  context, validation results, and external references a diff cannot carry.
- `README.md`, `DEVELOPMENT.md`, `docs/MAINTENANCE.md`, `docs/ARCHITECTURE.md`,
  and `docs/specs/` describe today's contract. The devlog is an append-only
  historical narrative; update the canonical owner first when behavior or
  guidance changes.
- Pull requests, plans, and reports retain detailed delivery evidence. The
  devlog summarizes the milestones that stay useful after those records close.

## When to add an entry

Add one after non-trivial work produces a durable outcome worth future lookup:
a shipped workflow or CLI capability, a validated packaging or release
milestone, a compatibility or security decision, an incident-relevant finding,
or a governance boundary that later work must respect. Skip trivial, transient,
and same-turn fixes with no future debugging or decision value: screenshot
refreshes, version-bump commits, formatting-only lint repairs, and routine
dependency bumps that changed no behavior.

## Conventions

- One file per month: `docs/devlog/YYYY-MM.md`, newest entry first.
- When a month file is created, add it to the `## Months` index below, newest
  first. The index is the only advertised discovery path, and no gate detects a
  month file that is missing from it.
- Write in English, like the rest of the repository.
- Keep current docs current. The devlog records history; it does not own the
  current runtime contract, policy, setup, or runbook.
- This is a public repository. Never record secrets, tokens, credentials,
  personal identifiers, internal hostnames, private topology, or machine-local
  paths. Reference identifiers and public URLs, never values.
- Prefer inline code spans for repository paths and full URLs for pull
  requests and issues, so the log survives file moves without breaking the
  local-link audit in `scripts/ci/markdownlint-audit.sh`.
- Search past entries with `devlog search <term> [--month YYYY-MM]`.
- When an entry is committed separately, use
  `docs(devlog): <YYYY-MM> - <subject>`.

### Entry template

```md
## YYYY-MM-DD - <short title>

**Result**

- What shipped or changed.

**Why / context**

- The non-obvious reasoning or compatibility context.

**Evidence**

- Commands run and concrete observations.

**Links**

- Commits, issues, pull requests, external references, and relevant docs.

**Follow-ups**

- Optional.
```

## Backfill note

Entries dated before 2026-09-14 were reconstructed on 2026-09-14 from the
repository's commit and pull-request history, the plans and reports under
`docs/`, and the checked-in gate scripts. Their **Evidence** sections cite that
record rather than a live run at the time of writing; where a commit body
recorded the gate it ran, the entry names that gate. Entries written from
2026-09-14 onward record evidence observed in the session that produced them.

## Months

- [2026-09](2026-09.md)
- [2026-08](2026-08.md)
- [2026-07](2026-07.md)
- [2026-06](2026-06.md)
- [2026-05](2026-05.md)
- [2026-04](2026-04.md)
- [2026-03](2026-03.md)
- [2026-02](2026-02.md)
