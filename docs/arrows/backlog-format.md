# Arrow: backlog-format

On-disk file formats and the parsing/serialization primitives shared by all commands: `backlog.md` frontmatter and regions, task-entry grammar, bullet-line markers, Crockford base32, tombstone, item files, project config, user config, and IO normalization.

## Status

**PARTIAL** — last audited 2026-06-03 (git SHA `52bbfb58a6f7f999969da68bef55b38bd59fb744`). Core infrastructure (frontmatter, regions, base32, tombstone, configs, item files) is implemented. Task-entry parsing (FMT-PARSE-*) and marker normalization (FMT-MARK-*) are not yet implemented; FMT-WS-001 infrastructure is present but callers are not yet wired.

## References

### HLD
- docs/high-level-design.md (§ File formats, § ID scheme, § System architecture)

### LLD
- docs/llds/backlog-format.md

### EARS
- docs/specs/backlog-format-specs.md (55 active specs: 38 implemented, 17 gaps)

### Tests
- src/backlog_file.rs (inline `#[test]` module)
- src/base32.rs (inline `#[test]` module)
- src/tombstone.rs (inline `#[test]` module)
- src/user_config.rs (inline `#[test]` module)
- src/project_config.rs (inline `#[test]` module)
- src/file_io.rs (inline `#[test]` module)
- src/item_file.rs (inline `#[test]` module)

### Code
- src/backlog_file.rs — frontmatter + region parsing (`@spec` FMT-FM-*, FMT-RGN-*)
- src/base32.rs — Crockford base32 validation + generation (`@spec` FMT-B32-*)
- src/tombstone.rs — `.used-ids` read/append (`@spec` FMT-TOMB-*)
- src/user_config.rs — user config load/save (`@spec` FMT-USR-*)
- src/project_config.rs — project config load/save (`@spec` FMT-CFG-*)
- src/file_io.rs — CRLF/CR normalization (`@spec` FMT-WS-001)
- src/item_file.rs — item file create/append + notes indentation stripping (`@spec` FMT-ITEM-*, SYNC-NOTES-004)

## Architecture

**Purpose:** Defines and implements every on-disk format VAT reads or writes, plus the shared utilities (base32, IO) that all commands depend on.

**Key Components:**
1. `backlog_file.rs` — parses frontmatter and splits body into parsed/freeform regions; serializes back
2. `base32.rs` — validates and generates Crockford base32 strings (shared by sync and init)
3. `tombstone.rs` — append-only `.used-ids` reader/writer with strict validation
4. `item_file.rs` — creates and appends to `backlog/items/<id>.md` files, including indentation stripping (SYNC-NOTES-004)
5. `project_config.rs` — reads/writes `backlog/vat.toml` with unknown-key preservation
6. `user_config.rs` — reads/writes `~/.config/vat/config.toml` with XDG path resolution
7. `file_io.rs` — single IO module: normalizes all line endings on read (CRLF → LF, bare CR → LF)

**Not yet implemented in this segment:** task-entry parsing (`FMT-PARSE-*`), bullet-line marker parsing and normalization (`FMT-MARK-*`), trailing-whitespace stripping on serialize (`FMT-WS-002`).

## Spec Coverage

| Category | Spec IDs | Total | Implemented | Deferred | Gaps |
|----------|----------|-------|-------------|----------|------|
| Frontmatter | FMT-FM-001..005 | 5 | 3 | 0 | 2 |
| Body regions | FMT-RGN-001..007 | 7 | 7 | 0 | 0 |
| Parsed region structure | FMT-PARSE-001..005, FMT-PARSE-006 | 6 | 0 | 0 | 6 |
| Base32 | FMT-B32-001..007 | 7 | 7 | 0 | 0 |
| Bullet markers | FMT-MARK-001..007 | 7 | 0 | 0 | 7 |
| Item files | FMT-ITEM-001..003 | 3 | 3 | 0 | 0 |
| Tombstone | FMT-TOMB-001..009 | 9 | 9 | 0 | 0 |
| Project config | FMT-CFG-001..003 | 3 | 3 | 0 | 0 |
| User config | FMT-USR-001..006 | 6 | 6 | 0 | 0 |
| Whitespace / IO | FMT-WS-001..002 | 2 | 0 | 0 | 2 |
| **Total** | | **55** | **38** | **0** | **17** |

**Summary:** 38 of 55 active specs implemented; 17 gaps concentrated in FMT-PARSE-* (entry parsing), FMT-MARK-* (marker normalization), and FMT-WS-* (IO wiring).

## Key Findings

1. **FMT-WS-001 partial implementation** — `src/file_io.rs` and `src/item_file.rs` carry `// @spec FMT-WS-001` and the normalization infrastructure is present, but the spec file notes "marker stays `[ ]` pending caller wiring." The spec should be updated to `[x]` once all read callers are confirmed to flow through `file_io::read_to_string`. Decision required (see structured findings report).

2. **FMT-PARSE-* and FMT-MARK-* not yet started** — No `@spec FMT-PARSE-*` or `@spec FMT-MARK-*` annotations exist in any source file. The task-entry grammar and marker parsing are prerequisites for `vat sync` and the bullet-mutating commands. These are blocked in practice on this segment completing.

3. **item_file.rs spans two segments** — `src/item_file.rs` carries both `@spec FMT-ITEM-*` (backlog-format) and `@spec SYNC-NOTES-004` (sync). The indentation-stripping logic lives here rather than in the sync command. Referenced in both `backlog-format.md` and `sync.md`.

## Work Required

### Must Fix
1. Implement FMT-PARSE-001..005 — task-entry grammar in backlog_file.rs (FMT-PARSE-001..005)
2. Implement FMT-PARSE-006 — malformed/empty bullet warning (FMT-PARSE-006)
3. Implement FMT-MARK-001..007 — bullet marker parsing and canonical serialization (FMT-MARK-*)
4. Wire FMT-WS-001 callers — confirm all read paths use `file_io::read_to_string`; update spec to `[x]`
5. Implement FMT-WS-002 — trailing-whitespace stripping on bullet serialize
6. Implement FMT-FM-002 — version-too-new abort check in backlog_file.rs
7. Implement FMT-FM-005 — vat init writes `version: 1` frontmatter (owned by commands segment but depends on backlog_file serializer)
