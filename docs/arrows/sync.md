# Arrow: sync

`vat sync` — assign IDs to new bullets, extract notes, normalize marker order.

## Status

**PARTIAL** — last audited 2026-06-12 (vat-v3k, on top of `03b355d`). Notes extraction (vat-t1h), ID assignment (vat-s9g), and marker normalization + write/idempotence guarantees (vat-v3k) are implemented; `sync.rs` is wired onto `src/bullet.rs` (`Bullet::parse`/`serialize`). The only remaining gap is the item-file pointer suffix (SYNC-PTR-001..003).

## References

### HLD
- docs/high-level-design.md (§ Commands — vat sync; § Key design decisions #1, #2, #3)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (23 active specs: 20 implemented, 3 gaps; 1 deferred)

### Tests
- src/sync.rs (inline `#[cfg(test)]` — integration tests covering SYNC-NOTES-*, SYNC-PRE-*, SYNC-WRITE-001/002/003/004, SYNC-MARK-001/002/003, SYNC-ID-001/002/004/005/006, FMT-PARSE-006)
- src/id_assignment.rs (inline `#[cfg(test)]` — unit tests covering SYNC-ID-001/002/003/005/006)
- src/item_file.rs (inline `#[cfg(test)]` — SYNC-NOTES-004 indentation stripping)

### Code
- src/sync.rs — sync engine: bullet parse/serialize wiring, marker normalization, ID assignment wiring, notes extraction, preconditions, write-skip, tombstone append (SYNC-MARK-*, SYNC-ID-004, SYNC-NOTES-*, SYNC-PRE-*, SYNC-WRITE-*, FMT-PARSE-006)
- src/id_assignment.rs — ID generation/validation core (SYNC-ID-001/002/003/005/006)
- src/main.rs — `cmd_sync` dispatches to `sync::run`
- src/item_file.rs — SYNC-NOTES-004 infrastructure (notes indentation stripping + item file create/append)
- src/base32.rs — ID generation primitives (used by SYNC-ID-001)
- src/tombstone.rs — .used-ids read/append (SYNC-ID-002 collision set, SYNC-ID-004 append)
- src/backlog_file.rs — parsed-region parse/serialize
- src/bullet.rs — bullet-line tokenizer consumed by sync for marker normalization (FMT-MARK-*)

## Architecture

**Purpose:** The only structural-mutation command. Assigns stable IDs to new bullets, extracts note lines to per-task item files, normalizes marker order. Must be idempotent: two successive runs on a stable input produce byte-identical output.

**Key Components:**
1. `src/sync.rs` — sync engine: `run()` orchestrates preconditions, notes extraction, and write-skip
2. `src/main.rs:cmd_sync` — entry point; dispatches to `sync::run`
3. `src/item_file.rs` — item file create/append and SYNC-NOTES-004 indentation stripping
4. `src/base32.rs` — random ID generation for SYNC-ID-001
5. `src/tombstone.rs` — tombstone read/append for SYNC-ID-002 collision avoidance
6. `src/backlog_file.rs` — task-entry parse/serialize
7. `src/bullet.rs` — bullet-line marker tokenizer (parse/serialize), the canonicalization engine behind SYNC-MARK-*

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| ID assignment | SYNC-ID-001 to 006 | 6 | 0 | 0 |
| Marker normalization | SYNC-MARK-001 to 003 | 3 | 0 | 0 |
| Notes extraction | SYNC-NOTES-001 to 005 | 5 | 0 | 0 |
| Item-file pointer suffix | SYNC-PTR-001 to 003 | 0 | 0 | 3 |
| Idempotence / writes | SYNC-WRITE-001 to 004 | 4 | 0 | 0 |
| Preconditions | SYNC-PRE-001 to 002 | 2 | 0 | 0 |
| Deferred | SYNC-GC-001 | 0 | 1 | 0 |

**Summary:** 20 of 23 active specs implemented; 1 deferred (SYNC-GC-001 orphaned-item GC). The only remaining gap is the item-file pointer suffix (SYNC-PTR-001..003).

## Key Findings

1. **Notes extraction implemented** — `src/sync.rs` (landed via vat-t1h, PR #32) implements SYNC-NOTES-001 to 005, SYNC-PRE-001/002, and SYNC-WRITE-002/004 with inline tests. `cmd_sync` dispatches to `sync::run`.

2. **ID assignment implemented** — SYNC-ID-001 to 006 (vat-s9g, PR #20): `src/id_assignment.rs` holds the generation/validation core; `sync::run` seeds the collision set from `.used-ids` plus existing region IDs, splices new `[id]` markers in at the front of unid'd bullets, and appends new IDs to `.used-ids` only after a successful `backlog.md` write (SYNC-ID-004).

3. **Marker normalization implemented** — SYNC-MARK-001 to 003 and full idempotence (SYNC-WRITE-001) plus all-or-nothing writes (SYNC-WRITE-003) landed via vat-v3k: `sync::run` parses every bullet with `Bullet::parse` and re-emits it via `Bullet::serialize`. Bullet identity now follows the front-loaded parser — an ID-shaped token behind an unknown bracketed token is title text, not the bullet's ID (the interim anywhere-scan `extract_id` was deleted; see the LLD's Decisions section).

4. **FMT-PARSE-006 wired** — Title-less bullets warn and pass through verbatim (line and note lines preserved), skipped for ID assignment and notes extraction. Malformed bullets are fully inert: their ID-shaped tokens do not seed collision avoidance or duplicate detection.

## Work Required

### Must Fix
1. Implement item-file pointer suffixes (SYNC-PTR-001 to 003) — the segment's last active gap. `Bullet.title` manipulation via `src/bullet.rs` is now available.
