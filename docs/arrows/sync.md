# Arrow: sync

`vat sync` — assign IDs to new bullets, extract notes, normalize marker order.

## Status

**OK** — re-verified 2026-10-17 (HEAD `9a79ce5`); previously 2026-10-14 (HEAD `eb38c23`, vat-ccg); previously 2026-10-12 (HEAD `f6e247e`, vat-ccg); previously 2026-10-10 (HEAD `c265c95`). Notes extraction (vat-t1h), ID assignment (vat-s9g), marker normalization + write/idempotence guarantees (vat-v3k), item-file pointer suffix (vat-mzd, PR #61), and the new-bullet title-length check (vat-ccg) are all implemented. All 33 active specs are satisfied; 1 deferred (SYNC-GC-001).

## References

### HLD
- docs/high-level-design.md (§ Commands — vat sync; § Key design decisions #1, #2, #3)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (33 active specs: 33 implemented, 0 gaps; 1 deferred)

### Tests
- src/sync.rs (inline `#[cfg(test)]` — integration tests covering SYNC-NOTES-*, SYNC-PRE-*, SYNC-WRITE-001/002/003/004, SYNC-MARK-001/002/003/004, SYNC-ID-001/002/004/005/006, SYNC-PTR-001/002/003, FMT-PARSE-006)
- src/id_assignment.rs (inline `#[cfg(test)]` — unit tests covering SYNC-ID-001/002/003/005/006)
- src/item_file.rs (inline `#[cfg(test)]` — SYNC-NOTES-004 indentation stripping)
- src/title_length.rs (inline `#[cfg(test)]` — SYNC-LEN-001 measured-title rule)
- src/sync.rs (inline `#[cfg(test)]` — SYNC-LEN-002..006, 009)
- tests/sync_title_length.rs (binary-driven — SYNC-LEN-002/006/007/008, FMT-CFG-005, CMD-CFG-007/009)
- tests/sync_golden.rs (fixture-directory golden tests driving the real `vat sync` binary; pins SYNC-ID-001/004, SYNC-WRITE-001/002, SYNC-NOTES-001/002/003/005, SYNC-MARK-003, SYNC-PTR-001/003 against `tests/fixtures/sync/<case>/{input,expected}` trees)

### Code
- src/sync.rs — sync engine: bullet parse/serialize wiring, marker normalization, ID assignment wiring, notes extraction, item-file pointer suffix, preconditions, write-skip, tombstone append (SYNC-MARK-*, SYNC-ID-004, SYNC-NOTES-*, SYNC-PTR-001..003, SYNC-PRE-*, SYNC-WRITE-*, FMT-PARSE-006)
- src/id_assignment.rs — ID generation/validation core (SYNC-ID-001/002/003/005/006)
- src/title_length.rs — measured-title rule and default limit (SYNC-LEN-001); `sync::title_offenders` applies it (SYNC-LEN-002/003/005/006)
- src/main.rs — `cmd_sync` dispatches to `sync::run`; `TitleTooLong` → exit 1 (SYNC-LEN-007)
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
| Marker normalization | SYNC-MARK-001 to 004 | 4 | 0 | 0 |
| Notes extraction | SYNC-NOTES-001 to 005 | 5 | 0 | 0 |
| Item-file pointer suffix | SYNC-PTR-001 to 003 | 3 | 0 | 0 |
| Idempotence / writes | SYNC-WRITE-001 to 004 | 4 | 0 | 0 |
| Preconditions | SYNC-PRE-001 to 002 | 2 | 0 | 0 |
| Title length | SYNC-LEN-001 to 009 | 9 | 0 | 0 |
| Deferred | SYNC-GC-001 | 0 | 1 | 0 |

**Summary:** 33 of 33 active specs implemented; 1 deferred (SYNC-GC-001 orphaned-item GC); 0 gaps.

## Key Findings

1. **Notes extraction implemented** — `src/sync.rs` (landed via vat-t1h, PR #32) implements SYNC-NOTES-001 to 005, SYNC-PRE-001/002, and SYNC-WRITE-002/004 with inline tests. `cmd_sync` dispatches to `sync::run`.

2. **ID assignment implemented** — SYNC-ID-001 to 006 (vat-s9g, PR #20): `src/id_assignment.rs` holds the generation/validation core; `sync::run` seeds the collision set from `.used-ids` plus existing region IDs, splices new `[id]` markers in at the front of unid'd bullets, and appends new IDs to `.used-ids` only after a successful `backlog.md` write (SYNC-ID-004).

3. **Marker normalization implemented** — SYNC-MARK-001 to 004 and full idempotence (SYNC-WRITE-001) plus all-or-nothing writes (SYNC-WRITE-003) landed via vat-v3k: `sync::run` parses every bullet with `Bullet::parse` and re-emits it via `Bullet::serialize`. Bullet identity now follows the front-loaded parser — an ID-shaped token behind an unknown bracketed token is title text, not the bullet's ID (the interim anywhere-scan `extract_id` was deleted; see the LLD's Decisions section). SYNC-MARK-004: when re-serialization would drop a second `[blocked-by:...]` (FMT-MARK-007), sync warns per dropped target ID via `Bullet::parse_reporting_dropped`, so multi-blocker lines aren't silently truncated on first sync.

4. **FMT-PARSE-006 wired** — Title-less bullets warn and pass through verbatim (line and note lines preserved), skipped for ID assignment and notes extraction. Malformed bullets are fully inert: their ID-shaped tokens do not seed collision avoidance or duplicate detection.

5. **Item-file pointer suffix implemented** — SYNC-PTR-001 to 003 (vat-mzd, PR #61): `sync.rs` pre-scans `backlog/items/` into a `HashSet<OsString>` (one `read_dir` per run), then after notes extraction calls `apply_pointer_suffix` to ensure each bullet's title ends with ` (see ./items/<id>.md)` when an item file exists. Idempotent (SYNC-PTR-003): suffix only appended when absent. Non-destructive (SYNC-PTR-002): bullets with no item file are unchanged. `Bullet::bare_title()` in `src/bullet.rs` strips the suffix for display/search/export contexts.

6. **Title-length check implemented** — SYNC-LEN-001 to 009 (vat-ccg): after bullet parsing and before ID assignment, `sync::title_offenders` measures every new (un-ID'd, well-formed) bullet with `title_length::measured_title` — raw line minus the `-`, every `[...]` span, and a trailing `(see ./items/…md)` pointer, trimmed, counted in chars — and aborts with `SyncError::TitleTooLong` listing every offender when any exceeds `[sync].max_title_length` (FMT-CFG-004, default 120, `0` disables). Dropped-`[blocked-by:]` warnings are withheld on that abort (SYNC-LEN-008). The rule is deliberately independent of the marker parser so the skill's prose procedure counts identically.

## Work Required

None — all active specs implemented.
