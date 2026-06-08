# Arrow: backlog-format

File format parsing and serialization — `backlog.md`, item files, tombstone, project and user config, ID scheme, and shared IO utilities.

## Status

**PARTIAL** — last audited 2026-06-08 (git SHA `e2c7ad8cf75a7da4a970a1eedfb8b6e5784d4c14`). Core parsing, tombstone, config, and ID modules are implemented. Bullet marker parsing/serialization (FMT-MARK-001..007), trailing-whitespace stripping (FMT-WS-002), and empty-bullet warning (FMT-PARSE-006) are active gaps. FMT-WS-001 has partial wiring (see Key Findings).

## References

### HLD
- docs/high-level-design.md (§ File formats, § ID scheme, § Key design decisions)

### LLD
- docs/llds/backlog-format.md

### EARS
- docs/specs/backlog-format-specs.md (55 specs: 44 implemented, 11 active gaps, 0 deferred)

### Tests
- src/backlog_file.rs (inline tests)
- src/base32.rs (inline tests)
- src/tombstone.rs (inline tests)
- src/item_file.rs (inline tests)
- src/file_io.rs (inline tests)
- src/project_config.rs (inline tests)
- src/user_config.rs (inline tests)

### Code
- src/backlog_file.rs — backlog.md parse/serialize (`@spec FMT-FM-*`, `FMT-RGN-*`, `FMT-PARSE-*`, `CMD-CC-001`)
- src/base32.rs — Crockford base32 validate + generate (`@spec FMT-B32-001..007`)
- src/tombstone.rs — `.used-ids` read/write (`@spec FMT-TOMB-001..009`)
- src/item_file.rs — `items/<id>.md` create/append + notes indent-strip (`@spec FMT-ITEM-001..003`, `SYNC-NOTES-004`, `FMT-WS-001`)
- src/file_io.rs — shared read/write with line-ending normalization (`@spec FMT-WS-001`)
- src/project_config.rs — `backlog/vat.toml` (`@spec FMT-CFG-001..003`)
- src/user_config.rs — `~/.config/vat/config.toml` (`@spec FMT-USR-001..006`)

## Architecture

**Purpose:** All persistent state in VAT is plain files. This segment owns the canonical format of every file VAT reads or writes, the shared IO utilities, and the ID generation primitive. All command segments consume these modules.

**Key Components:**
1. `backlog_file` — parses the flat-list format (frontmatter, body regions, task entries, preamble)
2. `base32` — validates and generates Crockford base32 identifiers
3. `tombstone` — maintains the append-only `.used-ids` list
4. `item_file` — manages per-task note files including indentation stripping
5. `file_io` — single read/write entry point with CRLF normalization
6. `project_config` — reads/writes `vat.toml`; validates project prefix
7. `user_config` — reads/writes `~/.config/vat/config.toml`; resolves XDG path

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Frontmatter (FMT-FM) | FMT-FM-001..005 | 4 | 0 | 1 |
| Body regions (FMT-RGN) | FMT-RGN-001..007 | 7 | 0 | 0 |
| Parsed region structure (FMT-PARSE) | FMT-PARSE-001..006 | 5 | 0 | 1 |
| Crockford base32 (FMT-B32) | FMT-B32-001..007 | 7 | 0 | 0 |
| Bullet markers (FMT-MARK) | FMT-MARK-001..007 | 0 | 0 | 7 |
| Item files (FMT-ITEM) | FMT-ITEM-001..003 | 3 | 0 | 0 |
| Tombstone (FMT-TOMB) | FMT-TOMB-001..009 | 9 | 0 | 0 |
| Project config (FMT-CFG) | FMT-CFG-001..003 | 3 | 0 | 0 |
| User config (FMT-USR) | FMT-USR-001..006 | 6 | 0 | 0 |
| Line endings / whitespace (FMT-WS) | FMT-WS-001..002 | 0 | 0 | 2 |

**Summary:** 44 of 55 active specs implemented; 0 deferred; 11 active gaps.

Active gap IDs: FMT-FM-005, FMT-PARSE-006, FMT-MARK-001..007, FMT-WS-001, FMT-WS-002.

## Key Findings

1. **FMT-WS-001 partially wired** — `file_io.rs` implements CRLF→LF normalization and carries `@spec FMT-WS-001`. `item_file.rs` also wires through `file_io` and annotates FMT-WS-001. The spec marker is `[ ]` with a note "pending caller wiring" (implying not all callers are connected). Whether `item_file.rs` wiring alone is sufficient to upgrade the marker, or whether `backlog_file.rs` also needs to route through `file_io` before the spec can be marked `[x]`, is a user decision.

2. **FMT-MARK-001..007 not yet implemented** — Bullet marker validation (ID format, `by:` name, `blocked-by:` format), canonical serialization order, and title-detection logic are all spec gaps. `backlog_file.rs` covers parsing (FMT-PARSE-*) but does not yet handle marker-level semantics. These gates the `start`, `block`, `unblock`, and `done` commands.

3. **FMT-FM-005 gap** — `vat init` writing a `version: 1` frontmatter block is a gap because `cmd_init` in `main.rs` is a stub. The frontmatter parse logic (FMT-FM-001..004) is implemented.

## Work Required

### Must Fix
1. Implement FMT-MARK-001..007 — bullet marker parsing and serialization (blocks all single-entry commands)
2. Implement FMT-WS-002 — trailing-whitespace stripping on bullet serialization

### Should Fix
3. Implement FMT-PARSE-006 — empty-bullet warning and skip logic
4. Implement FMT-FM-005 — writes `version: 1` frontmatter on `vat init` (deferred to commands segment)

### Nice to Have
5. Clarify FMT-WS-001 spec marker once all callers are wired (user decision — see Key Findings)
