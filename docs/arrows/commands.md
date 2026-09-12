# Arrow: commands

Single-entry and config commands — `vat init`, `start`, `block`, `unblock`, `done`, and `config`.

## Status

**OK** — re-verified 2026-09-12 (HEAD `c265c95`). All commands fully implemented. `classify_exit_code` and `classify_sync_exit_code` wired through every command dispatcher; CMD-EXIT-003 now applies to all commands including `cmd_init` and `cmd_sync`.

## References

### HLD
- docs/high-level-design.md (§ Commands table, § Key design decisions)

### LLD
- docs/llds/commands.md

### EARS
- docs/specs/commands-specs.md (43 active specs: all 43 marked `[x]` in the file; 4 deferred; 0 gaps)

### Tests
- src/cmd_config.rs (inline `#[cfg(test)]`)
- src/cmd_init.rs (inline `#[cfg(test)]`)
- src/cmd_start.rs (inline `#[cfg(test)]`)
- src/cmd_block.rs (inline `#[cfg(test)]` — CMD-BLOCK-001 to 006, CMD-CC-001/002/004)
- src/cmd_unblock.rs (inline `#[cfg(test)]`)
- src/cmd_done.rs (inline `#[cfg(test)]`)
- src/cmd_completions.rs (inline `#[cfg(test)]` — CMD-COMP-001 to 003, CMD-COMP-005)
- src/readme_template.rs (inline `#[cfg(test)]`)
- tests/commands_golden.rs (fixture-directory golden tests for start, block, unblock, done — real binary)
- tests/completions.rs (black-box CMD-COMP-001/002/004 tests — real binary)

### Code
- src/main.rs — clap command dispatch; all command dispatchers; `classify_exit_code()` (CMD-EXIT-001 to 003)
- src/cmd_config.rs — `vat config get` and `vat config set` (CMD-CFG-001 to 006)
- src/cmd_init.rs — `vat init` full implementation (CMD-INIT-001 to 007, FMT-FM-005)
- src/cmd_start.rs — `vat start <id>` (CMD-START-001 to 004) + the shared `find_entry_index`/`EntryLookup` lookup helper (CMD-CC-002, CMD-CC-004) and the interim-shared `serialize_region_with_replaced_bullet` emitter
- src/cmd_block.rs — `vat block <id> <blocker-id>` (CMD-BLOCK-001 to 006); reuses `find_entry_index` and `Bullet` marker machinery
- src/cmd_unblock.rs — `vat unblock <id>` (CMD-UNBLOCK-001 to 002); reuses `find_entry_index` (and the `Bullet` it parses) plus the shared emitter
- src/cmd_done.rs — `vat done <id>` (CMD-DONE-001 to 005); reuses the `find_entry_index` helper and the `Bullet`/`ParsedRegion` parse-emit machinery
- src/cmd_completions.rs — `vat completions <shell>` (CMD-COMP-001 to 005); bash/zsh/fish via clap_complete; hidden from help
- src/readme_template.rs — baked-in `backlog/README.md` template (CMD-INIT-006)
- src/errors.rs — `UserError`: typed wrapper for user-facing errors; exit-code classification anchor (CMD-EXIT-002)

## Architecture

**Purpose:** All VAT commands except `vat sync`. All commands are fully implemented: `vat init`, `vat start`, `vat block`, `vat unblock`, `vat done`, `vat config`, and `vat completions`. No remaining gaps.

**Key Components:**
1. `src/main.rs` — dispatch for all commands; `prompt_for_prefix()` for CMD-INIT-003; `classify_exit_code()` (CMD-EXIT-001 to 003)
2. `src/cmd_config.rs` — full `vat config get/set` logic
3. `src/cmd_init.rs` — full `vat init` logic (CMD-INIT-001 to 007)
4. `src/cmd_start.rs` — full `vat start` logic plus the shared `find_entry_index`/`EntryLookup` lookup helper and interim-shared `serialize_region_with_replaced_bullet` emitter
5. `src/cmd_block.rs` — full `vat block` logic (CMD-BLOCK-001 to 006); reuses `find_entry_index` and `Bullet` marker machinery
6. `src/cmd_unblock.rs` — full `vat unblock` logic (strip `[blocked-by:...]`, or no-op when absent), reusing `find_entry_index` and the shared emitter
7. `src/cmd_done.rs` — full `vat done` logic (entry removal + auto-unblock), reusing `find_entry_index` and the bullet/region machinery
8. `src/cmd_completions.rs` — shell completion generation (CMD-COMP-001 to 005); hidden `Completions` subcommand; bash/zsh/fish via `clap_complete`
9. `src/readme_template.rs` — `BACKLOG_README_TEMPLATE` rendered with project prefix on init

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Cross-cutting | CMD-CC-001 to 004 | 4 | 0 | 0 |
| `vat init` | CMD-INIT-001 to 007 | 7 | 0 | 0 |
| `vat start` | CMD-START-001 to 004 | 4 | 0 | 0 |
| `vat block` | CMD-BLOCK-001 to 006 | 6 | 0 | 0 |
| `vat unblock` | CMD-UNBLOCK-001 to 002 | 2 | 0 | 0 |
| `vat done` | CMD-DONE-001 to 005 | 5 | 0 | 0 |
| `vat config` | CMD-CFG-001 to 006 | 6 | 0 | 0 |
| `vat completions` | CMD-COMP-001 to 005 | 5 | 0 | 0 |
| Exit codes | CMD-EXIT-001 to 003 | 3 | 0 | 0 |
| Deferred | CMD-LOCK-001, CMD-FORCE-001, CMD-DRYRUN-001, CMD-INIT-ADOPT-001 | 0 | 4 | 0 |

**Summary:** 43 of 43 active specs implemented; 4 deferred; 0 gaps.

## Key Findings

1. **Config commands fully implemented** — `src/cmd_config.rs` covers CMD-CFG-001 to 006 with inline tests. `cmd_config_get` (main.rs:156) and `cmd_config_set` (main.rs:169) are both wired and annotated.

2. **`vat init` fully implemented** — `src/cmd_init.rs` (PR #29) implements CMD-INIT-001 to 007 with tests. The interactive-prompt path (`prompt_for_prefix()` in `main.rs`) covers CMD-INIT-003. `src/readme_template.rs` renders the baked-in template (CMD-INIT-006). `cmd_init.rs` also satisfies FMT-FM-005 by writing `"---\nversion: 1\n---\n"` to `backlog.md`.

3. **Cross-cutting layer fully implemented** — Version check on `backlog.md` reads is wired in `src/backlog_file.rs` (`check_version`, CMD-CC-001). CMD-CC-002 (unknown ID error), CMD-CC-003 (canonical marker emit), and CMD-CC-004 (present-but-malformed bullet) are all satisfied by the shared `find_entry_index`/`EntryLookup` helper in `src/cmd_start.rs` and the `Bullet` parse-emit path, consumed by both `cmd_start` and `cmd_done`.

4. **`vat block` fully implemented** — `src/cmd_block.rs` implements CMD-BLOCK-001 to 006 with full inline tests. Self-block guard (CMD-BLOCK-001) fires before any file read. Blocker validation (CMD-BLOCK-002/002a) requires a well-formed bullet, mirroring the target-id handling (CMD-CC-004). Idempotent re-block (CMD-BLOCK-003) is a no-op without writing. Replace-existing-blocker (CMD-BLOCK-004) and add-when-absent (CMD-BLOCK-005) set `Bullet.blocked_by` and re-serialize; CMD-BLOCK-006 explicitly allows cycles. The `find_entry_index` helper and `Bullet`/`ParsedRegion` machinery already proven by `cmd_start`, `cmd_unblock`, and `cmd_done` are reused unchanged.

5. **Exit-code framework (CMD-EXIT-001 to 003) fully wired** — `classify_exit_code()` at `src/main.rs` chain-searches the anyhow error for typed variants (`ConfigError`, `UserConfigError`, `UnsupportedVersion`, `UserError`) and maps to exit 1 (user-facing) or 2 (internal). `cmd_init` uses a typed match on `InitError` (Io → 2, others → 1). `cmd_sync` routes through `classify_sync_exit_code()` which exhaustively matches all `SyncError` variants. Shared helpers `classify_config_error()` and `classify_tombstone_error()` are called by both classifiers — single source of truth per error type. `UserError` in `src/errors.rs` lifts untyped `bail!` messages into the scheme. All three exit codes are `@spec`-annotated; unit tests cover every variant.

6. **Shell completions (`vat completions`) added** — `src/cmd_completions.rs` implements CMD-COMP-001 to 005 via `clap_complete`. The `Completions` subcommand is hidden from `vat --help` (CMD-COMP-003) via `#[command(hide = true)]` in `src/main.rs`; `visible_command()` rebuilds the CLI tree without hidden subcommands so generated scripts don't advertise it. Supported shells are exactly bash/zsh/fish (CMD-COMP-002) — narrower than `clap_complete`'s built-in set. Write failures propagate rather than panic (CMD-COMP-005); broken pipe is swallowed silently. CMD-COMP-004 (invalid shell → exit 2 usage error) is handled automatically by clap's `ValueEnum` constraint.

## Work Required

None. All active specs are implemented; deferred specs (CMD-LOCK-001, CMD-FORCE-001, CMD-DRYRUN-001, CMD-INIT-ADOPT-001) are explicitly out of scope for v1.
