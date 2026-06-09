# Arrow: sync

`vat sync` — assign IDs to new bullets, extract notes, normalize marker order.

## Status

**MAPPED** — last audited 2026-06-09 (git SHA `626528d`). Command body is a stub; all 23 active SYNC specs are gaps. Supporting infrastructure (ID generation, tombstone, item-file operations) is in place. Blocked on FMT-MARK-* from `backlog-format`.

## References

### HLD
- docs/high-level-design.md (§ Commands — vat sync; § Key design decisions #1, #2, #3)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (23 active specs: 0 implemented, 23 gaps; 1 deferred)

### Tests
- None yet

### Code
- src/main.rs:98 — `cmd_sync` stub ("not yet implemented")
- src/item_file.rs — SYNC-NOTES-004 infrastructure (notes indentation stripping + item file create/append)
- src/base32.rs — ID generation primitives (needed for SYNC-ID-001)
- src/tombstone.rs — ID collision avoidance (needed for SYNC-ID-002)
- src/backlog_file.rs — parsed-region parse/serialize (needed; marker parsing from FMT-MARK-* also required)

## Architecture

**Purpose:** The only structural-mutation command. Assigns stable IDs to new bullets, extracts note lines to per-task item files, normalizes marker order. Must be idempotent: two successive runs on a stable input produce byte-identical output.

**Key Components:**
1. `src/main.rs:cmd_sync` — entry point (stub)
2. `src/item_file.rs` — item file create/append and SYNC-NOTES-004 indentation stripping
3. `src/base32.rs` — random ID generation for SYNC-ID-001
4. `src/tombstone.rs` — tombstone read/append for SYNC-ID-002 collision avoidance
5. `src/backlog_file.rs` — task-entry parse/serialize (marker parsing to be added under FMT-MARK-*)

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| ID assignment | SYNC-ID-001 to 006 | 0 | 0 | 6 |
| Marker normalization | SYNC-MARK-001 to 003 | 0 | 0 | 3 |
| Notes extraction | SYNC-NOTES-001 to 005 | 0 | 0 | 5 |
| Item-file pointer suffix | SYNC-PTR-001 to 003 | 0 | 0 | 3 |
| Idempotence / writes | SYNC-WRITE-001 to 004 | 0 | 0 | 4 |
| Preconditions | SYNC-PRE-001 to 002 | 0 | 0 | 2 |
| Deferred | SYNC-GC-001 | 0 | 1 | 0 |

**Summary:** 0 of 23 active specs implemented; 1 deferred (SYNC-GC-001 orphaned-item GC). All infrastructure primitives are in place; the wiring in `cmd_sync` is the missing piece.

## Key Findings

1. **Command entirely unimplemented** — `src/main.rs:98`: `cmd_sync` prints "not yet implemented" and exits 1. No SYNC spec is wired.

2. **Infrastructure is ready** — The primitives needed to implement sync are all present: `src/item_file.rs` (SYNC-NOTES-004 logic), `src/base32.rs` (ID generation), `src/tombstone.rs` (collision avoidance), `src/backlog_file.rs` (task-entry parse/serialize). The missing piece is marker parsing (FMT-MARK-*) in `backlog_file.rs`.

3. **Blocked on backlog-format** — SYNC-MARK-001 to 003 (marker normalization) require the marker-token parser that is the primary gap in the `backlog-format` segment. Sync cannot be implemented until FMT-MARK-001 to 007 land.

## Work Required

### Must Fix
1. Implement `cmd_sync` end-to-end once `backlog-format` delivers FMT-MARK-001 to 007. Write the full algorithm per `docs/llds/sync.md` covering SYNC-ID, SYNC-MARK, SYNC-NOTES, SYNC-PTR, SYNC-WRITE, and SYNC-PRE spec groups.
2. Add integration tests for idempotence (SYNC-WRITE-001/002) and all-or-nothing writes (SYNC-WRITE-003).
