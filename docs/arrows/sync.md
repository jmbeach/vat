# Arrow: sync

The `vat sync` command: ID assignment, marker normalization, notes extraction, item-file pointer management.

## Status

**PARTIAL** — last audited 2026-06-04 (git SHA `426964053f024c0e1380a365543da31798536bb7`). All 23 active SYNC-* specs are gaps; the command handler in src/main.rs is a stub. Supporting infrastructure (notes indentation stripping — SYNC-NOTES-004) is implemented in item_file.rs but not yet wired into cmd_sync().

## References

### HLD
- docs/high-level-design.md (§ Commands, § Key design decisions §1–3)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (24 specs: 0 implemented, 23 gaps, 1 deferred)

### Tests
- src/item_file.rs (inline `#[cfg(test)]` — SYNC-NOTES-004)

### Code
- src/main.rs:97–100 — cmd_sync() stub (not yet implemented)
- src/item_file.rs — SYNC-NOTES-004 indentation stripping (supporting module, not yet called from sync)

## Architecture

**Purpose:** The only VAT command that mutates the structure of `backlog.md` — assigns IDs, extracts notes to item files, normalizes marker order. Idempotent by design.

**Key Components:**
1. ID assignment — reads .used-ids, generates Crockford base32 suffix, retries on collision (up to 100)
2. Marker normalization — canonical order per FMT-MARK-004 (owned by backlog-format segment)
3. Notes extraction — strips indentation, creates/appends to items/<id>.md (SYNC-NOTES-004)
4. Item-file pointer — appends ` (see ./items/<id>.md)` to bullet title when file exists
5. All-or-nothing write — no file is touched until all parsing and ID generation succeeds

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| ID assignment | SYNC-ID-001 to SYNC-ID-006 | 0 | 0 | 6 |
| Marker normalization | SYNC-MARK-001 to SYNC-MARK-003 | 0 | 0 | 3 |
| Notes extraction | SYNC-NOTES-001 to SYNC-NOTES-005 | 0 | 0 | 5 |
| Item-file pointer | SYNC-PTR-001 to SYNC-PTR-003 | 0 | 0 | 3 |
| Idempotence/writes | SYNC-WRITE-001 to SYNC-WRITE-004 | 0 | 0 | 4 |
| Preconditions | SYNC-PRE-001 to SYNC-PRE-002 | 0 | 0 | 2 |
| Garbage collection | SYNC-GC-001 | 0 | 1 | 0 |

**Summary:** 0 of 23 active specs implemented; 1 deferred; 23 gaps.

## Key Findings

1. **Command handler is a stub** — src/main.rs:97–100 emits "vat sync: not yet implemented" and exits 1. No sync logic exists above the supporting-module level.
2. **SYNC-NOTES-004 drift** — The notes indentation-stripping algorithm is fully implemented and tested in `src/item_file.rs`, and annotated `@spec SYNC-NOTES-004`, but the spec is still `[ ]`. The spec marker should be updated to `[x]` once the supporting module is confirmed as the canonical implementation location — requires user decision (see below).
3. **Hard blocker** — sync cannot be implemented until FMT-PARSE-001–006 and FMT-MARK-001–007 are done (see `backlog-format` segment). The `blockedBy: [backlog-format]` dependency applies.
4. **No integration tests** — Only inline unit tests exist (in item_file.rs for SYNC-NOTES-004). An end-to-end sync test against a sample backlog.md file would cover the integration path.

## Work Required

### Must Fix
1. Implement cmd_sync() in src/main.rs — wires all SYNC-* specs; depends on FMT-PARSE and FMT-MARK being done first
2. Update SYNC-NOTES-004 spec marker: once cmd_sync() calls item_file's stripping logic, confirm and mark `[x]`

### Should Fix
3. Add integration test for cmd_sync() covering the core sync path (ID assignment + notes extraction + idempotence)
