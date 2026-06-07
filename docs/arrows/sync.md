# Arrow: sync

The `vat sync` command — ID assignment, notes extraction, marker normalization, and all-or-nothing write to `backlog.md`.

## Status

**MAPPED** — last audited 2026-06-07 (git SHA `fe4be98`). No implementation yet; all 23 active specs are gaps. Blocked on FMT-MARK-* in the `backlog-format` segment (marker normalization requires a parser/serializer).

## References

### HLD
- docs/high-level-design.md (§ Commands — vat sync, § Key design decisions §1–§5)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (23 active specs; 1 deferred)

### Tests
- (none yet)

### Code
- src/main.rs:97–100 — `cmd_sync()` stub (prints "not yet implemented", exits 1)

## Architecture

**Purpose:** The only command that mutates the *structure* of `backlog.md`. Reads, parses, assigns IDs, extracts notes, normalizes markers, and writes atomically. Idempotent on stable input.

**Key Components:**
1. `cmd_sync` (to be implemented — `src/main.rs` or a new `src/sync.rs`) — orchestrates the full sync pipeline
2. ID generation loop — draws from `base32::random`, retries against `.used-ids` union in-file set; cap at 100 (backlog-format module)
3. Notes extractor — calls `item_file::create` or `item_file::append`; indentation-strip already implemented as `SYNC-NOTES-004` in `src/item_file.rs`
4. Marker normalizer — depends on FMT-MARK-* (bullet marker parser/serializer) in backlog-format segment
5. Write gate — compares serialized output to input bytes; skips write if identical (SYNC-WRITE-002)

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| ID assignment | SYNC-ID-001 to SYNC-ID-006 | 0 | 0 | 6 |
| Marker normalization | SYNC-MARK-001 to SYNC-MARK-003 | 0 | 0 | 3 |
| Notes extraction | SYNC-NOTES-001 to SYNC-NOTES-005 | 0 | 0 | 5 |
| Item-file pointer suffix | SYNC-PTR-001 to SYNC-PTR-003 | 0 | 0 | 3 |
| Idempotence / writes | SYNC-WRITE-001 to SYNC-WRITE-004 | 0 | 0 | 4 |
| Preconditions | SYNC-PRE-001 to SYNC-PRE-002 | 0 | 0 | 2 |
| Out of scope | SYNC-GC-001 | 0 | 1 | 0 |

**Summary:** 0 of 23 active specs implemented; 1 deferred; 23 gaps.

## Key Findings

1. **Blocked on FMT-MARK-* in backlog-format** — `vat sync` cannot normalize markers (SYNC-MARK-001) until FMT-MARK-001..007 (bullet marker parsing/serialization) are implemented in the `backlog-format` segment.
2. **SYNC-NOTES-004 already implemented** — `src/item_file.rs` implements the indentation-stripping algorithm with `@spec SYNC-NOTES-004` annotations throughout. This primitive is ready to be called from sync.
3. **No sync.rs module yet** — `cmd_sync()` in `src/main.rs:97–100` is a plain stub. The sync pipeline should be implemented in `src/main.rs` (for a small impl) or extracted to `src/sync.rs`.

## Work Required

### Must Fix
1. Implement FMT-MARK-001..007 in `backlog-format` segment first (hard dependency)
2. Implement `cmd_sync` — all 23 SYNC-* specs are gaps; see `docs/llds/sync.md` for the full algorithm
