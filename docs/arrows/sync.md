# Arrow: sync

The `vat sync` command — the only structurally-mutating command: assigns IDs, extracts notes, normalizes markers.

## Status

**PARTIAL** — last audited 2026-06-06 (git SHA `426964053f024c0e1380a365543da31798536bb7`). All supporting format infrastructure is implemented (base32, tombstone, item files, config). The sync algorithm itself is not implemented — `cmd_sync()` in `main.rs` is a stub. All 23 active specs are gaps.

## References

### HLD
- docs/high-level-design.md (§ Commands — vat sync row, § Key design decisions §2, §3)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (23 active specs, 1 deferred)

### Tests
- src/item_file.rs (inline tests cover SYNC-NOTES-004 — indentation stripping)

### Code
- src/main.rs (`cmd_sync()` stub)
- src/item_file.rs (`@spec SYNC-NOTES-004` — notes extraction and indentation stripping, implemented)
- src/base32.rs (`@spec FMT-B32-*` — ID generation, implemented)
- src/tombstone.rs (`@spec FMT-TOMB-*` — used-ids read/append, implemented)

## Architecture

**Purpose:** Idempotent structural mutation of `backlog.md` — assign IDs to new bullets, extract notes into item files, normalize marker order.

**Key Components:**
1. `cmd_sync` in `main.rs` — top-level orchestrator (stub)
2. `backlog_file.rs` — parse backlog (frontmatter, region split, bullet grammar once FMT-PARSE-* / FMT-MARK-* land)
3. `base32.rs` — random ID generation with retry loop
4. `tombstone.rs` — read used-ids set, append new IDs
5. `item_file.rs` — create or append notes to item files

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

**Summary:** 0 of 23 active specs implemented; 23 gaps; 1 deferred.

**Note:** SYNC-NOTES-004 (indentation stripping) is fully implemented in `src/item_file.rs` with comprehensive tests, but the SYNC-NOTES spec as a whole is `[ ]` because the calling `cmd_sync` orchestrator is not yet wired.

## Key Findings

1. **All SYNC-NOTES-004 infrastructure is done** — `item_file.rs` implements the indentation-stripping algorithm with ~18 test cases; this is the complex part of notes extraction. The remaining SYNC-NOTES work is wiring it from `cmd_sync`.
2. **All ID-generation infrastructure is done** — `base32.rs` generates random Crockford IDs; `tombstone.rs` reads/writes `.used-ids`. The 100-retry collision loop belongs in `cmd_sync` (per sync LLD).
3. **Blocked by backlog-format parsed-region grammar** — The sync algorithm requires FMT-PARSE-* and FMT-MARK-* to be implemented first so it can parse bullets and normalize markers. See `backlog-format` segment.

## Work Required

### Must Fix
1. Implement `cmd_sync` orchestrator in `main.rs` per the algorithm in `docs/llds/sync.md` — this is the primary work item; all dependencies are implemented
2. Note: requires FMT-PARSE-* and FMT-MARK-* from `backlog-format` segment first
