# `docs/arrows/` — Arrow of Intent Tracking

This directory tracks the arrow of intent across the project — the chain from high-level design through to realized code:

```
HLD → LLDs → EARS → Tests → Code
```

## Files in this directory

- **`index.yaml`** — The dependency graph. Load this first to understand what's available, what's blocked, and what needs work.
- **`backlog-format.md`** — File format parsing/serialization, tombstone, ID scheme, shared IO, config modules.
- **`cli.md`** — CLI shell: argument parsing, error handling strategy, exit codes, output conventions.
- **`commands.md`** — Single-entry commands: init, start, block, unblock, done, config.
- **`sync.md`** — `vat sync`: ID assignment, notes extraction, marker normalization, write semantics.

## Starting a session

1. Load `index.yaml`.
2. Find unblocked segments (none with `blockedBy` entries): `backlog-format` and `cli` are unblocked.
3. Load the relevant `{segment-name}.md`.
4. Follow its References to the LLD, spec file, tests, or code.

## Status enum

| Status | Meaning |
|---|---|
| UNMAPPED | Not yet explored |
| MAPPED | Structure known, specs not verified against code |
| AUDITED | Specs verified — implementation status understood |
| OK | Fully coherent — all specs implemented |
| PARTIAL | Some specs missing or partial |
| BROKEN | Code and docs have diverged significantly |
| STALE | Docs exist but outdated |
| OBSOLETE | Superseded, kept for historical reference |
| MERGED | Combined into another arrow (see `merged_into`) |

Normal progression: `UNMAPPED → MAPPED → AUDITED → OK`.

## Dependency order

```
backlog-format ──┐
                 ├──► commands
cli ─────────────┤
                 └──► sync
```

`commands` and `sync` are blocked until `backlog-format` (especially FMT-MARK-001..007) and `cli` (command wiring) reach OK.
