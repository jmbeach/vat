# Arrow: sync

`vat sync` — the structural-mutation command. Scans `backlog.md`, assigns IDs to new bullets, extracts notes into per-item files, normalizes marker order, and appends new IDs to `.used-ids`. The only command that may rewrite the structure of `backlog.md`.

## Status

**MAPPED** — last audited 2026-06-03 (git SHA `52bbfb58a6f7f999969da68bef55b38bd59fb744`). LLD and EARS spec file exist and are detailed. `cmd_sync()` in `main.rs` is a stub. Notes indentation-stripping logic (SYNC-NOTES-004) is implemented in `src/item_file.rs` but not yet called from a sync pipeline. All other 22 SYNC-* active specs are unimplemented. Blocked on `backlog-format` completing task-entry parsing (FMT-PARSE-*) and marker normalization (FMT-MARK-*).

## References

### HLD
- docs/high-level-design.md (§ Commands — `vat sync` row)

### LLD
- docs/llds/sync.md

### EARS
- docs/specs/sync-specs.md (24 specs: 0 implemented, 23 active gaps, 1 deferred)

### Tests
- src/item_file.rs (inline `#[test]` module — covers SYNC-NOTES-004 indentation stripping)

### Code
- src/main.rs — cmd_sync() stub
- src/item_file.rs — notes indentation stripping + item-file create/append (`@spec` SYNC-NOTES-004, FMT-ITEM-*)

## Architecture

**Purpose:** The only structural-mutation command. Idempotent: running twice on a stable file produces byte-identical output.

**Key Components:**
1. Version check + file load — reads `backlog.md`, parses frontmatter, splits into parsed/freeform regions
2. ID assignment loop — for each bullet without `[id]`, generates a random Crockford base32 suffix, retries against `.used-ids` union live IDs (≤100 tries)
3. Marker normalization — rewrites each bullet in canonical marker order
4. Notes extraction — strips indentation and moves notes to `backlog/items/<id>.md` (create or append); logic in `item_file.rs`
5. Item-file pointer suffix — ensures bullet title ends with ` (see ./items/<id>.md)` when item file exists
6. All-or-nothing write — only writes after all parsing and ID generation succeed
7. Tombstone append — writes new IDs to `.used-ids` after successful `backlog.md` write

## Spec Coverage

| Category | Spec IDs | Total | Implemented | Deferred | Gaps |
|----------|----------|-------|-------------|----------|------|
| ID assignment | SYNC-ID-001..006 | 6 | 0 | 0 | 6 |
| Marker normalization | SYNC-MARK-001..003 | 3 | 0 | 0 | 3 |
| Notes extraction | SYNC-NOTES-001..005 | 5 | 0 | 0 | 5 |
| Item-file pointer suffix | SYNC-PTR-001..003 | 3 | 0 | 0 | 3 |
| Idempotence / writes | SYNC-WRITE-001..004 | 4 | 0 | 0 | 4 |
| Preconditions | SYNC-PRE-001..002 | 2 | 0 | 0 | 2 |
| Deferred | SYNC-GC-001 | 1 | 0 | 1 | 0 |
| **Total** | | **24** | **0** | **1** | **23** |

**Summary:** 0 of 23 active specs implemented; 1 deferred. SYNC-NOTES-004 logic exists in `item_file.rs` but is not called from a sync pipeline yet and the spec is still marked `[ ]`.

## Key Findings

1. **SYNC-NOTES-004 implementation drift** — `src/item_file.rs` carries `// @spec SYNC-NOTES-004` and implements the notes indentation-stripping algorithm, but `sync-specs.md` still marks SYNC-NOTES-004 as `[ ]`. Once `cmd_sync()` calls the `item_file` functions, SYNC-NOTES-004 should be marked `[x]`. Decision required (see structured findings report).

2. **cmd_sync() is a complete stub** — The function body is `eprintln!("vat sync: not yet implemented"); std::process::exit(1)`. None of the sync algorithm steps are started.

3. **Cross-segment dependency on item_file.rs** — `item_file.rs` is also part of the backlog-format segment (FMT-ITEM-*). The notes-extraction behavior bridges both segments. When implementing sync, the item-file module is already present and tested; sync wires it into the pipeline.

4. **Idempotence is a key invariant** — SYNC-WRITE-001 and SYNC-WRITE-002 require byte-identical output on a second run and skipping the write when unchanged. These should be verified with integration-style tests once the algorithm is implemented.

## Work Required

### Must Fix
1. Implement `cmd_sync()` body following the algorithm in `docs/llds/sync.md`
2. Wire `item_file.rs` into the sync pipeline (SYNC-NOTES-001..005)
3. Implement ID assignment loop with tombstone + live-set collision avoidance (SYNC-ID-001..006)
4. Implement marker normalization pass (SYNC-MARK-001..003) — depends on FMT-MARK-* in backlog-format
5. Implement item-file pointer suffix logic (SYNC-PTR-001..003)
6. Implement all-or-nothing write semantics (SYNC-WRITE-003)
7. Implement no-op write skip (SYNC-WRITE-002)
8. Implement precondition checks: missing backlog.md (SYNC-PRE-001), version too new (SYNC-PRE-002)
9. Update SYNC-NOTES-004 spec marker to `[x]` after pipeline is wired
