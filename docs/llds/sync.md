# LLD: `vat sync`

`vat sync` is the only command that mutates the structure of `backlog.md`. It is idempotent: running it twice on the same input produces the same output as running it once. See [backlog-format LLD](./backlog-format.md) for the file grammar and [HLD](../high-level-design.md) for context.

## Inputs

- `backlog/backlog.md` — current state.
- `backlog/vat.toml` — for `project.id`, and `[sync].max_title_length` (optional; default 120, `0` disables — see [format LLD § vat.toml](./backlog-format.md#backlogvattoml)).
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
3.5. Title-length check (new bullets only). For each well-formed task_entry whose
     bullet has no [id], compute its *measured title* from the raw bullet line
     (see "Title-length check" below) and count it in chars. If max_title_length > 0
     and any measured title is longer than max_title_length, collect *every* such
     bullet and abort with a single error, writing nothing (see "Title-length error").
     Bullets that already carry an [id] are never checked; malformed bullets are
     inert here as everywhere else.
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

## Title-length check

The check exists to keep `backlog.md` scannable: a bullet is a one-line handle, and detail belongs in notes, which sync already moves into `items/<id>.md`. It runs at the moment of capture-to-structure — when sync is about to give a bullet its ID — because that is the last point where the author is still editing the raw line.

- **What is measured: the measured title.** A plain text rule on the raw bullet line, independent of the marker parser, so the binary and the skill count identically:
  1. Drop the leading `- ` bullet marker.
  2. Remove every `[...]` span anywhere on the line — a `[` through the next `]`. Every bracketed group counts as a tag, recognized marker or not: `[vat-7k2]`, `[in-progress]`, `[by:jared]`, `[TODO]`, `[P1]`, the text half of a markdown link. An unmatched `[` (no later `]`) is ordinary text.
  3. Remove a trailing `(see ./items/<anything>.md)` pointer.
  4. Trim leading and trailing whitespace. Interior whitespace counts as written (removing a mid-line tag can leave a double space; that is one extra char, not worth a rule).
  5. Count what's left.

  Excluding every tag and the pointer means VAT's own decorations — `[id]` from sync, `[in-progress] [by:<name>]` from `vat start`, the pointer suffix from sync — never push a passing bullet over the limit, and a bullet whose `[id]` was demoted behind an unknown tag (`- [TODO] [vat-7k2] title`) measures the same as before.

  Examples (all measure as `Fix sync`, 8 chars): `- Fix sync`, `- [TODO] Fix sync`, `- [vat-7k2] [in-progress] [by:jared] Fix sync (see ./items/vat-7k2.md)`.
- **Units: chars.** Unicode scalar values, so a non-ASCII title is not penalized for its UTF-8 width. No grapheme clustering — the limit is a scannability heuristic, not a typesetting rule.
- **Boundary.** A measured title of exactly `max_title_length` chars passes; `max_title_length + 1` fails.
- **Who is checked: bullets without an `[id]`.** Already-ID'd bullets are grandfathered, so upgrading VAT (or lowering the limit) never stops an existing backlog from syncing. Raising or lowering the limit only affects bullets captured afterward.
- **Disable.** `max_title_length = 0` turns the check off.
- **Ordering.** The check runs after bullet parsing and before ID assignment, so a run that fails here has not generated any IDs and the duplicate-ID / retry-exhaustion errors are not reached. A file with both problems reports the title-length error first; the duplicate surfaces on the next run. Accepted — both are rare together, and pre-computing duplicates just to merge the reports isn't worth the code.
- **Warnings on abort.** Malformed-bullet warnings from step 3 still print ahead of the error (they remain true). Dropped-`[blocked-by:]` warnings (SYNC-MARK-004) are suppressed on a title-length abort, because nothing was re-serialized and nothing was dropped.

### Title-length error

A hard error — `SyncError::TitleTooLong { limit, offenders }`, exit code 1 (user-fixable, like a duplicate ID), no files written (SYNC-WRITE-003). It names every offending bullet in one report so the user fixes them in one pass, then teaches the fix:

```
error: 2 new bullets have titles longer than 120 characters (backlog/vat.toml [sync].max_title_length):
  bullet #4 (147 chars): "Refactor the parser so markers can appear anywhere on the line and also handl…"
  bullet #9 (133 chars): "Investigate why sync is slow on large backlogs, probably the item-file scan bu…"

Keep the bullet to a short title and put the detail in indented notes beneath it:

  - Speed up sync on large backlogs
    Probably the per-run item-file scan; profile with 5k bullets.
    Compare against the read_dir pre-scan from vat-mzd.

`vat sync` moves the indented notes into backlog/items/<id>.md and links the bullet to it.
```

- The first line is the variant's `Display`; it does not carry its own `error:` prefix (the CLI's `fail` adds it). The header agrees in number: `1 new bullet has a title longer than…` / `N new bullets have titles longer than…`.
- Bullets are named by 1-based position among *all* task bullets, malformed ones included (`bullet #N`) — the same locator the malformed-bullet and dropped-blocker warnings use (the parsed region does not track file line numbers).
- Each offender shows its measured length and its measured title, truncated to the first 80 chars (with `…` appended when cut) *before* debug-quoting.
- The example block and the closing sentence are fixed text; the limit and the count are interpolated.
- Exact wording is not specified beyond these elements; the skill's prose implementation conveys the same elements without being byte-identical (fidelity covers file state, not message text).

## Idempotence

After one successful sync:
- Every entry has an `[id]`.
- No entry has notes lines.
- All markers are in canonical order.
- All assigned ids are in `.used-ids`.
- Every entry whose id has a corresponding `items/<id>.md` file ends its title with ` (see ./items/<id>.md)`.

A second run finds nothing to assign, nothing to extract, and nothing to normalize, so it produces a byte-identical file (modulo trailing newline normalization, which sync also normalizes to a single trailing `\n`).

## Edge behaviors

- **New bullet over the title-length limit**: hard error listing every offender, no writes (see "Title-length check"). Its notes do not count toward the length and do not rescue it — the title itself must be shortened. The bullet and its notes are left verbatim; the error is the same whether or not notes exist.
- **Already-ID'd bullet over the limit**: not checked; passes through.
- **Bullet already has an id, no notes**: pass-through (only marker order normalization).
- **Bullet already has an id, with notes**: notes are appended to existing item file (or a new one is created if missing). Bullet collapses to one line.
- **Bullet without id, with notes**: id is generated; item file is created with notes.
- **Two bullets share the same id (hand-edit error)**: hard error, abort sync, no writes performed. Message points the user at the offending lines.
- **Bullet has an id whose project prefix doesn't match `project.id`**: warn, but pass through. (Allows users to import IDs from another project later if needed; v1 just preserves them.)
- **Empty bullet** (`-` alone, or markers with no title text, e.g. `- [foo-7k2]`): warn, skip the entry, do not assign an id, do not extract any "notes" that follow it. The bullet line *and its note lines* are preserved in place — skipping extraction without preserving the notes would silently destroy them. A marker-only bullet's ID token is inert: it is not seeded into the collision set and does not trigger duplicate-ID detection.
- **ID-shaped token behind an unknown token** (`- [TODO] [foo-7k2] title`): the token is title text (format LLD parsing rules), so the bullet has no ID and receives a fresh one. The title, including the old token, is preserved verbatim.
- **Uppercase ID values** (`- [FOO-7K2] title`): rewritten lowercase on normalization, per the alphabet rule that everything VAT writes is lowercase.
- **Multiple `[blocked-by:...]` on one bullet** (`- [vat-t1h] [blocked-by:vat-f1w] [blocked-by:vat-h8x] title`): only the first survives re-serialization (FMT-MARK-007). Sync warns once per dropped target ID, naming the bullet position and the original line (SYNC-MARK-004) — re-serialization would otherwise silently lose blockers a user listed on purpose. Parsing exposes the discards via `Bullet::parse_reporting_dropped`; the kept blocker is unchanged. (A title-less bullet with multiple blockers is skipped wholesale and passes through verbatim, so no blocker is lost and no drop-warning fires.)
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

All writes happen at the end, after all parsing and id generation succeed. If any step fails (parse error, title-length violation, duplicate id, retry exhaustion), no files are mutated. This makes the command safe to retry.

## Decisions & alternatives

- **All-or-nothing writes.** Considered streaming line-by-line writes; rejected because a parse failure mid-stream would leave the file half-mutated. The whole-file rewrite is fine for backlog files of any reasonable size.
- **Retry cap of 100 on id generation.** With 32k-ID space and (say) 1000 used ids, collision probability per try is ~3%; 100 retries is overkill. Cap exists to prevent infinite loops in degenerate cases (project nearing namespace exhaustion).
- **Bullet identity comes from the front-loaded marker parser, not an anywhere-scan.** An earlier interim implementation found an ID-shaped token anywhere on the line, so `- [TODO] [foo-7k2] title` counted as "has ID foo-7k2". That contradicted the format LLD (markers are front-loaded; the first unknown token starts the title). Sync now uses the shared parser: such a bullet has no ID and gets a fresh one. Cost: a user who relied on a mid-line token being the bullet's identity sees a new ID assigned and the old token demoted to title text — visible in the diff, never destructive (the token text is preserved).
- **Malformed (title-less) bullets are fully inert.** Their ID-shaped tokens are not seeded into the collision set and don't count for duplicate detection. Seeding them would require a second, looser parse of lines we've declared unparseable; the per-candidate collision odds are 1/32768 against a random draw, and the printed warning drives the human fix. Cost: until the user fixes the line, a fresh assignment could in principle mint the same ID, surfacing as a duplicate-ID error on a later sync.
- **Title length: tag-stripped raw line, new bullets only, hard error.** Alternatives considered: measuring the full serialized line (rejected — VAT's own markers and pointer suffix would push a passing bullet over the limit later); measuring the marker parser's title (rejected — it keeps unknown tags like `[TODO]` and only strips a pointer matching the bullet's own `[id]`, which a new bullet lacks, so the count would depend on parser subtleties the skill would have to reproduce exactly; treating every `[...]` as a tag is one rule anyone can apply by eye); checking every bullet (rejected — an upgrade or a lowered limit would wedge existing backlogs until someone edited old titles); warning instead of erroring (rejected — a warning scrolls past, and the bullet gets an ID and becomes permanent, at which point it is grandfathered). Cost: a long title that was ID'd before the limit existed stays long until someone edits it by hand.
- **No garbage collection of orphaned item files.** Keeping it out of v1 because it's risky (silently deleting user content). Could be added as `vat sync --gc` later.
- **Thematic breaks elsewhere in the parsed region**: the *first* break is the boundary. Any thematic break inside the parsed region truncates parsing at that point. Documented as the known cost of this design choice.
