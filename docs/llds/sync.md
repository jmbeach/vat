# LLD: `vat sync`

`vat sync` is the only command that mutates the structure of `backlog.md`. It is idempotent: running it twice on the same input produces the same output as running it once. See [backlog-format LLD](./backlog-format.md) for the file grammar and [HLD](../high-level-design.md) for context.

## Inputs

- `backlog/backlog.md` — current state.
- `backlog/vat.toml` — for `project.id`.
- `backlog/.used-ids` — to avoid handing out previously-used IDs.
- `backlog/items/*.md` — to know which item files already exist (and to append to them).

## Outputs

- Mutated `backlog/backlog.md` (parsed region only).
- New or appended `backlog/items/<id>.md` files for entries that have notes.
- Appended lines in `backlog/.used-ids` for each newly-assigned ID.

## Algorithm

```
1. Load project config; fail loudly if vat.toml missing or invalid.
2. Read backlog.md.
   a. If it starts with a YAML frontmatter block, parse it. If `version` is greater
      than the CLI's supported major (currently 1), abort with a clear error:
      "backlog file is version N, this CLI supports up to version 1; please upgrade vat."
      Missing/empty frontmatter is treated as version 1.
   b. After the frontmatter block, split the body into (parsed_region, freeform_region)
      at the first `---` line.
3. Parse parsed_region into a preamble plus a sequence of task_entry elements.
   The preamble is any content (blank lines, headings, paragraphs, etc.)
   appearing before the first `- ` bullet — see the format LLD's "Preamble" definition.
   For each task_entry capture: bullet_line, notes_lines.
   Parse each bullet_line with the format LLD's greedy front-loaded marker
   parser ("Bullet line parsing rules"). A bullet whose parse yields an empty
   title is *malformed*: print a warning naming the line, leave the bullet line
   and any note lines following it untouched in place, and exclude the entry
   from every subsequent step (no ID assignment, no notes extraction, no
   normalization). Malformed bullets are fully inert — an ID-shaped token on a
   malformed line does not participate in collision seeding (step 4) or
   duplicate detection; the printed warning is the guard that gets the line
   fixed.
4. Read .used-ids into a set `used`. Add to it every id currently present in parsed_region
   (the parsed `[id]` markers of well-formed bullets; title text is never scanned for IDs).
5. For each well-formed task_entry in order:
   a. If the bullet has no [id]:
        - Generate a new id: project_prefix + "-" + 3 random Crockford base32 chars,
          retrying until it isn't in `used`. Cap retries at 100; if exceeded, hard error.
        - Add the new id to `used` and to the append-set for .used-ids.
        - Insert the [id] marker at the front of the bullet (before any other markers).
   b. Normalize markers on the bullet line into canonical order with single-space
      separators, by re-serializing the parsed bullet (format LLD "Bullet line
      canonical form"). Normalization reorders and respaces markers and lowercases
      ID values in `[id]` and `[blocked-by:...]` (the alphabet rule: everything VAT
      writes is lowercase); it never changes which markers are present or their
      payloads. Per the format LLD's parsing rules, an ID-shaped token that appears
      after the first unknown bracketed token is title text, not the bullet's ID —
      `- [TODO] [foo-7k2] title` is a bullet *without* an ID, and sync assigns it a
      fresh one, front-loaded before `[TODO]`, leaving the title (including the
      `[foo-7k2]` text) verbatim.
   c. If the entry has notes:
        - Strip indentation (longest common leading-whitespace byte prefix across
          non-blank lines; tab ≠ space) and trim leading/trailing blank lines.
          See the [format LLD](./backlog-format.md) and SYNC-NOTES-004.
        - If the result is empty (the notes were only whitespace/blank lines):
            do nothing — do not create an item file, do not append.
        - Else if items/<id>.md does not exist:
            create it with frontmatter {id: <id>} and body = the stripped notes.
        - Else:
            append a blank line and the stripped notes to the existing body
            (frontmatter is left untouched).
        - In all cases, clear notes_lines on the entry (so the bullet becomes a single
          line in the output, regardless of whether the notes were empty or substantive).
   d. Item-file pointer suffix:
        - If items/<id>.md exists (whether pre-existing or just created in step c) and
          the bullet's title does not already end with " (see ./items/<id>.md)",
          append that suffix (with a single leading space).
        - If items/<id>.md does not exist, do not add a suffix and do not strip an
          existing one. Sync is conservative: it never removes information.
6. Serialize the elements back into the parsed_region.
   Preamble is emitted verbatim at the top.
   Each task_entry emits exactly one line (the normalized bullet).
7. Write frontmatter (if any) + parsed_region + freeform_region back to backlog.md.
8. Append new ids to .used-ids (one per line, no dedup needed — the set logic above already
   ensured uniqueness within this run; if a re-run for some reason tries to re-append, the
   read step dedups).
```

## Idempotence

After one successful sync:
- Every entry has an `[id]`.
- No entry has notes lines.
- All markers are in canonical order.
- All assigned ids are in `.used-ids`.
- Every entry whose id has a corresponding `items/<id>.md` file ends its title with ` (see ./items/<id>.md)`.

A second run finds nothing to assign, nothing to extract, and nothing to normalize, so it produces a byte-identical file (modulo trailing newline normalization, which sync also normalizes to a single trailing `\n`).

## Edge behaviors

- **Bullet already has an id, no notes**: pass-through (only marker order normalization).
- **Bullet already has an id, with notes**: notes are appended to existing item file (or a new one is created if missing). Bullet collapses to one line.
- **Bullet without id, with notes**: id is generated; item file is created with notes.
- **Two bullets share the same id (hand-edit error)**: hard error, abort sync, no writes performed. Message points the user at the offending lines.
- **Bullet has an id whose project prefix doesn't match `project.id`**: warn, but pass through. (Allows users to import IDs from another project later if needed; v1 just preserves them.)
- **Empty bullet** (`-` alone, or markers with no title text, e.g. `- [foo-7k2]`): warn, skip the entry, do not assign an id, do not extract any "notes" that follow it. The bullet line *and its note lines* are preserved in place — skipping extraction without preserving the notes would silently destroy them. A marker-only bullet's ID token is inert: it is not seeded into the collision set and does not trigger duplicate-ID detection.
- **ID-shaped token behind an unknown token** (`- [TODO] [foo-7k2] title`): the token is title text (format LLD parsing rules), so the bullet has no ID and receives a fresh one. The title, including the old token, is preserved verbatim.
- **Uppercase ID values** (`- [FOO-7K2] title`): rewritten lowercase on normalization, per the alphabet rule that everything VAT writes is lowercase.
- **Item file exists but no bullet references it**: not touched. (Could be a stale file from a manual delete; sync doesn't garbage-collect.)
- **Item file exists, bullet has the id, but the `(see ./items/<id>.md)` suffix is missing or hand-edited away**: sync re-appends the canonical suffix.
- **Item file does not exist but a `(see ./items/<id>.md)` suffix is present** (e.g., the user manually deleted the item file): sync leaves the suffix alone. Sync never strips information.
- **`backlog.md` does not exist**: hard error pointing at `vat init`.
- **`backlog/items/` does not exist but a write is needed**: created.
- **No `---` separator in the file**: entire file is the parsed region; sync does not add one.
- **Paragraph or text between two bullets**: attaches to the prior bullet as notes (per the format LLD); on sync, if the trimmed content is non-empty, it is moved to that bullet's item file. Whitespace-only "notes" between bullets do not create or modify an item file.
- **Line endings**: handled by the shared IO layer — see [File IO and line endings](./backlog-format.md#file-io-and-line-endings). Sync has no line-ending policy of its own.
- **Trailing whitespace on bullet lines**: stripped on serialize.
- **No-op sync**: if the serialized output is byte-identical to the input, sync skips the write so the git working tree stays clean.
- **Item file frontmatter**: VAT does not validate that the `id:` field inside an existing `items/<id>.md` matches the filename. The frontmatter is left untouched on append; the filename is the source of truth.

## Failure modes

All writes happen at the end, after all parsing and id generation succeed. If any step fails (parse error, duplicate id, retry exhaustion), no files are mutated. This makes the command safe to retry.

## Decisions & alternatives

- **All-or-nothing writes.** Considered streaming line-by-line writes; rejected because a parse failure mid-stream would leave the file half-mutated. The whole-file rewrite is fine for backlog files of any reasonable size.
- **Retry cap of 100 on id generation.** With 32k-ID space and (say) 1000 used ids, collision probability per try is ~3%; 100 retries is overkill. Cap exists to prevent infinite loops in degenerate cases (project nearing namespace exhaustion).
- **Bullet identity comes from the front-loaded marker parser, not an anywhere-scan.** An earlier interim implementation found an ID-shaped token anywhere on the line, so `- [TODO] [foo-7k2] title` counted as "has ID foo-7k2". That contradicted the format LLD (markers are front-loaded; the first unknown token starts the title). Sync now uses the shared parser: such a bullet has no ID and gets a fresh one. Cost: a user who relied on a mid-line token being the bullet's identity sees a new ID assigned and the old token demoted to title text — visible in the diff, never destructive (the token text is preserved).
- **Malformed (title-less) bullets are fully inert.** Their ID-shaped tokens are not seeded into the collision set and don't count for duplicate detection. Seeding them would require a second, looser parse of lines we've declared unparseable; the per-candidate collision odds are 1/32768 against a random draw, and the printed warning drives the human fix. Cost: until the user fixes the line, a fresh assignment could in principle mint the same ID, surfacing as a duplicate-ID error on a later sync.
- **No garbage collection of orphaned item files.** Keeping it out of v1 because it's risky (silently deleting user content). Could be added as `vat sync --gc` later.
- **Thematic breaks elsewhere in the parsed region**: the *first* break is the boundary. Any thematic break inside the parsed region truncates parsing at that point. Documented as the known cost of this design choice.
