# Arrow: sync

`vat sync` command — ID assignment, marker normalization, notes extraction, item-file pointer management, and all-or-nothing write semantics.

## Status

**PARTIAL** — last audited 2026-06-08 (git SHA `e2c7ad8cf75a7da4a970a1eedfb8b6e5784d4c14`). All underlying library modules (`backlog_file`, `item_file`, `tombstone`, `base32`) are implemented and annotated. The `cmd_sync` body in `main.rs` is a stub ("not yet implemented"). All 23 active SYNC-* specs are active gaps; 1 is deferred.

## References

### HLD
- docs/high-level-design.md (§ Commands — `vat sync` row, § Key design decisions §2 §3 §4)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (24 specs: 0 implemented, 23 active gaps, 1 deferred)

### Tests
- None yet (command not implemented)

### Code
- src/main.rs — `cmd_sync` stub
- src/backlog_file.rs — parse/serialize called by sync (`@spec FMT-FM-*`, `FMT-RGN-*`, `FMT-PARSE-*`)
- src/item_file.rs — notes extraction and item file create/append (`@spec SYNC-NOTES-004`, `FMT-ITEM-001..003`)
- src/tombstone.rs — `.used-ids` read/write (`@spec FMT-TOMB-*`)
- src/base32.rs — random ID generation (`@spec FMT-B32-006..007`)

## Architecture

**Purpose:** `vat sync` is the only structural-mutation command. It assigns IDs to untagged bullets, extracts notes into item files, normalizes marker order, and writes back atomically (all-or-nothing). All constituent library modules are already built; sync wires them into a pipeline.

**Key Components:**
1. Parse phase — `backlog_file::parse()` → preamble + task entries
2. ID assignment loop — generate Crockford random suffix, retry against tombstone + live IDs (cap 100)
3. Marker normalization — canonical order per FMT-MARK-004 (awaits FMT-MARK implementation)
4. Notes extraction — `item_file::create()` or `item_file::append()` per entry
5. Item-file pointer suffix — append ` (see ./items/<id>.md)` when item file exists
6. Write phase — serialize back to `backlog.md` (skip write if byte-identical), append to tombstone

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| ID assignment (SYNC-ID) | SYNC-ID-001..006 | 0 | 0 | 6 |
| Marker normalization (SYNC-MARK) | SYNC-MARK-001..003 | 0 | 0 | 3 |
| Notes extraction (SYNC-NOTES) | SYNC-NOTES-001..005 | 0 | 0 | 5 |
| Item-file pointer (SYNC-PTR) | SYNC-PTR-001..003 | 0 | 0 | 3 |
| Idempotence / writes (SYNC-WRITE) | SYNC-WRITE-001..004 | 0 | 0 | 4 |
| Preconditions (SYNC-PRE) | SYNC-PRE-001..002 | 0 | 0 | 2 |
| Deferred | SYNC-GC-001 | — | 1 | — |

**Summary:** 0 of 23 active specs implemented; 1 deferred; 23 active gaps.

## Key Findings

1. **Library modules are ready; only the wiring is missing** — `backlog_file`, `item_file`, `tombstone`, and `base32` are all implemented with `@spec` annotations and inline tests. `vat sync` is blocked only on writing `cmd_sync` to call them in the correct order (per sync LLD algorithm).

2. **SYNC-MARK-001 depends on FMT-MARK** — Marker normalization (canonical order) requires the FMT-MARK-001..007 marker parsing/serialization work in the backlog-format segment. This is the only hard blocker from another segment; the rest of sync can be implemented in parallel.

3. **All-or-nothing write semantics** — SYNC-WRITE-003 requires no files to be written on any parse/generation error. The sync LLD specifies that all writes happen at the end. This is a design invariant the implementation must respect.

4. **SYNC-NOTES-004 already implemented** — Indentation stripping logic lives in `item_file.rs` and is fully annotated and tested. The sync command just needs to call `item_file::create_or_append()`.

## Work Required

### Must Fix
1. Implement `cmd_sync` wiring the full algorithm from sync LLD (all SYNC-* specs)
2. Depends on FMT-MARK-001..007 (backlog-format segment) for SYNC-MARK-001

### Should Fix
3. Add integration tests for sync idempotence (SYNC-WRITE-001..002) once the command is implemented

### Deferred
- SYNC-GC-001 — orphan item file garbage collection (`vat sync --gc`)
