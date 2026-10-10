# EARS Specs: `vat sync`

Requirements for the `vat sync` command. See [sync LLD](../llds/sync.md).

Status: `[x]` implemented, `[ ]` active gap, `[D]` deferred.

## ID assignment

- [x] **SYNC-ID-001** — When `vat sync` encounters a bullet without an `[id]` marker, the system shall assign it a new ID composed of the project prefix from `vat.toml`, a literal `-`, and 3 randomly-generated Crockford base32 characters.
- [x] **SYNC-ID-002** — When generating a new ID, the system shall reject any candidate that appears in `backlog/.used-ids` or that is currently present on another bullet in the parsed region, and retry up to 100 times.
- [x] **SYNC-ID-003** — When ID generation exhausts its retry cap, the system shall abort with an error and shall not write to any file.
- [x] **SYNC-ID-004** — When `vat sync` assigns a new ID, the system shall append that ID to `backlog/.used-ids` after a successful write of `backlog.md`.
- [x] **SYNC-ID-005** — When `vat sync` encounters a bullet whose `[id]` prefix does not match the configured `project.id`, the system shall print a warning and leave the marker unchanged.
- [x] **SYNC-ID-006** — When `vat sync` encounters two bullets sharing the same `[id]`, the system shall abort with an error and shall not write to any file.

## Marker normalization

- [x] **SYNC-MARK-001** — When `vat sync` writes a bullet, the system shall emit markers in the canonical order defined by FMT-MARK-004.
- [x] **SYNC-MARK-002** — `vat sync` shall not modify the value of `[in-progress]`, `[by:...]`, or `[blocked-by:...]` markers; it only reorders and respaces them. Lowercasing of ID values in `[id]` and `[blocked-by:...]` markers (FMT-MARK-001, FMT-MARK-003 — everything VAT writes is lowercase) is canonicalization, not a value modification.
- [x] **SYNC-MARK-003** — `vat sync` shall not strip dangling `[blocked-by:<id>]` markers whose target ID is not present in the parsed region.
- [x] **SYNC-MARK-004** — When `vat sync` re-serializes a bullet carrying more than one `[blocked-by:...]` marker (only the first is kept, per FMT-MARK-007), the system shall print a warning naming each dropped target ID, so the loss is not silent.

## Notes extraction

- [x] **SYNC-NOTES-001** — When a bullet has note lines associated with it, `vat sync` shall remove those lines from `backlog.md`.
- [x] **SYNC-NOTES-002** — When a bullet has note lines whose trimmed content is non-empty and no `backlog/items/<id>.md` exists, `vat sync` shall create that file with frontmatter `id: <id>` and the trimmed notes as the body.
- [x] **SYNC-NOTES-003** — When a bullet has note lines whose trimmed content is non-empty and `backlog/items/<id>.md` already exists, `vat sync` shall append a blank line followed by the trimmed notes to the existing body.
- [x] **SYNC-NOTES-004** — When extracting notes, `vat sync` shall first trim leading and trailing blank lines, then strip the longest common leading-whitespace *byte* prefix shared by all remaining non-blank lines. Leading whitespace is the run of space and tab bytes at the start of a line; the common prefix is compared byte-for-byte (a tab and a space do not match), so notes whose non-blank lines do not share an identical leading-whitespace prefix are left un-stripped. Interior blank lines are preserved as empty lines and do not contribute to the common prefix.
- [x] **SYNC-NOTES-005** — When a bullet's note lines are empty after trimming (only whitespace and blank lines), `vat sync` shall not create or modify any item file but shall still remove those lines from `backlog.md`.

## Item-file pointer suffix

- [x] **SYNC-PTR-001** — When `vat sync` finishes processing a bullet whose id has a corresponding `backlog/items/<id>.md` file, the system shall ensure the bullet's title ends with the literal suffix ` (see ./items/<id>.md)` (single leading space, path relative to `backlog/`), appending it if not already present.
- [x] **SYNC-PTR-002** — When `vat sync` finishes processing a bullet whose id has no corresponding `backlog/items/<id>.md` file, the system shall not add the pointer suffix and shall not remove an existing one.
- [x] **SYNC-PTR-003** — When the bullet's title already ends with the canonical ` (see ./items/<id>.md)` suffix and the item file exists, `vat sync` shall leave the suffix unchanged (idempotent).

## Title length

Throughout: a **new bullet** is a well-formed bullet (non-empty title, FMT-PARSE-006) with no `[id]` marker; the **measured title** is computed from the raw bullet line per SYNC-LEN-001; the **limit** is `[sync].max_title_length` from `vat.toml` (FMT-CFG-004).

- [ ] **SYNC-LEN-001** — When `vat sync` measures a new bullet's title, the system shall derive the measured title from the raw bullet line by (1) removing the leading `- ` bullet marker, (2) removing every span from a `[` through the next `]`, anywhere on the line, whether or not it is a recognized marker (an unmatched `[` is kept as text), (3) removing a trailing `(see ./items/<anything>.md)` pointer, and (4) trimming leading and trailing whitespace; interior whitespace is kept as written. Its length is the count of Unicode scalar values (chars, not bytes).
- [ ] **SYNC-LEN-002** — When the limit is greater than 0 and at least one new bullet's measured title is longer than the limit, `vat sync` shall abort with an error and shall not write to any file (no `backlog.md` rewrite, no item-file create/append, no `.used-ids` append). A measured title of exactly the limit passes.
- [ ] **SYNC-LEN-003** — When `vat sync` aborts per SYNC-LEN-002, the error shall list every offending new bullet in the file (not only the first), each identified by its 1-based position among all task bullets in the parsed region (malformed bullets included, matching the FMT-PARSE-006 warning), its measured length, and its measured title truncated to the first 80 chars with `…` appended when cut.
- [ ] **SYNC-LEN-004** — When `vat sync` aborts per SYNC-LEN-002, the error shall state the limit and the config key that sets it, show a short example of a well-formed bullet with indented notes beneath it, and explain that `vat sync` moves indented notes into `backlog/items/<id>.md`.
- [ ] **SYNC-LEN-005** — `vat sync` shall not apply the title-length check to a bullet that already carries an `[id]` marker, nor to a malformed bullet (FMT-PARSE-006).
- [ ] **SYNC-LEN-006** — When the limit is `0`, `vat sync` shall not perform the title-length check.
- [ ] **SYNC-LEN-007** — When `vat sync` aborts per SYNC-LEN-002, the system shall exit with code 1 (a user-fixable command error, CMD-EXIT-002).
- [ ] **SYNC-LEN-008** — When `vat sync` aborts per SYNC-LEN-002, the system shall not print the dropped-`[blocked-by:...]` warnings of SYNC-MARK-004 (nothing was re-serialized); malformed-bullet warnings (FMT-PARSE-006) shall still print before the error.
- [ ] **SYNC-LEN-009** — When a `vat sync` run has both a title-length violation and a duplicate `[id]` (SYNC-ID-006), the system shall report the title-length error.

## Idempotence and writes

- [x] **SYNC-WRITE-001** — `vat sync` shall produce byte-identical output when run twice in succession on a file that already has all bullets ID'd, no notes, and canonical marker order.
- [x] **SYNC-WRITE-002** — When the serialized output of `vat sync` is byte-identical to the input file, the system shall skip the write to `backlog.md`.
- [x] **SYNC-WRITE-003** — When `vat sync` aborts due to any error during parsing or ID generation, the system shall not write to any file.
- [x] **SYNC-WRITE-004** — When `vat sync` runs and `backlog/items/` does not exist but a write is needed, the system shall create it.

## Preconditions

- [x] **SYNC-PRE-001** — When `backlog/backlog.md` does not exist, `vat sync` shall abort with an error pointing the user at `vat init`.
- [x] **SYNC-PRE-002** — When the `backlog.md` frontmatter `version` exceeds the CLI's supported major version, `vat sync` shall abort before any other processing.

## Out of scope for v1

- [D] **SYNC-GC-001** — Garbage-collecting orphaned `backlog/items/<id>.md` files whose IDs no longer appear in `backlog.md`.
