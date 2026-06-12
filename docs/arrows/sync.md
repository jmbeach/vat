# Arrow: sync

`vat sync` — assign IDs to new bullets, extract notes, normalize marker order.

## Status

**PARTIAL** — last audited 2026-06-12 (git SHA `9dbd445`); counts updated 2026-06-10 by vat-s9g (PR #20). Notes extraction, preconditions, write-skip behavior (vat-t1h), and ID assignment (vat-s9g) are implemented. Pointer suffixes and marker normalization remain gaps; marker normalization is blocked on FMT-MARK-* from `backlog-format`.

## References

### HLD
- docs/high-level-design.md (§ Commands — vat sync; § Key design decisions #1, #2, #3)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (23 active specs: 15 implemented, 8 gaps; 1 deferred)

### Tests
- src/sync.rs (inline `#[cfg(test)]` — integration tests covering SYNC-NOTES-*, SYNC-PRE-*, SYNC-WRITE-001/002/004, SYNC-ID-001/002/004/005/006)
- src/id_assignment.rs (inline `#[cfg(test)]` — unit tests covering SYNC-ID-001/002/003/005/006)
- src/item_file.rs (inline `#[cfg(test)]` — SYNC-NOTES-004 indentation stripping)

### Code
- src/sync.rs — sync engine: ID assignment wiring, notes extraction, preconditions, write-skip, tombstone append (SYNC-ID-004, SYNC-NOTES-*, SYNC-PRE-*, SYNC-WRITE-002/004)
- src/id_assignment.rs — ID generation/validation core (SYNC-ID-001/002/003/005/006)
- src/main.rs — `cmd_sync` dispatches to `sync::run`
- src/item_file.rs — SYNC-NOTES-004 infrastructure (notes indentation stripping + item file create/append)
- src/base32.rs — ID generation primitives (used by SYNC-ID-001)
- src/tombstone.rs — .used-ids read/append (SYNC-ID-002 collision set, SYNC-ID-004 append)
- src/backlog_file.rs — parsed-region parse/serialize (marker parsing from FMT-MARK-* still required)

## Architecture

**Purpose:** The only structural-mutation command. Assigns stable IDs to new bullets, extracts note lines to per-task item files, normalizes marker order. Must be idempotent: two successive runs on a stable input produce byte-identical output.

**Key Components:**
1. `src/sync.rs` — sync engine: `run()` orchestrates preconditions, notes extraction, and write-skip
2. `src/main.rs:cmd_sync` — entry point; dispatches to `sync::run`
3. `src/item_file.rs` — item file create/append and SYNC-NOTES-004 indentation stripping
4. `src/base32.rs` — random ID generation for SYNC-ID-001
5. `src/tombstone.rs` — tombstone read/append for SYNC-ID-002 collision avoidance
6. `src/backlog_file.rs` — task-entry parse/serialize (marker parsing to be added under FMT-MARK-*)

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| ID assignment | SYNC-ID-001 to 006 | 6 | 0 | 0 |
| Marker normalization | SYNC-MARK-001 to 003 | 0 | 0 | 3 |
| Notes extraction | SYNC-NOTES-001 to 005 | 5 | 0 | 0 |
| Item-file pointer suffix | SYNC-PTR-001 to 003 | 0 | 0 | 3 |
| Idempotence / writes | SYNC-WRITE-001 to 004 | 2 | 0 | 2 |
| Preconditions | SYNC-PRE-001 to 002 | 2 | 0 | 0 |
| Deferred | SYNC-GC-001 | 0 | 1 | 0 |

**Summary:** 15 of 23 active specs implemented; 1 deferred (SYNC-GC-001 orphaned-item GC). Notes extraction (vat-t1h) landed the engine and ID assignment (vat-s9g) is wired into it; remaining gaps are pointer suffixes and marker normalization.

## Key Findings

1. **Notes extraction implemented** — `src/sync.rs` (landed via vat-t1h, PR #32) implements SYNC-NOTES-001 to 005, SYNC-PRE-001/002, and SYNC-WRITE-002/004 with inline tests. `cmd_sync` dispatches to `sync::run`.

2. **ID assignment implemented** — SYNC-ID-001 to 006 (vat-s9g, PR #20): `src/id_assignment.rs` holds the generation/validation core; `sync::run` seeds the collision set from `.used-ids` plus existing region IDs, splices new `[id]` markers in at the front of unid'd bullets, and appends new IDs to `.used-ids` only after a successful `backlog.md` write (SYNC-ID-004).

3. **SYNC-WRITE-001 drift signal** — `src/sync.rs` carries a `@spec SYNC-WRITE-001` test annotation (`run_is_idempotent`), but the spec marker is still `[ ]`. Full idempotence per the spec ("all bullets ID'd, canonical marker order") can't hold until ID assignment and marker normalization land; the test covers the implemented subset only.

4. **Blocked on backlog-format** — SYNC-MARK-001 to 003 (marker normalization) require the marker-token parser that is the primary gap in the `backlog-format` segment. SYNC-PTR-* also needs bullet-title manipulation from the same parser.

## Work Required

### Must Fix
1. Implement marker normalization (SYNC-MARK-001 to 003) and pointer suffixes (SYNC-PTR-001 to 003) once `backlog-format` delivers FMT-MARK-001 to 007.
2. Implement SYNC-WRITE-003 (all-or-nothing writes on error) and flip SYNC-WRITE-001 once full idempotence holds.
