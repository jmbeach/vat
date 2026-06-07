# Arrow: commands

Command handlers for `vat init`, `vat start`, `vat block`, `vat unblock`, `vat done`, and `vat config`. Does not cover `vat sync` (see `sync` segment).

## Status

**PARTIAL** — last audited 2026-06-07 (git SHA `fe4be98`). 1 of 35 active specs implemented (CMD-CC-001, the version-check helper in `backlog_file.rs`); 34 gaps; all command handlers are stubs in `main.rs`. `src/readme_template.rs` is ready (CMD-INIT-006 template) but not yet wired.

## References

### HLD
- docs/high-level-design.md (§ Commands, § Key design decisions §1 §4 §5)

### LLD
- docs/llds/commands.md

### EARS
- docs/specs/commands-specs.md (35 active specs; 4 deferred)

### Tests
- src/readme_template.rs (inline `#[cfg(test)]` module — covers CMD-INIT-006 template rendering)

### Code
- src/main.rs:92–130 — eight `cmd_*` stub functions
- src/readme_template.rs — README template baked into binary via `include_str!` (CMD-INIT-006)
- src/backlog_file.rs:158 — `check_version` helper (`@spec FMT-FM-002, CMD-CC-001`; cross-segment, owned by format layer)

## Architecture

**Purpose:** Implements each CLI subcommand (except sync). Each command follows parse → validate → mutate → write; shares a `find_entry(id)` helper and the common version check from the `backlog-format` layer.

**Key Components:**
1. `cmd_init` — creates `backlog/`, `vat.toml`, `backlog.md`, `.used-ids`, `README.md` (calls `readme_template::render`)
2. `cmd_start` — locates bullet, checks claim state, adds `[in-progress]` and `[by:<user>]` markers in canonical position
3. `cmd_block` / `cmd_unblock` — locates bullet, adds/replaces/removes `[blocked-by:...]` marker
4. `cmd_done` — removes bullet, deletes item file, appends to tombstone, auto-unblocks dependents
5. `cmd_config_get` / `cmd_config_set` — reads/writes user config or project config
6. `readme_template` — static README content rendered with project prefix substitution (CMD-INIT-006)

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
| Out of scope v1 | CMD-LOCK-001, CMD-FORCE-001, CMD-DRYRUN-001, CMD-INIT-ADOPT-001 | 0 | 4 | 0 |

**Summary:** 1 of 35 active specs implemented; 4 deferred; 34 gaps.

## Key Findings

1. **CMD-CC-001 cross-segment** — implemented and counted as implemented for this segment. See `backlog-format.md` Key Finding #4 for the rationale (the version-check helper lives in the format layer at `src/backlog_file.rs:158`).
2. **README template ready but not wired** — `src/readme_template.rs` implements CMD-INIT-006 (template with `{prefix}` substitution) and has test coverage. The `cmd_init` handler must call it — no change needed in `readme_template.rs`.
3. **All command handlers are stubs** — `src/main.rs:92–130`; all eight `cmd_*` functions print to stderr and call `std::process::exit(1)`. No business logic is present.
4. **Blocked on FMT-MARK-* in backlog-format** — `vat start`, `vat block`, `vat unblock`, `vat done`, and `vat sync` all need bullet marker parsing/serialization (FMT-MARK-001..007) to be implemented first.

## Work Required

### Must Fix
1. Implement FMT-MARK-001..007 in `backlog-format` segment first (hard dependency for all bullet-mutating commands)
2. Implement `cmd_init` (CMD-INIT-001..007) — wire `readme_template::render`, create files
3. Implement `cmd_start` (CMD-START-001..003)
4. Implement `cmd_block` (CMD-BLOCK-001..006)
5. Implement `cmd_unblock` (CMD-UNBLOCK-001..002)
6. Implement `cmd_done` (CMD-DONE-001..005)
7. Implement `cmd_config_get` / `cmd_config_set` (CMD-CFG-001..006)
8. Wire exit codes CMD-EXIT-001..003 (0 = success, 1 = user error, 2 = internal error)

### Should Fix
1. Implement CMD-CC-002 (error on unknown id) and CMD-CC-003 (canonical marker order) in the shared `find_entry` helper
