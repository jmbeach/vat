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
