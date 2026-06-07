# Arrow: sync

The `vat sync` command — ID assignment, marker normalization, notes extraction, item-file pointer suffix, idempotent writes.

## Status

**PARTIAL** — last audited 2026-06-05 (see `index.yaml` for audited SHA). The sync command is a stub. Indentation-stripping utility (SYNC-NOTES-004 infrastructure) exists in `src/item_file.rs` but is not yet invoked by the sync command. All 23 active SYNC-* specs are gaps.

## References

### HLD
- docs/high-level-design.md (§ Commands — vat sync, § Key design decisions §2)

### LLD
- docs/llds/sync.md
- docs/llds/backlog-format.md (file grammar shared by sync)

### EARS
- docs/specs/sync-specs.md (24 specs: 0 implemented, 23 gaps, 1 deferred)

### Tests
- src/item_file.rs (inline `#[cfg(test)]` — strip_indent tests, @spec SYNC-NOTES-004)

### Code
- src/main.rs (sync stub — `cmd_sync()`)
- src/item_file.rs (@spec SYNC-NOTES-004 — `strip_indent()` utility)

## Architecture

**Purpose:** `vat sync` is the only command that may mutate the structure of `backlog.md`. It assigns IDs to unID'd bullets, extracts notes into item files, normalizes marker order, and maintains item-file pointer suffixes. Idempotent: two runs on stable input produce the same output.

**Key Components:**
1. `src/main.rs` — `cmd_sync()` stub
2. `src/item_file.rs` — `strip_indent()` (SYNC-NOTES-004 indentation stripping); `create()` and `append()` for item-file writes (SYNC-NOTES-002, SYNC-NOTES-003)
3. `src/base32.rs` — `random()` for ID generation (SYNC-ID-001 to SYNC-ID-003)
4. `src/tombstone.rs` — `read()` and `append()` for used-ID tracking (SYNC-ID-002, SYNC-ID-004)
5. (missing) bullet parser — SYNC needs FMT-PARSE-* and FMT-MARK-* from the backlog-format segment before it can parse bullets to assign IDs or normalize markers

## Spec Coverage

| Category | Spec IDs | Implemented | Gaps | Deferred |
|----------|----------|-------------|------|----------|
| ID assignment (SYNC-ID) | SYNC-ID-001 to SYNC-ID-006 | 0 | 6 | 0 |
| Marker normalization (SYNC-MARK) | SYNC-MARK-001 to SYNC-MARK-003 | 0 | 3 | 0 |
| Notes extraction (SYNC-NOTES) | SYNC-NOTES-001 to SYNC-NOTES-005 | 0 | 5 | 0 |
| Item-file pointer suffix (SYNC-PTR) | SYNC-PTR-001 to SYNC-PTR-003 | 0 | 3 | 0 |
| Idempotence / writes (SYNC-WRITE) | SYNC-WRITE-001 to SYNC-WRITE-004 | 0 | 4 | 0 |
| Preconditions (SYNC-PRE) | SYNC-PRE-001 to SYNC-PRE-002 | 0 | 2 | 0 |
| Deferred | SYNC-GC-001 | — | — | 1 |

**Summary:** 0 of 23 active specs implemented; 23 gaps; 1 deferred (SYNC-GC-001 orphan GC).

## Key Findings

1. **Sync command is a stub** — `src/main.rs::cmd_sync()` prints "not yet implemented" and exits 1. No sync logic exists.

2. **SYNC-NOTES-004 infrastructure is ready** — `src/item_file.rs::strip_indent()` implements the indentation-stripping algorithm (longest-common-leading-whitespace-byte-prefix, tab≠space, blank-line exclusion) and is tested. It will be called by sync's notes-extraction path once the command body is implemented.

3. **Hard dependency on backlog-format bullet parsing** *(derived from `index.yaml` dependency graph — remove when `sync.blockedBy` edge resolves)* — sync cannot proceed until FMT-PARSE-* and FMT-MARK-* are implemented in the backlog-format segment, as it needs to parse bullet lines to find unID'd bullets and normalize marker order.

4. **Infrastructure for other sub-tasks exists** — `src/base32.rs::random()` (ID generation) and `src/tombstone.rs::read()`/`append()` (used-ID tracking) are fully implemented. Once the bullet parser is available, the sync algorithm can be assembled from existing pieces.

## Work Required

### Must Fix
1. Implement bullet-line parsing in backlog-format segment first (FMT-PARSE-*, FMT-MARK-*)
2. Implement `cmd_sync()` body: ID assignment loop, marker normalization, notes extraction, item-file pointer suffix, all-or-nothing write (SYNC-ID-*, SYNC-MARK-*, SYNC-NOTES-*, SYNC-PTR-*, SYNC-WRITE-*, SYNC-PRE-*)
