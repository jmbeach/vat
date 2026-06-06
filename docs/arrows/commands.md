# Arrow: commands

Single-entry and config commands — `vat init`, `vat start`, `vat block`, `vat unblock`, `vat done`, and `vat config`.

## Status

**PARTIAL** — last audited 2026-06-06 (git SHA `426964053f024c0e1380a365543da31798536bb7`). Supporting infrastructure is in place (README template module, config parsing). All command implementations are stubs. 34 of 35 active specs are gaps.

## References

### HLD
- docs/high-level-design.md (§ Commands, § Key design decisions)

### LLD
- docs/llds/commands.md

### EARS
- docs/specs/commands-specs.md (35 active specs, 4 deferred)

### Tests
- src/readme_template.rs (inline `#[cfg(test)]` module, covers CMD-INIT-006)

### Code
- src/main.rs (all command stubs: `cmd_init`, `cmd_start`, `cmd_block`, `cmd_unblock`, `cmd_done`, `cmd_config_get`, `cmd_config_set`)
- src/readme_template.rs (`@spec CMD-INIT-006` — README template logic, not yet wired to init)

## Architecture

**Purpose:** Implement the short, targeted mutation commands. Each reads `backlog.md`, locates the matching bullet, applies a single change, and writes back.

**Key Components:**
1. `cmd_init` in `main.rs` — create `backlog/`, write initial files (stub)
2. `cmd_start` / `cmd_block` / `cmd_unblock` / `cmd_done` in `main.rs` — bullet-mutating commands (stubs)
3. `cmd_config_get` / `cmd_config_set` in `main.rs` — config read/write (stubs)
4. `src/readme_template.rs` — README template rendered at init time (implemented, not yet wired)

## Spec Coverage

| Category | Spec IDs | Implemented | Gaps | Deferred |
|----------|----------|-------------|------|----------|
| Cross-cutting (CMD-CC) | CMD-CC-001 to CMD-CC-003 | 1 | 2 | 0 |
| Init (CMD-INIT) | CMD-INIT-001 to CMD-INIT-007 | 0 | 7 | 0 |
| Start (CMD-START) | CMD-START-001 to CMD-START-003 | 0 | 3 | 0 |
| Block (CMD-BLOCK) | CMD-BLOCK-001 to CMD-BLOCK-006 | 0 | 6 | 0 |
| Unblock (CMD-UNBLOCK) | CMD-UNBLOCK-001 to CMD-UNBLOCK-002 | 0 | 2 | 0 |
| Done (CMD-DONE) | CMD-DONE-001 to CMD-DONE-005 | 0 | 5 | 0 |
| Config (CMD-CFG) | CMD-CFG-001 to CMD-CFG-006 | 0 | 6 | 0 |
| Exit codes (CMD-EXIT) | CMD-EXIT-001 to CMD-EXIT-003 | 0 | 3 | 0 |
| Deferred | CMD-LOCK-001, CMD-FORCE-001, CMD-DRYRUN-001, CMD-INIT-ADOPT-001 | — | — | 4 |

**Summary:** 1 of 35 active specs implemented (CMD-CC-001 — version check, implemented in `backlog_file.rs`); 34 gaps; 4 deferred.

## Key Findings

1. **All command implementations are stubs** — Every `cmd_*` function in `src/main.rs` prints "not yet implemented" and exits 1. The LLD and specs are complete; implementation is the remaining work.
2. **CMD-INIT-006 partially done** — `src/readme_template.rs` implements the README template and render function with `@spec CMD-INIT-006` annotations; spec is correctly `[ ]` because the write wiring (`vat init` → `readme_template::render`) is not yet connected.
3. **CMD-CC-001 implemented cross-segment** — The version check is implemented in `src/backlog_file.rs` (frontmatter parser) rather than in a commands-specific file; this is architecturally correct per the LLD.

## Work Required

### Must Fix
1. Implement `cmd_init` — create `backlog/`, write `vat.toml`, `backlog.md`, `.used-ids`, and `README.md` (CMD-INIT-001 to CMD-INIT-007); wire `readme_template::render` here
2. Implement `cmd_start` — resolve user.name, find entry, refuse if claimed, write markers (CMD-START-001 to CMD-START-003)
3. Implement `cmd_block` / `cmd_unblock` — self-block guard, blocker existence check, marker mutation (CMD-BLOCK-001 to CMD-BLOCK-006, CMD-UNBLOCK-001 to CMD-UNBLOCK-002)
4. Implement `cmd_done` — remove bullet, delete item file, append tombstone, auto-unblock (CMD-DONE-001 to CMD-DONE-005)
5. Implement `cmd_config_get` / `cmd_config_set` — read/write user and project config, validate prefix-change guard (CMD-CFG-001 to CMD-CFG-006)
6. Implement exit codes per CMD-EXIT-001 to CMD-EXIT-003
