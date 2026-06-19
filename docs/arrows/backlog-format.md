# Arrow: backlog-format

File format parsing and serialization — every on-disk format VAT reads or writes.

## Status

**OK** — last touched 2026-06-19 (vat-h4n: project-ID prefix relaxed to alphanumeric); previously audited 2026-06-14 (HEAD `ee8b0e6`). All 59 active specs implemented across 9 modules. vat-h4n added the `src/prefix.rs` validator (FMT-PFX-001..004) and relaxed the prefix segment of every `<prefix>-<suffix>` token check (project config, tombstone, bullet) from Crockford to alphanumeric while keeping the suffix Crockford. The bullet marker tokenizer (FMT-MARK-001..007, FMT-WS-002) landed via vat-g5y (PR #46) in `src/bullet.rs`; FMT-PARSE-006 (empty-bullet warn-and-skip) landed via vat-v3k when `vat sync` was wired onto `Bullet::parse`/`serialize`.

## References

### HLD
- docs/high-level-design.md (§ File formats, § ID scheme, § System architecture)

### LLD
- docs/llds/backlog-format.md

### EARS
- docs/specs/backlog-format-specs.md (59 active specs: 59 implemented, 0 gaps)

### Tests
- src/backlog_file.rs (inline `#[cfg(test)]`)
- src/base32.rs (inline `#[cfg(test)]`)
- src/tombstone.rs (inline `#[cfg(test)]`)
- src/prefix.rs (inline `#[cfg(test)]` — FMT-PFX-001 to 004)
- src/user_config.rs (inline `#[cfg(test)]`)
- src/project_config.rs (inline `#[cfg(test)]`)
- src/file_io.rs (inline `#[cfg(test)]`)
- src/item_file.rs (inline `#[cfg(test)]`)
- src/bullet.rs (inline `#[cfg(test)]` — FMT-MARK-001 to 007, FMT-PARSE-006, FMT-WS-002)
- src/sync.rs (inline `#[cfg(test)]` — FMT-PARSE-006 warn-and-skip integration)

### Code
- src/backlog_file.rs — FMT-FM-*, FMT-RGN-*, FMT-PARSE-*
- src/base32.rs — FMT-B32-* (suffix validator/generator)
- src/prefix.rs — FMT-PFX-* (alphanumeric project-ID prefix validator)
- src/tombstone.rs — FMT-TOMB-* (suffix via base32, prefix via `prefix`)
- src/user_config.rs — FMT-USR-*
- src/project_config.rs — FMT-CFG-* (prefix validated via `prefix`)
- src/file_io.rs — FMT-WS-001 (line-ending normalization; wired into cmd_config and sync)
- src/item_file.rs — FMT-ITEM-*, SYNC-NOTES-004
- src/bullet.rs — FMT-MARK-001 to 007, FMT-PARSE-006 (detection), FMT-WS-002
- src/sync.rs — FMT-PARSE-006 (warn-and-skip entry point)

## Architecture

**Purpose:** Defines and implements every on-disk file format VAT reads or writes: `backlog.md` structure (frontmatter, body regions, task entries), bullet-line grammar and marker parsing, per-task item files, tombstone file, project config, and user config.

**Key Components:**
1. `src/backlog_file.rs` — frontmatter parsing, body-region splitting, task-entry parse/serialize
2. `src/base32.rs` — Crockford base32 validation and random generation (suffix)
3. `src/prefix.rs` — alphanumeric project-ID prefix validation
4. `src/tombstone.rs` — `.used-ids` reader/writer
5. `src/user_config.rs` — global user config parse/serialize/load/save
6. `src/project_config.rs` — project config parse/serialize/load/save
7. `src/file_io.rs` — shared read/write with LF line-ending normalization
8. `src/item_file.rs` — per-task item file create/append + notes indentation stripping
9. `src/bullet.rs` — bullet-line marker tokenizer: `Bullet::parse`/`serialize` (greedy front-loaded marker parsing, canonical-order serialization)

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Frontmatter | FMT-FM-001 to 005 | 5 | 0 | 0 |
| Body regions | FMT-RGN-001 to 007 | 7 | 0 | 0 |
| Parsed region structure | FMT-PARSE-001 to 006 | 6 | 0 | 0 |
| Crockford base32 (suffix) | FMT-B32-001 to 007 | 7 | 0 | 0 |
| Project-ID prefix | FMT-PFX-001 to 004 | 4 | 0 | 0 |
| Bullet line markers | FMT-MARK-001 to 007 | 7 | 0 | 0 |
| Item files | FMT-ITEM-001 to 003 | 3 | 0 | 0 |
| Tombstone file | FMT-TOMB-001 to 009 | 9 | 0 | 0 |
| Project config | FMT-CFG-001 to 003 | 3 | 0 | 0 |
| User config | FMT-USR-001 to 006 | 6 | 0 | 0 |
| Line endings / WS | FMT-WS-001 to 002 | 2 | 0 | 0 |

**Summary:** 59 of 59 active specs implemented; 0 deferred; 0 gaps. The segment no longer blocks `sync`; `commands` remains a consumer of `src/bullet.rs`.

## Key Findings

1. **FMT-MARK-* implemented** — `src/bullet.rs` (vat-g5y, PR #46) implements the greedy front-loaded marker tokenizer (`Bullet::parse`) and canonical-order serializer (`Bullet::serialize`) with full inline tests. `vat sync` is wired onto it (vat-v3k); the bullet-mutating commands (`start`, `block`, `unblock`, `done`) can now consume it.

2. **FMT-WS-001 implemented** — The earlier drift signal is resolved: `vat sync` (vat-t1h, PR #32) reads `backlog.md` through `file_io::read_to_string`, completing the caller wiring, and the spec marker flipped to `[x]`. `src/file_io.rs` carries the `@spec FMT-WS-001` annotations and tests.

3. **FMT-FM-005 implemented** — `cmd_init.rs:write_backlog_files` (PR #29) writes `"---\nversion: 1\n---\n"` to `backlog.md`, satisfying this spec. `@spec FMT-FM-005` annotation added to `src/cmd_init.rs`.

4. **FMT-PARSE-006 implemented** — Detection (`Bullet::parse` → `EmptyTitle`) lives in `src/bullet.rs`; the warn-and-skip behavior (warning printed, line and note lines preserved verbatim, no ID assignment, no notes extraction) lives in `src/sync.rs` (vat-v3k).

5. **FMT-WS-002 implemented** — `Bullet::serialize` strips trailing whitespace (PR #46).

6. **FMT-PFX-* implemented (vat-h4n)** — `src/prefix.rs` validates the user-chosen project-ID prefix as 3 ASCII alphanumeric characters, with a dedicated `PrefixError` (not the Crockford `Base32Error`, which would mislabel a prefix failure). The prefix segment of every `<prefix>-<suffix>` token check was switched from the Crockford `base32::validate` to `prefix::validate` in `src/project_config.rs`, `src/tombstone.rs`, and `src/bullet.rs`, so a relaxed prefix (e.g. `lib`, `ui0`) round-trips end to end. The auto-generated suffix keeps Crockford (`base32::validate`/`random`) unchanged.

## Work Required

None — all active specs implemented. Future consumers: the bullet-mutating commands (`start`, `block`, `unblock`, `done`) should build on `src/bullet.rs` rather than re-parsing lines.

