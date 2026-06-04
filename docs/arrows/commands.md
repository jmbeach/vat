# Arrow: commands

Single-entry and config commands: `vat init`, `vat start`, `vat block`, `vat unblock`, `vat done`, `vat config`.

## Status

**PARTIAL** — last audited 2026-06-04 (git SHA `426964053f024c0e1380a365543da31798536bb7`). One spec implemented (CMD-CC-001, the version-check guard in backlog_file.rs). All command handlers in src/main.rs are unimplemented stubs. README template for `vat init` is ready in src/readme_template.rs but not yet wired.

## References

### HLD
- docs/high-level-design.md (§ Commands table, § Key design decisions §4–6)

### LLD
- docs/llds/commands.md

### EARS
- docs/specs/commands-specs.md (39 specs: 1 implemented, 34 gaps, 4 deferred)

### Tests
- src/readme_template.rs (inline `#[cfg(test)]` — CMD-INIT-006)

### Code
- src/main.rs:92–130 — all cmd_* handlers (stubs)
- src/readme_template.rs — README template for vat init (CMD-INIT-006)
- src/backlog_file.rs:158 — check_version() (CMD-CC-001, FMT-FM-002)

## Architecture

**Purpose:** Implements every VAT command except `vat sync`. All bullet-mutating commands share a `find_entry(id)` helper pattern: load → parse → locate → mutate → serialize → write.

**Key Components:**
1. `vat init` — creates backlog/ directory structure; validated prefix; writes vat.toml, backlog.md, .used-ids, README.md
2. `vat start` — adds `[in-progress]` and `[by:<user>]` markers; refuses if either is already present
3. `vat block` / `vat unblock` — adds/removes `[blocked-by:<id>]` marker
4. `vat done` — deletes bullet, deletes item file, appends to .used-ids, auto-unblocks dependents
5. `vat config` — get/set for `user.name` (global) and `project.id` (project)

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Cross-cutting | CMD-CC-001 to CMD-CC-003 | 1 | 0 | 2 |
| vat init | CMD-INIT-001 to CMD-INIT-007 | 0 | 0 | 7 |
| vat start | CMD-START-001 to CMD-START-003 | 0 | 0 | 3 |
| vat block | CMD-BLOCK-001 to CMD-BLOCK-006 | 0 | 0 | 6 |
| vat unblock | CMD-UNBLOCK-001 to CMD-UNBLOCK-002 | 0 | 0 | 2 |
| vat done | CMD-DONE-001 to CMD-DONE-005 | 0 | 0 | 5 |
| vat config | CMD-CFG-001 to CMD-CFG-006 | 0 | 0 | 6 |
| Exit codes | CMD-EXIT-001 to CMD-EXIT-003 | 0 | 0 | 3 |
| Locks (deferred) | CMD-LOCK-001 | 0 | 1 | 0 |
| Force flag (deferred) | CMD-FORCE-001 | 0 | 1 | 0 |
| Dry-run (deferred) | CMD-DRYRUN-001 | 0 | 1 | 0 |
| Init adopt (deferred) | CMD-INIT-ADOPT-001 | 0 | 1 | 0 |

**Summary:** 1 of 35 active specs implemented; 4 deferred; 34 gaps.

## Key Findings

1. **All command handlers are stubs** — src/main.rs:92–130 contains eight functions that each `eprintln!("...: not yet implemented")` and `std::process::exit(1)`. No command logic exists.
2. **CMD-INIT-006 drift** — `src/readme_template.rs` fully implements the README template for `vat init`, annotated `@spec CMD-INIT-006`, but the spec is `[ ]`. The template code is ready; it becomes active once cmd_init() is wired.
3. **CMD-CC-001 implemented** — `check_version()` in `src/backlog_file.rs:158` is annotated `@spec FMT-FM-002, CMD-CC-001` and enforces the version-check guard. This is the one implemented spec; the wiring in cmd_* handlers will call it when handlers are written.
4. **find_entry() helper not yet written** — The shared `find_entry(id)` helper described in the LLD (commands.md §Common machinery) has no implementation. Implementing it is a prerequisite for start, block, unblock, and done.
5. **Hard blocker** — FMT-PARSE and FMT-MARK gaps in `backlog-format` must be resolved before any bullet-mutating command can work. The `blockedBy: [backlog-format]` dependency applies.

## Work Required

### Must Fix
1. Implement `find_entry()` shared helper (src/main.rs or a new module) — prerequisite for start, block, unblock, done
2. Implement cmd_init() — creates backlog/ structure (CMD-INIT-001–007); wires readme_template
3. Implement cmd_start(), cmd_block(), cmd_unblock(), cmd_done() — depends on find_entry() and FMT-PARSE/FMT-MARK
4. Implement cmd_config_get() and cmd_config_set() — reads/writes user_config and project_config modules (already implemented)

### Should Fix
5. Update CMD-INIT-006 spec marker to `[x]` when cmd_init() is wired
6. Update CMD-CC-001 coverage note: the spec is `[x]` but only the check function exists; it needs to be called in each handler
