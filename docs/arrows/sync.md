# Arrow: sync

`vat sync` — assign IDs to new bullets, extract notes, normalize marker order.

## Status

**PARTIAL** — last audited 2026-06-10 (git SHA `17e8914`). Notes extraction, preconditions, and write-skip behavior are implemented (`src/sync.rs`, landed via vat-t1h). ID assignment (vat-s9g, PR #20 in flight), pointer suffixes, and marker normalization remain gaps; marker normalization is blocked on FMT-MARK-* from `backlog-format`.

<!-- NOTE: PR #20 (SYNC-ID-001/002/003/005/006) in flight — update counts when merged. -->

## References

### HLD
- docs/high-level-design.md (§ Commands — vat sync; § Key design decisions #1, #2, #3)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (23 active specs: 9 implemented, 14 gaps; 1 deferred)

### Tests
- src/sync.rs (inline `#[cfg(test)]` — 25 tests covering SYNC-NOTES-*, SYNC-PRE-*, SYNC-WRITE-001/002/004)
- src/item_file.rs (inline `#[cfg(test)]` — SYNC-NOTES-004 indentation stripping)

### Code
- src/sync.rs — sync engine: notes extraction, preconditions, write-skip (SYNC-NOTES-*, SYNC-PRE-*, SYNC-WRITE-002/004)
- src/main.rs:99 — `cmd_sync` dispatches to `sync::run` with a partial-implementation warning (ID assignment not wired, vat-s9g)
- src/item_file.rs — SYNC-NOTES-004 infrastructure (notes indentation stripping + item file create/append)
- src/base32.rs — ID generation primitives (needed for SYNC-ID-001)
- src/tombstone.rs — ID collision avoidance (needed for SYNC-ID-002)
- src/backlog_file.rs — parsed-region parse/serialize (marker parsing from FMT-MARK-* still required)

## Architecture

**Purpose:** The only structural-mutation command. Assigns stable IDs to new bullets, extracts note lines to per-task item files, normalizes marker order. Must be idempotent: two successive runs on a stable input produce byte-identical output.

**Key Components:**
1. `src/sync.rs` — sync engine: `run()` orchestrates preconditions, notes extraction, and write-skip
2. `src/main.rs:cmd_sync` — entry point; dispatches to `sync::run` (emits partial-implementation warning)
3. `src/item_file.rs` — item file create/append and SYNC-NOTES-004 indentation stripping
4. `src/base32.rs` — random ID generation for SYNC-ID-001
5. `src/tombstone.rs` — tombstone read/append for SYNC-ID-002 collision avoidance
6. `src/backlog_file.rs` — task-entry parse/serialize (marker parsing to be added under FMT-MARK-*)

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| ID assignment | SYNC-ID-001 to 006 | 0 | 0 | 6 |
| Marker normalization | SYNC-MARK-001 to 003 | 0 | 0 | 3 |
| Notes extraction | SYNC-NOTES-001 to 005 | 5 | 0 | 0 |
| Item-file pointer suffix | SYNC-PTR-001 to 003 | 0 | 0 | 3 |
| Idempotence / writes | SYNC-WRITE-001 to 004 | 2 | 0 | 2 |
| Preconditions | SYNC-PRE-001 to 002 | 2 | 0 | 0 |
| Deferred | SYNC-GC-001 | 0 | 1 | 0 |

**Summary:** 9 of 23 active specs implemented; 1 deferred (SYNC-GC-001 orphaned-item GC). Notes extraction (vat-t1h) landed the engine; remaining gaps are ID assignment (vat-s9g, PR #20 in flight), pointer suffixes, and marker normalization.

## Key Findings

1. **Notes extraction implemented** — `src/sync.rs` (landed via vat-t1h, PR #32) implements SYNC-NOTES-001 to 005, SYNC-PRE-001/002, and SYNC-WRITE-002/004 with 25 inline tests. `cmd_sync` (`src/main.rs:99`) dispatches to `sync::run` and warns that the command is partial.

2. **ID assignment not wired** — SYNC-ID-001 to 006 remain gaps (tracked as vat-s9g; PR #20 in flight covers SYNC-ID-001/002/003/005/006). The primitives (`src/base32.rs`, `src/tombstone.rs`) are ready.

3. **SYNC-WRITE-001 drift signal** — `src/sync.rs` carries a `@spec SYNC-WRITE-001` test annotation (`run_is_idempotent`), but the spec marker is still `[ ]`. Full idempotence per the spec ("all bullets ID'd, canonical marker order") can't hold until ID assignment and marker normalization land; the test covers the implemented subset only.

4. **Blocked on backlog-format** — SYNC-MARK-001 to 003 (marker normalization) require the marker-token parser that is the primary gap in the `backlog-format` segment. SYNC-PTR-* also needs bullet-title manipulation from the same parser.

## Work Required

### Must Fix
1. Wire ID assignment (SYNC-ID-001 to 006) into `sync::run` — vat-s9g; PR #20 in flight.
2. Implement marker normalization (SYNC-MARK-001 to 003) and pointer suffixes (SYNC-PTR-001 to 003) once `backlog-format` delivers FMT-MARK-001 to 007.
3. Implement SYNC-WRITE-003 (all-or-nothing writes on error) and flip SYNC-WRITE-001 once full idempotence holds.
