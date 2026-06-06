# Arrow: backlog-format

File format parsing and serialization — every file VAT reads or writes, and the shared parsing primitives used by all commands.

## Status

**PARTIAL** — last audited 2026-06-06 (git SHA `426964053f024c0e1380a365543da31798536bb7`). Frontmatter, region splitting, Crockford base32, item files, tombstone, and config parsing are fully implemented. Parsed-region grammar (FMT-PARSE-*), bullet marker parsing (FMT-MARK-*), and line-ending normalization wiring (FMT-WS-*) remain as active gaps.

## References

### HLD
- docs/high-level-design.md (§ File formats, § ID scheme)

### LLD
- docs/llds/backlog-format.md

### EARS
- docs/specs/backlog-format-specs.md (55 active specs)

### Tests
- src/backlog_file.rs (inline `#[cfg(test)]` module, ~35 tests)
- src/base32.rs (inline `#[cfg(test)]` module)
- src/tombstone.rs (inline `#[cfg(test)]` module)
- src/item_file.rs (inline `#[cfg(test)]` module)
- src/user_config.rs (inline `#[cfg(test)]` module)
- src/project_config.rs (inline `#[cfg(test)]` module)
- src/file_io.rs (inline `#[cfg(test)]` module)

### Code
- src/backlog_file.rs (`@spec FMT-FM-*, FMT-RGN-*`)
- src/base32.rs (`@spec FMT-B32-*`)
- src/tombstone.rs (`@spec FMT-TOMB-*`)
- src/item_file.rs (`@spec FMT-ITEM-*, SYNC-NOTES-004, FMT-WS-001`)
- src/file_io.rs (`@spec FMT-WS-001`)
- src/user_config.rs (`@spec FMT-USR-*`)
- src/project_config.rs (`@spec FMT-CFG-*`)

## Architecture

**Purpose:** Define and enforce the on-disk format of every file VAT reads or writes. All commands depend on these primitives; no command reads `std::fs` directly.

**Key Components:**
1. `backlog_file.rs` — frontmatter parser, body region splitting, serializer
2. `base32.rs` — Crockford base32 validation and random generation
3. `tombstone.rs` — `.used-ids` reader and appender
4. `item_file.rs` — `backlog/items/<id>.md` create, append, and indentation-stripping
5. `file_io.rs` — shared read/write with CRLF normalization
6. `project_config.rs` — `backlog/vat.toml` parse/serialize
7. `user_config.rs` — `~/.config/vat/config.toml` parse/serialize

## Spec Coverage

| Category | Spec IDs | Implemented | Gaps | Deferred |
|----------|----------|-------------|------|----------|
| Frontmatter (FMT-FM) | FMT-FM-001 to FMT-FM-005 | 4 | 1 | 0 |
| Body regions (FMT-RGN) | FMT-RGN-001 to FMT-RGN-007 | 7 | 0 | 0 |
| Parsed region grammar (FMT-PARSE) | FMT-PARSE-001 to FMT-PARSE-006 | 0 | 6 | 0 |
| Crockford base32 (FMT-B32) | FMT-B32-001 to FMT-B32-007 | 7 | 0 | 0 |
| Bullet markers (FMT-MARK) | FMT-MARK-001 to FMT-MARK-007 | 0 | 7 | 0 |
| Item files (FMT-ITEM) | FMT-ITEM-001 to FMT-ITEM-003 | 3 | 0 | 0 |
| Tombstone (FMT-TOMB) | FMT-TOMB-001 to FMT-TOMB-009 | 9 | 0 | 0 |
| Project config (FMT-CFG) | FMT-CFG-001 to FMT-CFG-003 | 3 | 0 | 0 |
| User config (FMT-USR) | FMT-USR-001 to FMT-USR-006 | 6 | 0 | 0 |
| Whitespace / IO (FMT-WS) | FMT-WS-001 to FMT-WS-002 | 0 | 2 | 0 |

**Summary:** 39 of 55 active specs implemented; 16 gaps; 0 deferred.

## Key Findings

1. **Parsed region grammar not yet implemented** — FMT-PARSE-001 through FMT-PARSE-006 are `[ ]`; `backlog_file.rs` implements frontmatter and region splitting but the bullet/notes/preamble grammar is not wired.
2. **Bullet marker parsing not yet implemented** — FMT-MARK-001 through FMT-MARK-007 are `[ ]`; marker normalization needed before commands or sync can work.
3. **FMT-WS-001 caller wiring pending** — `file_io.rs` provides CRLF normalization; spec note says "marker stays `[ ]` pending caller wiring." `item_file.rs` and `file_io.rs` have `@spec FMT-WS-001` annotations but callers that invoke them may not all route through `file_io.read_to_string` yet.
4. **FMT-FM-005 gap** — `vat init` creates `backlog.md` with `version: 1` frontmatter; not yet wired (init itself is a stub).

## Work Required

### Must Fix
1. Implement parsed-region grammar (FMT-PARSE-001 to FMT-PARSE-006) — needed before any command can parse bullets
2. Implement bullet marker parsing and normalization (FMT-MARK-001 to FMT-MARK-007) — needed for sync and all bullet-mutating commands
3. Wire FMT-WS-001/FMT-WS-002 callers — ensure all reads route through `file_io::read_to_string`

### Should Fix
4. Wire FMT-FM-005 (init frontmatter write) once `vat init` is implemented
