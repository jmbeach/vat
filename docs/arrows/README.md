# `docs/arrows/` — Arrow of Intent Tracking

This directory tracks the arrow of intent across the project — the chain from high-level design through to realized code:

```
HLD → LLDs → EARS → Tests → Code
```

## Files in this directory

- **`index.yaml`** — The dependency graph. Load this first to understand what's available, what's blocked, and what needs work.
- **`backlog-format.md`** — File format, parsing, base32, tombstone, item files, config (PARTIAL — 38/55 specs implemented).
- **`cli.md`** — CLI shell: argument parsing, error handling, exit codes, output conventions (MAPPED — no spec file yet).
- **`commands.md`** — Single-entry and config commands: init, start, block, unblock, done, config (MAPPED — all stubs).
- **`sync.md`** — `vat sync` algorithm: ID assignment, notes extraction, marker normalization (MAPPED — all stubs).

## Starting a session

1. Load `index.yaml`.
2. Find unblocked segments: `backlog-format` and `cli` have no `blockedBy` — start there.
3. Load the relevant `{segment-name}.md`.
4. Follow its References to the LLD, spec file, tests, or code.

## Project conventions

- **Tests**: inline Rust `#[test]` modules inside each source file (no separate test directory). `@spec` annotations in test functions serve as coverage links.
- **@spec placement**: entry point of the behavior's implementation graph (module-level or function-level comment).
- **Eval assertions**: not used in this project; `@spec`-annotated test functions are the coverage mechanism.

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
