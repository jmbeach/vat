# Arrow: commands

Single-entry and config commands — `vat init`, `vat start`, `vat block`, `vat unblock`, `vat done`, `vat config get/set`.

## Status

**PARTIAL** — last audited 2026-06-08 (git SHA `e2c7ad8cf75a7da4a970a1eedfb8b6e5784d4c14`). Library modules that commands depend on are implemented (backlog-format segment). Command bodies in `main.rs` are all stubs. Only CMD-CC-001 (version check) is implemented via `backlog_file.rs`, and the README template (CMD-INIT-006 partial) lives in `readme_template.rs`.

## References

### HLD
- docs/high-level-design.md (§ Commands, § Key design decisions §1-§6)

### LLD
- docs/llds/commands.md

### EARS
- docs/specs/commands-specs.md (39 specs: 1 implemented, 34 active gaps, 4 deferred)

### Tests
- src/readme_template.rs (inline tests for CMD-INIT-006)

### Code
- src/main.rs — command stubs: `cmd_init`, `cmd_start`, `cmd_block`, `cmd_unblock`, `cmd_done`, `cmd_config_get`, `cmd_config_set`
- src/readme_template.rs — README template baked into binary (`@spec CMD-INIT-006`)
- src/backlog_file.rs — version check machinery (`@spec CMD-CC-001`)

## Architecture

**Purpose:** Each command is a one-shot read → mutate → write cycle. All bullet-mutating commands share `find_entry(id)` to locate the target task; after mutation they serialize back through the same emitter `sync` uses. Config commands delegate to `project_config` and `user_config`.

**Key Components:**
1. `cmd_init` — creates `backlog/`, writes `vat.toml`, `backlog.md`, `.used-ids`, `README.md`
2. `cmd_start` — adds `[in-progress]` and `[by:<name>]` to a bullet
3. `cmd_block` / `cmd_unblock` — adds / removes `[blocked-by:<id>]` marker
4. `cmd_done` — deletes bullet, optional item file, appends to tombstone, auto-unblocks dependents
5. `cmd_config_get` / `cmd_config_set` — reads/writes `user.name` and `project.id`
6. `readme_template` — static README rendered with project prefix at init time

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Cross-cutting (CMD-CC) | CMD-CC-001..003 | 1 | 0 | 2 |
| vat init (CMD-INIT) | CMD-INIT-001..007 | 0 | 0 | 7 |
| vat start (CMD-START) | CMD-START-001..003 | 0 | 0 | 3 |
| vat block (CMD-BLOCK) | CMD-BLOCK-001..006 | 0 | 0 | 6 |
| vat unblock (CMD-UNBLOCK) | CMD-UNBLOCK-001..002 | 0 | 0 | 2 |
| vat done (CMD-DONE) | CMD-DONE-001..005 | 0 | 0 | 5 |
| vat config (CMD-CFG) | CMD-CFG-001..006 | 0 | 0 | 6 |
| Exit codes (CMD-EXIT) | CMD-EXIT-001..003 | 0 | 0 | 3 |
| Deferred | CMD-LOCK-001, CMD-FORCE-001, CMD-DRYRUN-001, CMD-INIT-ADOPT-001 | — | 4 | — |

**Summary:** 1 of 35 active specs implemented; 4 deferred; 34 active gaps.

Active gap summary: CMD-CC-002..003, CMD-INIT-001..007, CMD-START-001..003, CMD-BLOCK-001..006, CMD-UNBLOCK-001..002, CMD-DONE-001..005, CMD-CFG-001..006, CMD-EXIT-001..003.

## Key Findings

1. **CMD-INIT-006 partial — intentional** — `readme_template.rs` implements the README template content and carries `@spec CMD-INIT-006`. The spec marker is `[ ]` with a note that it stays pending `vat init` wiring (CMD-INIT-005). This is an intentional split: the content is ready, the write path isn't. No action needed for this finding.

2. **CMD-CC-001 implemented in backlog_file** — The version-check logic (`version > supported → abort`) lives in `src/backlog_file.rs` and is annotated `@spec CMD-CC-001`. Every command that calls `backlog_file::parse()` automatically satisfies this spec. CMD-CC-002 (unknown-id abort) and CMD-CC-003 (canonical marker order) are still gaps pending `find_entry` implementation.

3. **All command bodies are stubs** — `main.rs` contains `cmd_init`, `cmd_sync`, and five other stubs, all printing "not yet implemented" and exiting 1. The library modules they need (backlog_file, tombstone, item_file, project_config, user_config) are ready.

4. **FMT-MARK dependency** — `vat start`, `vat block`, `vat unblock`, and `vat done` all require bullet marker parsing (FMT-MARK-001..007), which is a gap in the backlog-format segment. These commands are blocked until backlog-format's marker work is done.

## Work Required

### Must Fix
1. Implement `cmd_init` (CMD-INIT-001..007) — create backlog directory structure and write initial files
2. Implement `cmd_start` (CMD-START-001..003) — after FMT-MARK-* is resolved
3. Implement `cmd_block` / `cmd_unblock` (CMD-BLOCK-001..006, CMD-UNBLOCK-001..002) — after FMT-MARK-*
4. Implement `cmd_done` (CMD-DONE-001..005) — after FMT-MARK-*
5. Implement `cmd_config_get` / `cmd_config_set` (CMD-CFG-001..006)

### Should Fix
6. Implement CMD-CC-002..003 once `find_entry` helper exists
7. Verify CMD-EXIT-001..003 exit codes for each implemented command

### Deferred
- CMD-LOCK-001, CMD-FORCE-001, CMD-DRYRUN-001, CMD-INIT-ADOPT-001 (per spec)
