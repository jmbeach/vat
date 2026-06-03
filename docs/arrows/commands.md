# Arrow: commands

Single-entry and config commands: `vat init`, `vat start`, `vat block`, `vat unblock`, `vat done`, `vat config get`, `vat config set`. These share the `find_entry` helper and the version-check gate; each performs a parse → single-line mutation → write cycle.

## Status

**MAPPED** — last audited 2026-06-03 (git SHA `52bbfb58a6f7f999969da68bef55b38bd59fb744`). LLD and EARS spec file exist. All 35 active CMD-* specs are unimplemented; all `cmd_*` functions in `main.rs` are stubs. Blocked on `backlog-format` (needs task-entry parsing and marker normalization) and `cli` (needs error-handling wiring).

## References

### HLD
- docs/high-level-design.md (§ Commands)

### LLD
- docs/llds/commands.md

### EARS
- docs/specs/commands-specs.md (39 specs: 0 implemented, 35 active gaps, 4 deferred)

### Tests
- (none — no behavior implemented yet)

### Code
- src/main.rs — cmd_init(), cmd_start(), cmd_block(), cmd_unblock(), cmd_done(), cmd_config_get(), cmd_config_set() (all stubs)

## Architecture

**Purpose:** Implements every VAT command except `vat sync`. Each command is a one-shot parse → mutate → write cycle on `backlog.md` and related files.

**Key Components:**
1. `find_entry(id)` helper — shared by start/block/unblock/done; loads + parses + locates bullet by ID
2. Version-check gate — every command that reads `backlog.md` verifies `version` ≤ 1 before any other work (CMD-CC-001)
3. `vat init` — creates `backlog/` directory tree; validates and stores project prefix
4. `vat start` — claims a task with `[in-progress] [by:<user>]`; refuses if already claimed
5. `vat block` / `vat unblock` — adds or removes `[blocked-by:<id>]` marker
6. `vat done` — removes bullet, deletes item file, auto-unblocks dependents, appends to tombstone
7. `vat config get` / `set` — reads/writes user or project config with validation

## Spec Coverage

| Category | Spec IDs | Total | Implemented | Deferred | Gaps |
|----------|----------|-------|-------------|----------|------|
| Cross-cutting | CMD-CC-001..003 | 3 | 0 | 0 | 3 |
| vat init | CMD-INIT-001..007 | 7 | 0 | 0 | 7 |
| vat start | CMD-START-001..003 | 3 | 0 | 0 | 3 |
| vat block | CMD-BLOCK-001..006 | 6 | 0 | 0 | 6 |
| vat unblock | CMD-UNBLOCK-001..002 | 2 | 0 | 0 | 2 |
| vat done | CMD-DONE-001..005 | 5 | 0 | 0 | 5 |
| vat config | CMD-CFG-001..006 | 6 | 0 | 0 | 6 |
| Exit codes | CMD-EXIT-001..003 | 3 | 0 | 0 | 3 |
| Deferred | CMD-LOCK-001, CMD-FORCE-001, CMD-DRYRUN-001, CMD-INIT-ADOPT-001 | 4 | 0 | 4 | 0 |
| **Total** | | **39** | **0** | **4** | **35** |

**Summary:** 0 of 35 active specs implemented; 4 deferred.

## Key Findings

1. **All commands are stubs** — Every `cmd_*` function in `main.rs` prints "not yet implemented" and exits 1. No implementations exist yet.

2. **Blocked on backlog-format** — Command bodies need `find_entry` which depends on task-entry parsing (FMT-PARSE-*) and marker normalization (FMT-MARK-*), both unimplemented in the backlog-format segment.

3. **No @spec annotations in main.rs** — Once commands are implemented, each `cmd_*` function should carry `// @spec` annotations linking to its CMD-* IDs.

4. **CMD-INIT-005 / FMT-FM-005 cross-segment dependency** — `vat init` writes the initial `backlog.md` with `version: 1` frontmatter. FMT-FM-005 is in backlog-format-specs but the writing logic belongs in this segment's `cmd_init`. Coordinate across segments when implementing.

## Work Required

### Must Fix
1. Implement `find_entry(id)` helper (shared by start/block/unblock/done)
2. Implement version-check gate (CMD-CC-001)
3. Implement `vat init` (CMD-INIT-001..007)
4. Implement `vat start` (CMD-START-001..003)
5. Implement `vat block` (CMD-BLOCK-001..006)
6. Implement `vat unblock` (CMD-UNBLOCK-001..002)
7. Implement `vat done` (CMD-DONE-001..005)
8. Implement `vat config get` / `set` (CMD-CFG-001..006)
9. Wire exit codes (CMD-EXIT-001..003)
