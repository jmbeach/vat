# Arrow: backlog-format

On-disk file formats — parsing and serialization of `backlog.md`, item files, tombstone, project config, and global user config. Also covers Crockford base32 ID primitives and file IO normalization.

## Status

**PARTIAL** — last audited 2026-06-07 (git SHA `fe4be98`). 44 of 55 active specs implemented; 11 gaps concentrated in bullet-marker parsing/serialization (FMT-MARK-001..007) and line-ending/whitespace (FMT-WS-001..002). Blocks the `sync` and `commands` segments.

## References

### HLD
- docs/high-level-design.md (§ File formats, § ID scheme)

### LLD
- docs/llds/backlog-format.md

### EARS
- docs/specs/backlog-format-specs.md (55 active specs; 0 deferred)

### Tests
- src/backlog_file.rs (inline `#[cfg(test)]` module)
- src/base32.rs (inline `#[cfg(test)]` module)
- src/item_file.rs (inline `#[cfg(test)]` module)
- src/tombstone.rs (inline `#[cfg(test)]` module)
- src/user_config.rs (inline `#[cfg(test)]` module)
- src/file_io.rs (inline `#[cfg(test)]` module)
- src/project_config.rs (inline `#[cfg(test)]` module)

### Code
- src/backlog_file.rs — `backlog.md` frontmatter and body-region parsing/serialization
- src/base32.rs — Crockford base32 validation and random generation
- src/item_file.rs — item file create/append and indentation-strip
- src/file_io.rs — line-ending normalization (FMT-WS-001 infrastructure)
- src/project_config.rs — `vat.toml` parse/serialize
- src/tombstone.rs — `.used-ids` read/append
- src/user_config.rs — `~/.config/vat/config.toml` parse/serialize

## Architecture

**Purpose:** Provides the parsing, serialization, and file-IO layer that all commands consume. No command calls `std::fs` directly — all reads flow through `file_io::read_to_string` (with CRLF normalization) and writes through module-level helpers.

**Key Components:**
1. `backlog_file` — parses `backlog.md` into frontmatter + parsed region + freeform region; serializes back. Hosts the `check_version` helper cross-used by all commands (CMD-CC-001).
2. `base32` — validates and generates Crockford base32 identifiers. Injected RNG for test determinism.
3. `item_file` — creates and appends to `backlog/items/<id>.md`; owns the indentation-stripping algorithm (SYNC-NOTES-004).
4. `file_io` — `read_to_string` (CRLF + bare-CR normalization) and `write`. Normalization infrastructure is present but not yet exercised end-to-end (FMT-WS-001 caller wiring depends on `sync`/`commands`).
5. `project_config` — `vat.toml` round-trip with unknown-key preservation.
6. `tombstone` — `.used-ids` strict reader and blind-append writer.
7. `user_config` — `~/.config/vat/config.toml` round-trip with strict XDG path resolution.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Frontmatter | FMT-FM-001 to FMT-FM-005 | 4 | 0 | 1 |
| Body regions | FMT-RGN-001 to FMT-RGN-007 | 7 | 0 | 0 |
| Parsed region | FMT-PARSE-001 to FMT-PARSE-006 | 5 | 0 | 1 |
| Crockford base32 | FMT-B32-001 to FMT-B32-007 | 7 | 0 | 0 |
| Bullet markers | FMT-MARK-001 to FMT-MARK-007 | 0 | 0 | 7 |
| Item files | FMT-ITEM-001 to FMT-ITEM-003 | 3 | 0 | 0 |
| Tombstone | FMT-TOMB-001 to FMT-TOMB-009 | 9 | 0 | 0 |
| Project config | FMT-CFG-001 to FMT-CFG-003 | 3 | 0 | 0 |
| Global config | FMT-USR-001 to FMT-USR-006 | 6 | 0 | 0 |
| Line endings / WS | FMT-WS-001 to FMT-WS-002 | 0 | 0 | 2 |

**Summary:** 44 of 55 active specs implemented; 0 deferred; 11 gaps.

## Key Findings

1. **Bullet marker parsing not yet implemented** — FMT-MARK-001..007 have no `[x]` in the spec and no `@spec FMT-MARK-*` annotations exist in any source file. The LLD "Bullet line canonical form" section describes the grammar and parsing rules but no corresponding code exists.
2. **FMT-WS-001 infrastructure present but not wired end-to-end** — `file_io::read_to_string` normalizes line endings; the spec marker remains `[ ]` because the command handlers (`vat sync`, mutating commands) that exercise the full round-trip are not yet implemented.
3. **FMT-WS-002 not implemented** — trailing whitespace strip on bullet serialization has no code; likely part of the marker serializer (FMT-MARK-*).
4. **CMD-CC-001 cross-segment** — `src/backlog_file.rs:158` has `// @spec FMT-FM-002, CMD-CC-001`; the version-check helper lives in the format layer (appropriate, since it's a format-level check) and is counted as implemented in the `commands` segment.

## Work Required

### Must Fix
1. Implement bullet marker parser and serializer (FMT-MARK-001..007) — required before `sync` or any bullet-mutating command can be implemented
2. Implement FMT-PARSE-006 (warn on empty bullet) — missing warning path in the parser

### Should Fix
1. Implement FMT-WS-002 (trailing whitespace strip on bullet serialize) — natural companion to FMT-MARK-*
2. FMT-FM-005 (vat init writes `version: 1` frontmatter) — belongs to the `commands` segment (init command); no change needed in this module
3. FMT-WS-001 caller wiring — belongs to `sync` and `commands` segments; no change needed in `file_io` itself
