# Arrow: backlog-format

File format parsing and serialization — every on-disk format VAT reads or writes.

## Status

**PARTIAL** — last audited 2026-06-12 (git SHA `9dbd445`). Core parsing fully implemented across 7 modules. FMT-FM-005 now implemented (cmd_init.rs, PR #29). FMT-MARK-* (bullet marker parsing) is the largest gap and blocks both `sync` and the bullet-mutating commands.

## References

### HLD
- docs/high-level-design.md (§ File formats, § ID scheme, § System architecture)

### LLD
- docs/llds/backlog-format.md

### EARS
- docs/specs/backlog-format-specs.md (55 active specs: 46 implemented, 9 gaps)

### Tests
- src/backlog_file.rs (inline `#[cfg(test)]`)
- src/base32.rs (inline `#[cfg(test)]`)
- src/tombstone.rs (inline `#[cfg(test)]`)
- src/user_config.rs (inline `#[cfg(test)]`)
- src/project_config.rs (inline `#[cfg(test)]`)
- src/file_io.rs (inline `#[cfg(test)]`)
- src/item_file.rs (inline `#[cfg(test)]`)

### Code
- src/backlog_file.rs — FMT-FM-*, FMT-RGN-*, FMT-PARSE-*
- src/base32.rs — FMT-B32-*
- src/tombstone.rs — FMT-TOMB-*
- src/user_config.rs — FMT-USR-*
- src/project_config.rs — FMT-CFG-*
- src/file_io.rs — FMT-WS-001 (line-ending normalization; wired into cmd_config and sync)
- src/item_file.rs — FMT-ITEM-*, SYNC-NOTES-004

## Architecture

**Purpose:** Defines and implements every on-disk file format VAT reads or writes: `backlog.md` structure (frontmatter, body regions, task entries), bullet-line grammar and marker parsing, per-task item files, tombstone file, project config, and user config.

**Key Components:**
1. `src/backlog_file.rs` — frontmatter parsing, body-region splitting, task-entry parse/serialize
2. `src/base32.rs` — Crockford base32 validation and random generation
3. `src/tombstone.rs` — `.used-ids` reader/writer
4. `src/user_config.rs` — global user config parse/serialize/load/save
5. `src/project_config.rs` — project config parse/serialize/load/save
6. `src/file_io.rs` — shared read/write with LF line-ending normalization
7. `src/item_file.rs` — per-task item file create/append + notes indentation stripping

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Frontmatter | FMT-FM-001 to 005 | 5 | 0 | 0 |
| Body regions | FMT-RGN-001 to 007 | 7 | 0 | 0 |
| Parsed region structure | FMT-PARSE-001 to 006 | 5 | 0 | 1 |
| Crockford base32 | FMT-B32-001 to 007 | 7 | 0 | 0 |
| Bullet line markers | FMT-MARK-001 to 007 | 0 | 0 | 7 |
| Item files | FMT-ITEM-001 to 003 | 3 | 0 | 0 |
| Tombstone file | FMT-TOMB-001 to 009 | 9 | 0 | 0 |
| Project config | FMT-CFG-001 to 003 | 3 | 0 | 0 |
| User config | FMT-USR-001 to 006 | 6 | 0 | 0 |
| Line endings / WS | FMT-WS-001 to 002 | 1 | 0 | 1 |

**Summary:** 46 of 55 active specs implemented; 0 deferred; 9 gaps. FMT-MARK-* (7 specs) is the largest gap and the primary blocker for other segments.

## Key Findings

1. **FMT-MARK-* entirely absent** — No `@spec FMT-MARK-*` annotations appear anywhere in the source. `src/backlog_file.rs` parses task entries (bullet lines + notes) but does not yet parse or serialize the individual marker tokens within a bullet. This is the critical missing piece that blocks `vat sync` (marker normalization) and all bullet-mutating commands (`start`, `block`, `unblock`, `done`).

2. **FMT-WS-001 implemented** — The earlier drift signal is resolved: `vat sync` (vat-t1h, PR #32) reads `backlog.md` through `file_io::read_to_string`, completing the caller wiring, and the spec marker flipped to `[x]`. `src/file_io.rs` carries the `@spec FMT-WS-001` annotations and tests.

3. **FMT-FM-005 implemented** — `cmd_init.rs:write_backlog_files` (PR #29) writes `"---\nversion: 1\n---\n"` to `backlog.md`, satisfying this spec. `@spec FMT-FM-005` annotation added to `src/cmd_init.rs`.

4. **FMT-PARSE-006 gap** — Empty/malformed bullet warning behavior not implemented. `src/backlog_file.rs` parses entries but has no warning path for empty bullets.

5. **FMT-WS-002 gap** — Trailing whitespace stripping on bullet serialization not implemented (no `@spec FMT-WS-002` annotations anywhere).

## Work Required

### Must Fix
1. Implement marker parsing and serialization (FMT-MARK-001 to 007) — unblocks `sync` and all bullet-mutating commands. This is the segment's top priority.

### Should Fix
2. Wire FMT-WS-002: strip trailing whitespace in bullet serializer (backlog_file.rs serialize path).
3. Implement FMT-PARSE-006: empty-bullet warning in the parsed-region parser.

