# Arrow: commands

Single-entry and config commands — init, start, block, unblock, done, config get/set.

## Status

**PARTIAL** — last audited 2026-06-05 (see `index.yaml` for audited SHA). Only CMD-CC-001 (version check) is implemented; all command bodies are stubs. Infrastructure for CMD-INIT-006 (README template) exists and is tested but is not yet wired into `vat init`.

## References

### HLD
- docs/high-level-design.md (§ Commands)

### LLD
- docs/llds/commands.md
- docs/llds/backlog-format.md (shared file grammar and `find_entry` helper)

### EARS
- docs/specs/commands-specs.md (35 active specs: 1 implemented, 34 gaps, 4 deferred)

### Tests
- src/backlog_file.rs (inline `#[cfg(test)]` — CMD-CC-001 version check)
- src/readme_template.rs (inline `#[cfg(test)]` — CMD-INIT-006 README template)

### Code
- src/main.rs (command dispatch and stubs)
- src/readme_template.rs (@spec CMD-INIT-006 — BACKLOG_README_TEMPLATE and render())
- src/backlog_file.rs (@spec CMD-CC-001 — check_version())

## Architecture

**Purpose:** Implements all VAT commands other than `vat sync`. Each command is a parse → single-line mutation → write cycle on `backlog.md` and related files.

**Key Components:**
1. `src/main.rs` — command stubs (all currently `eprintln!("…not yet implemented"); exit(1)`)
2. `src/readme_template.rs` — `BACKLOG_README_TEMPLATE` and `render(prefix)` for `vat init`
3. `src/backlog_file.rs` — `check_version()` cross-cutting version guard (CMD-CC-001)

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

**Summary:** 1 of 35 active specs implemented (CMD-CC-001); 34 gaps; 4 deferred.

## Key Findings

1. **All command bodies are stubs** — `src/main.rs` dispatches to `cmd_init`, `cmd_sync`, `cmd_start`, etc., each of which prints "not yet implemented" and exits 1. No business logic exists.

2. **README template is ready (CMD-INIT-006)** — `src/readme_template.rs` implements `render(prefix)` with `{prefix}` substitution, fully tested. The `vat init` stub just needs to call it; the infrastructure is not the blocker.

3. **Version check is implemented (CMD-CC-001)** — `check_version()` in `backlog_file.rs` is implemented and tested. It needs to be called by each bullet-mutating command once those are implemented.

4. **CMD-CC-002 and CMD-CC-003 are gaps** — the `find_entry(id)` helper that returns "unknown id" errors and writes bullets in canonical marker order does not yet exist.

5. **blockedBy backlog-format** *(derived from `index.yaml` dependency graph — remove when `commands.blockedBy` edge resolves)* — bullet-mutating commands (start, block, unblock, done) depend on the `find_entry` helper, which in turn depends on bullet-line parsing (FMT-PARSE-*) and marker parsing (FMT-MARK-*) — both active gaps in the backlog-format segment.

## Work Required

### Must Fix
1. Implement `find_entry(id)` helper (CMD-CC-002, CMD-CC-003) — depends on backlog-format bullet parsing
2. Implement `vat init`: create backlog/, write vat.toml, backlog.md, .used-ids, README.md (CMD-INIT-001 to CMD-INIT-007)
3. Implement `vat start <id>` (CMD-START-001 to CMD-START-003)
4. Implement `vat block <id> <blocker-id>` (CMD-BLOCK-001 to CMD-BLOCK-006)
5. Implement `vat unblock <id>` (CMD-UNBLOCK-001 to CMD-UNBLOCK-002)
6. Implement `vat done <id>` (CMD-DONE-001 to CMD-DONE-005)
7. Implement `vat config get` and `vat config set` (CMD-CFG-001 to CMD-CFG-006)
8. Wire proper exit codes (CMD-EXIT-001 to CMD-EXIT-003)
