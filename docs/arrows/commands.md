# Arrow: commands

Single-entry and config commands — `vat init`, `start`, `block`, `unblock`, `done`, and `config`.

## Status

**PARTIAL** — last audited 2026-06-11 (git SHA `017ee5f`). Config commands (CMD-CFG-001 to 006) and `vat init` (CMD-INIT-001 to 007) fully implemented with tests. Start, block, unblock, and done are stubs blocked on FMT-MARK-* from `backlog-format`.

## References

### HLD
- docs/high-level-design.md (§ Commands table, § Key design decisions)

### LLD
- docs/llds/commands.md

### EARS
- docs/specs/commands-specs.md (35 active specs: 14 implemented, 21 gaps; 4 deferred)

### Tests
- src/cmd_config.rs (inline `#[cfg(test)]`)
- src/cmd_init.rs (inline `#[cfg(test)]`)
- src/readme_template.rs (inline `#[cfg(test)]`)

### Code
- src/main.rs — clap command dispatch; `cmd_init` dispatcher; stubs for start/block/unblock/done
- src/cmd_config.rs — `vat config get` and `vat config set` (CMD-CFG-001 to 006)
- src/cmd_init.rs — `vat init` full implementation (CMD-INIT-001 to 007, FMT-FM-005)
- src/readme_template.rs — baked-in `backlog/README.md` template (CMD-INIT-006)

## Architecture

**Purpose:** All VAT commands except `vat sync`. Config commands and `vat init` are fully implemented. Start, block, unblock, and done are scaffolded (argument types wired in clap) but return "not yet implemented" from their bodies.

**Key Components:**
1. `src/main.rs` — dispatch for all commands; stubs for start/block/unblock/done; `prompt_for_prefix()` for CMD-INIT-003
2. `src/cmd_config.rs` — full `vat config get/set` logic
3. `src/cmd_init.rs` — full `vat init` logic (CMD-INIT-001 to 007)
4. `src/readme_template.rs` — `BACKLOG_README_TEMPLATE` rendered with project prefix on init

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Cross-cutting | CMD-CC-001 to 003 | 1 | 0 | 2 |
| `vat init` | CMD-INIT-001 to 007 | 7 | 0 | 0 |
| `vat start` | CMD-START-001 to 003 | 0 | 0 | 3 |
| `vat block` | CMD-BLOCK-001 to 006 | 0 | 0 | 6 |
| `vat unblock` | CMD-UNBLOCK-001 to 002 | 0 | 0 | 2 |
| `vat done` | CMD-DONE-001 to 005 | 0 | 0 | 5 |
| `vat config` | CMD-CFG-001 to 006 | 6 | 0 | 0 |
| Exit codes | CMD-EXIT-001 to 003 | 0 | 0 | 3 |
| Deferred | CMD-LOCK-001, CMD-FORCE-001, CMD-DRYRUN-001, CMD-INIT-ADOPT-001 | 0 | 4 | 0 |

**Summary:** 14 of 35 active specs implemented (CMD-CC-001, CMD-INIT-001 to 007, CMD-CFG-001 to 006); 4 deferred; 21 gaps.

## Key Findings

1. **Config commands fully implemented** — `src/cmd_config.rs` covers CMD-CFG-001 to 006 with inline tests. `cmd_config_get` (main.rs:156) and `cmd_config_set` (main.rs:169) are both wired and annotated.

2. **`vat init` fully implemented** — `src/cmd_init.rs` (PR #29) implements CMD-INIT-001 to 007 with tests. The interactive-prompt path (`prompt_for_prefix()` in `main.rs`) covers CMD-INIT-003. `src/readme_template.rs` renders the baked-in template (CMD-INIT-006). `cmd_init.rs` also satisfies FMT-FM-005 by writing `"---\nversion: 1\n---\n"` to `backlog.md`.

3. **CMD-CC-001 implemented** — Version check on `backlog.md` reads is wired in `src/backlog_file.rs:163` (`check_version`). CMD-CC-002 (unknown ID error) and CMD-CC-003 (canonical marker emit) are gaps requiring the `find_entry` helper and marker parser.

4. **start/block/unblock/done are stubs** — `src/main.rs:135–153` shows each as "not yet implemented". Start, block, unblock, and done all need the `find_entry` shared helper and marker manipulation, which in turn need FMT-MARK-* from `backlog-format`.

5. **CMD-EXIT-001 to 003 unformalized** — Exit codes 0 and 1 are used in practice (cmd_config and main.rs) but code 2 (internal error) has no `@spec CMD-EXIT-003` annotation anywhere and is never emitted.

## Work Required

### Must Fix
1. Implement `find_entry` shared helper — blocked on FMT-MARK-* from `backlog-format`.
2. Implement `start`, `block`, `unblock`, `done` using `find_entry` once it exists.

### Should Fix
4. Add `@spec CMD-EXIT-003` and emit exit code 2 on internal errors (currently all errors exit 1).
5. Implement CMD-CC-002 (unknown-id error path) and CMD-CC-003 (canonical marker emit) as part of `find_entry`.
