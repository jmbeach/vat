# Arrow: backlog-format

File formats, parsing, serialization, ID generation, and per-file modules for all VAT-managed files.

## Status

**PARTIAL** — last audited 2026-06-04 (git SHA `426964053f024c0e1380a365543da31798536bb7`). Core format modules implemented (frontmatter, body regions, base32, tombstone, item files, project config, user config); parsed-region grammar and bullet-marker parsing/serialization are active gaps.

## References

### HLD
- docs/high-level-design.md (§ File formats, § ID scheme, § Key design decisions)

### LLD
- docs/llds/backlog-format.md

### EARS
- docs/specs/backlog-format-specs.md (55 specs: 39 implemented, 16 gaps)

### Tests
- src/backlog_file.rs (inline `#[cfg(test)]` — FMT-FM-*, FMT-RGN-*)
- src/base32.rs (inline `#[cfg(test)]` — FMT-B32-*)
- src/tombstone.rs (inline `#[cfg(test)]` — FMT-TOMB-*)
- src/item_file.rs (inline `#[cfg(test)]` — FMT-ITEM-*, SYNC-NOTES-004)
- src/user_config.rs (inline `#[cfg(test)]` — FMT-USR-*)
- src/project_config.rs (inline `#[cfg(test)]` — FMT-CFG-*)

### Code
- src/backlog_file.rs — frontmatter and body-region parsing/serialization
- src/base32.rs — Crockford base32 validation and random generation
- src/tombstone.rs — backlog/.used-ids reader/writer
- src/file_io.rs — shared read/write with line-ending normalization
- src/item_file.rs — backlog/items/<id>.md creation/append and notes indentation stripping
- src/user_config.rs — ~/.config/vat/config.toml parse/serialize/load/save
- src/project_config.rs — backlog/vat.toml parse/serialize/load/save

## Architecture

**Purpose:** Defines and implements every on-disk file format VAT reads or writes, providing typed parsing to in-memory structs and deterministic serialization back to disk.

**Key Components:**
1. `backlog_file` — parses `backlog.md` frontmatter and body regions; serializes back
2. `base32` — Crockford base32 alphabet validation and random ID suffix generation
3. `tombstone` — reads/writes `backlog/.used-ids` (ID reuse prevention)
4. `file_io` — shared read/write with CRLF → LF normalization
5. `item_file` — creates and appends to `backlog/items/<id>.md`; strips note indentation
6. `project_config` — reads/writes `backlog/vat.toml`
7. `user_config` — reads/writes `~/.config/vat/config.toml`

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Frontmatter | FMT-FM-001 to FMT-FM-005 | 4 | 0 | 1 |
| Body regions | FMT-RGN-001 to FMT-RGN-007 | 7 | 0 | 0 |
| Parsed region | FMT-PARSE-001 to FMT-PARSE-006 | 0 | 0 | 6 |
| Base32 | FMT-B32-001 to FMT-B32-007 | 7 | 0 | 0 |
| Markers | FMT-MARK-001 to FMT-MARK-007 | 0 | 0 | 7 |
| Item files | FMT-ITEM-001 to FMT-ITEM-003 | 3 | 0 | 0 |
| Tombstone | FMT-TOMB-001 to FMT-TOMB-009 | 9 | 0 | 0 |
| Project config | FMT-CFG-001 to FMT-CFG-003 | 3 | 0 | 0 |
| User config | FMT-USR-001 to FMT-USR-006 | 6 | 0 | 0 |
| Whitespace/IO | FMT-WS-001 to FMT-WS-002 | 0 | 0 | 2 |

**Summary:** 39 of 55 active specs implemented; 0 deferred; 16 gaps.

## Key Findings

1. **Parsed-region grammar unimplemented** — FMT-PARSE-001–006 have no `@spec` code annotations. The bullet-line grammar (task entries, notes, preamble) described in the LLD has not been written. This directly blocks sync and all bullet-mutating commands.
2. **Bullet marker parsing unimplemented** — FMT-MARK-001–007 have no `@spec` code annotations. Marker parsing and canonical serialization (order, spacing) are needed by sync and all bullet-mutating commands.
3. **FMT-WS-001 drift** — `src/file_io.rs:1` is annotated `@spec FMT-WS-001` but the spec is `[ ]` with note "Normalization infrastructure landed in `file_io`; marker stays `[ ]` pending caller wiring." Infrastructure exists; callers that still use `std::fs` directly need to migrate.
4. **FMT-WS-002 gap** — No `@spec FMT-WS-002` annotation anywhere; trailing-whitespace stripping on bullet serialization is not yet implemented.
5. **FMT-FM-005 gap** — `vat init` creation of `backlog.md` with `version: 1` frontmatter is not yet implemented (cmd_init is a stub in src/main.rs:92).

## Work Required

### Must Fix
1. Implement FMT-PARSE-001–006 (parsed-region grammar) — hard blocker for sync and all bullet-mutating commands
2. Implement FMT-MARK-001–007 (bullet marker parsing and canonical serialization) — hard blocker for sync and all bullet-mutating commands

### Should Fix
3. Resolve FMT-WS-001 drift: audit remaining `std::fs` callers, route through `file_io::read_to_string`, then update spec marker to `[x]`
4. Implement FMT-WS-002: trailing-whitespace stripping on bullet serialize (small; add alongside marker serialization)

### Nice to Have
5. FMT-FM-005 will resolve naturally as part of cmd_init implementation (see `commands` segment)
