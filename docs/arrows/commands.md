# Arrow: commands

Single-entry and config commands — `vat init`, `start`, `block`, `unblock`, `done`, and `config`.

## Status

**PARTIAL** — last audited 2026-06-13. Config commands (CMD-CFG-001 to 006), `vat init` (CMD-INIT-001 to 007), `vat start` (CMD-START-001 to 004), and `vat done` (CMD-DONE-001 to 005) are fully implemented with tests, as is the cross-cutting layer (CMD-CC-001 to 004). Exit-code framework (CMD-EXIT-001 to 003) implemented and tested, and now wired through `cmd_config`, `cmd_start`, and `cmd_done` via `classify_exit_code` (see Key Finding #5). `block` and `unblock` remain stubs.

## References

### HLD
- docs/high-level-design.md (§ Commands table, § Key design decisions)

### LLD
- docs/llds/commands.md

### EARS
- docs/specs/commands-specs.md (35 active specs: 17 marked `[x]`, 18 open; 4 deferred — see Spec Coverage for reality-adjusted counts)

### Tests
- src/cmd_config.rs (inline `#[cfg(test)]`)
- src/cmd_init.rs (inline `#[cfg(test)]`)
- src/cmd_start.rs (inline `#[cfg(test)]`)
- src/cmd_done.rs (inline `#[cfg(test)]`)
- src/readme_template.rs (inline `#[cfg(test)]`)

### Code
- src/main.rs — clap command dispatch; `cmd_init`/`cmd_start`/`cmd_done` dispatchers; stubs for block/unblock; `classify_exit_code()` (CMD-EXIT-001 to 003)
- src/cmd_config.rs — `vat config get` and `vat config set` (CMD-CFG-001 to 006)
- src/cmd_start.rs — `vat start <id>` (CMD-START-001 to 004) + the shared `find_entry_index`/`EntryLookup` lookup helper (CMD-CC-002, CMD-CC-004)
- src/cmd_done.rs — `vat done <id>` (CMD-DONE-001 to 005); reuses the `find_entry_index` helper and the `Bullet`/`ParsedRegion` parse-emit machinery
- src/cmd_init.rs — `vat init` full implementation (CMD-INIT-001 to 007, FMT-FM-005)
- src/readme_template.rs — baked-in `backlog/README.md` template (CMD-INIT-006)
- src/errors.rs — `UserError`: typed wrapper for user-facing errors; exit-code classification anchor (CMD-EXIT-002)

## Architecture

**Purpose:** All VAT commands except `vat sync`. Config, `vat init`, `vat start`, and `vat done` are fully implemented. `block` and `unblock` are scaffolded (argument types wired in clap) but return "not yet implemented" from their bodies.

**Key Components:**
1. `src/main.rs` — dispatch for all commands; stubs for block/unblock; `prompt_for_prefix()` for CMD-INIT-003
2. `src/cmd_config.rs` — full `vat config get/set` logic
3. `src/cmd_init.rs` — full `vat init` logic (CMD-INIT-001 to 007)
4. `src/cmd_start.rs` — full `vat start` logic plus the shared `find_entry_index` lookup helper
5. `src/cmd_done.rs` — full `vat done` logic (entry removal + auto-unblock), reusing `find_entry_index` and the bullet/region machinery
6. `src/readme_template.rs` — `BACKLOG_README_TEMPLATE` rendered with project prefix on init

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Cross-cutting | CMD-CC-001 to 004 | 4 | 0 | 0 |
| `vat init` | CMD-INIT-001 to 007 | 7 | 0 | 0 |
| `vat start` | CMD-START-001 to 004 | 4 | 0 | 0 |
| `vat block` | CMD-BLOCK-001 to 006 | 0 | 0 | 6 |
| `vat unblock` | CMD-UNBLOCK-001 to 002 | 0 | 0 | 2 |
| `vat done` | CMD-DONE-001 to 005 | 5 | 0 | 0 |
| `vat config` | CMD-CFG-001 to 006 | 6 | 0 | 0 |
| Exit codes | CMD-EXIT-001 to 003 | 2 | 0 | 1 |
| Deferred | CMD-LOCK-001, CMD-FORCE-001, CMD-DRYRUN-001, CMD-INIT-ADOPT-001 | 0 | 4 | 0 |

**Summary:** 28 of 37 active specs implemented (CMD-CC-001 to 004, CMD-INIT-001 to 007, CMD-START-001 to 004, CMD-DONE-001 to 005, CMD-CFG-001 to 006, CMD-EXIT-001 and 002); 4 deferred; 9 gaps (`block` 6, `unblock` 2, and CMD-EXIT-003). Note: `commands-specs.md` marks CMD-EXIT-003 `[x]`, but it is counted as a gap here because `cmd_init` and `cmd_sync` exit 1 for internal errors (see Key Finding #5 and the `commands` drift entry in `index.yaml`).

## Key Findings

1. **Config commands fully implemented** — `src/cmd_config.rs` covers CMD-CFG-001 to 006 with inline tests. `cmd_config_get` (main.rs:156) and `cmd_config_set` (main.rs:169) are both wired and annotated.

2. **`vat init` fully implemented** — `src/cmd_init.rs` (PR #29) implements CMD-INIT-001 to 007 with tests. The interactive-prompt path (`prompt_for_prefix()` in `main.rs`) covers CMD-INIT-003. `src/readme_template.rs` renders the baked-in template (CMD-INIT-006). `cmd_init.rs` also satisfies FMT-FM-005 by writing `"---\nversion: 1\n---\n"` to `backlog.md`.

3. **Cross-cutting layer fully implemented** — Version check on `backlog.md` reads is wired in `src/backlog_file.rs` (`check_version`, CMD-CC-001). CMD-CC-002 (unknown ID error), CMD-CC-003 (canonical marker emit), and CMD-CC-004 (present-but-malformed bullet) are all satisfied by the shared `find_entry_index`/`EntryLookup` helper in `src/cmd_start.rs` and the `Bullet` parse-emit path, consumed by both `cmd_start` and `cmd_done`.

4. **`block` and `unblock` remain stubs** — `src/main.rs` returns "not yet implemented" for both. They need the same `find_entry_index` helper (already available) plus marker add/remove logic. `start` and `done` are now fully implemented on top of that helper and the FMT-MARK-* bullet machinery.

5. **Exit-code framework (CMD-EXIT-001 to 003) implemented for cmd_config** — `classify_exit_code()` at `src/main.rs:183` chain-searches the anyhow error for typed variants (`ConfigError`, `UserConfigError`, `UnsupportedVersion`, `UserError`) and maps them to exit 1 (user-facing) or 2 (internal). `UserError` in `src/errors.rs` lifts untyped `bail!` messages into the classification scheme. 17 unit tests in `src/main.rs`; all three exit codes are `@spec`-annotated. **Partial coverage:** `classify_exit_code` is currently wired only through `cmd_config_get` and `cmd_config_set` — `cmd_init` and `cmd_sync` still exit 1 for all errors.

## Work Required

### Must Fix
1. Implement `block` and `unblock` using the existing `find_entry_index` helper and the `Bullet` marker add/remove path.

### Should Fix
2. Thread `classify_exit_code` through `cmd_init` and `cmd_sync` so CMD-EXIT-003 applies to every command, not just `cmd_config`/`cmd_start`/`cmd_done` operations.
