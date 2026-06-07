# Arrow: backlog-format

On-disk file formats and low-level parsing primitives — backlog.md regions, bullet lines, item files, tombstone, base32, and config files.

## Status

**PARTIAL** — last audited 2026-06-05 (see `index.yaml` for audited SHA). Frontmatter, region splitting, base32, item files, tombstone, and config parsing are implemented and tested. Bullet-line parsing (FMT-PARSE-*) and marker parsing/serialization (FMT-MARK-*) are not yet implemented.

## References

### HLD
- docs/high-level-design.md (§ File formats, § ID scheme)

### LLD
- docs/llds/backlog-format.md

### EARS
- docs/specs/backlog-format-specs.md (55 active specs: 39 implemented, 16 gaps)

### Tests
- src/backlog_file.rs (inline `#[cfg(test)]` — frontmatter and region splitting)
- src/base32.rs (inline `#[cfg(test)]` — FMT-B32-*)
- src/file_io.rs (inline `#[cfg(test)]` — FMT-WS-001)
- src/item_file.rs (inline `#[cfg(test)]` — FMT-ITEM-*, SYNC-NOTES-004 strip_indent)
- src/tombstone.rs (inline `#[cfg(test)]` — FMT-TOMB-*)
- src/project_config.rs (inline `#[cfg(test)]` — FMT-CFG-*)
- src/user_config.rs (inline `#[cfg(test)]` — FMT-USR-*)

### Code
- src/backlog_file.rs (@spec FMT-FM-*, FMT-RGN-*, CMD-CC-001)
- src/base32.rs (@spec FMT-B32-*)
- src/file_io.rs (@spec FMT-WS-001)
- src/item_file.rs (@spec FMT-ITEM-*, SYNC-NOTES-004, FMT-WS-001)
- src/tombstone.rs (@spec FMT-TOMB-*)
- src/project_config.rs (@spec FMT-CFG-*)
- src/user_config.rs (@spec FMT-USR-*)

## Architecture

**Purpose:** Defines and enforces the on-disk formats for every file VAT reads or writes. All parsers and serializers live here; commands do not touch `std::fs` directly.

**Key Components:**
1. `src/backlog_file.rs` — frontmatter parsing, region splitting (parsed vs. freeform); bullet-line parsing not yet present
2. `src/base32.rs` — Crockford base32 validation (`validate`) and generation (`random`)
3. `src/file_io.rs` — read/write with CRLF→LF normalization
4. `src/item_file.rs` — item file create, append, and indentation stripping (`strip_indent`)
5. `src/tombstone.rs` — `.used-ids` read (returning `HashSet<String>`) and blind append
6. `src/project_config.rs` — `backlog/vat.toml` parse and serialize
7. `src/user_config.rs` — `~/.config/vat/config.toml` parse, serialize, path resolution

## Spec Coverage

| Category | Spec IDs | Implemented | Gaps | Deferred |
|----------|----------|-------------|------|----------|
| Frontmatter (FMT-FM) | FMT-FM-001 to FMT-FM-005 | 4 | 1 | 0 |
| Body regions (FMT-RGN) | FMT-RGN-001 to FMT-RGN-007 | 7 | 0 | 0 |
| Parsed region (FMT-PARSE) | FMT-PARSE-001 to FMT-PARSE-006 | 0 | 6 | 0 |
| Base32 (FMT-B32) | FMT-B32-001 to FMT-B32-007 | 7 | 0 | 0 |
| Markers (FMT-MARK) | FMT-MARK-001 to FMT-MARK-007 | 0 | 7 | 0 |
| Item files (FMT-ITEM) | FMT-ITEM-001 to FMT-ITEM-003 | 3 | 0 | 0 |
| Tombstone (FMT-TOMB) | FMT-TOMB-001 to FMT-TOMB-009 | 9 | 0 | 0 |
| Project config (FMT-CFG) | FMT-CFG-001 to FMT-CFG-003 | 3 | 0 | 0 |
| User config (FMT-USR) | FMT-USR-001 to FMT-USR-006 | 6 | 0 | 0 |
| IO / whitespace (FMT-WS) | FMT-WS-001 to FMT-WS-002 | 0 | 2 | 0 |

**Summary:** 39 of 55 active specs implemented; 16 gaps (FMT-FM-005, FMT-PARSE-001–006, FMT-MARK-001–007, FMT-WS-001–002); 0 deferred.

## Key Findings

1. **Bullet parsing not yet implemented** — `src/backlog_file.rs` parses frontmatter and splits regions correctly but does not yet parse bullet lines or markers. FMT-PARSE-001–006 and FMT-MARK-001–007 (13 specs) are all active gaps with no code.

2. **FMT-WS-001 infrastructure present, caller wiring pending** — `src/file_io.rs::read_to_string` normalizes line endings and `src/item_file.rs` applies it. The spec itself notes "normalization infrastructure landed in file_io; marker stays `[ ]` pending caller wiring." `src/backlog_file.rs` reads must be routed through `file_io::read_to_string`.

3. **FMT-FM-005 depends on init wiring** — the spec (vat init writes version: 1 frontmatter) is blocked on CMD-INIT-005 in the commands segment.

4. **FMT-WS-002 (trailing whitespace strip on serialize) is a gap** — the backlog serializer does not yet strip trailing whitespace from bullet lines.

## Work Required

### Must Fix
1. Implement bullet-line parsing: task entry recognition, preamble, notes attachment (FMT-PARSE-001–006)
2. Implement marker parsing and serialization in canonical order (FMT-MARK-001–007)

### Should Fix
3. Wire `file_io::read_to_string` into `backlog_file.rs` reader so FMT-WS-001 caller wiring is complete
4. Add trailing-whitespace strip to bullet serializer (FMT-WS-002)
5. FMT-FM-005 resolves automatically once `vat init` is implemented in the commands segment
