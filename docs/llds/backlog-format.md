# LLD: Backlog file formats

Defines the on-disk format of every file VAT reads or writes, and the parsing/serialization rules that all commands share. See [HLD](../high-level-design.md) for context.

## Files

```
backlog/
  backlog.md          # the flat list (canonical state)
  vat.toml            # project config
  .used-ids           # tombstone list
  README.md           # human-facing explainer (written once by `vat init`)
  items/
    <id>.md           # per-task notes (only when notes exist)
~/.config/vat/config.toml   # user config
```

`backlog/README.md` is written once by `vat init` and never read or rewritten by VAT after that. Users may freely edit or delete it; VAT does not depend on its contents.

## `backlog/backlog.md`

### Optional YAML frontmatter

The file MAY begin with a YAML frontmatter block, delimited by lines consisting solely of `---`:

```
---
version: 1
---
```

Recognized keys:

- **`version`** — integer. The schema major version this file conforms to. `vat init` writes `version: 1`. The CLI refuses to operate on any file whose `version` is greater than the CLI's supported major version (currently `1`). A file with no frontmatter, or frontmatter without `version`, is treated as version 1 for backward compatibility.

Unknown frontmatter keys are preserved on rewrite. The frontmatter, if present, is preserved byte-for-byte except for keys VAT actively manages.

A file without frontmatter is fully supported — frontmatter only becomes mandatory if/when a future major version requires it.

### Body regions

After any frontmatter, the body has two regions separated by the **first** line consisting solely of `---` (optionally surrounded by whitespace). v1 supports only `---` as the separator; the other CommonMark thematic-break forms (`***`, `___`) are not recognized.

Note: the closing `---` of frontmatter is consumed as part of the frontmatter block; the body's separator is the *next* `---` line after that.

- **Parsed region**: everything *above* the separator. VAT reads, mutates, and writes only this region.
- **Freeform region**: everything from the separator onward, including the separator line itself. VAT preserves it byte-for-byte.

If no separator exists, the entire file is the parsed region.

### Parsed region grammar

The parsed region is a sequence of **task entries**, optionally preceded by **preamble**.

A **task entry** is:

1. A **bullet line** starting at column 0 with `- ` (a hyphen followed by a single space).
2. Optionally followed by **note lines**: any subsequent lines that are *not* a bullet line, up until the next bullet line at column 0 or the end of the parsed region.

Indentation under a bullet is conventional but not required — any non-bullet line following a bullet is part of that bullet's notes. This means a paragraph or sub-list written at column 0 between two bullets attaches to the *first* bullet, not the second.

Only `-` is recognized as a bullet marker; lines starting with `*` or `+` are treated as note content of the preceding bullet (or as preamble if no bullet has appeared yet).

**Preamble** is any content before the first bullet line in the parsed region. VAT preserves it byte-for-byte and emits it back at the top of the file.

### Bullet line canonical form

```
- [<id>] [in-progress] [by:<name>] [blocked-by:<id>] <title>
```

- **`[<id>]`** — required after `vat sync`. Format: `<project>-<suffix>` where `<project>` is the 3-char Crockford base32 project prefix from `vat.toml` and `<suffix>` is 3 chars Crockford base32. Pre-sync, the bullet may have no `[id]`.
- **`[in-progress]`** — literal string, optional.
- **`[by:<name>]`** — `<name>` is a non-empty string of `[A-Za-z0-9_.-]+`. Optional.
- **`[blocked-by:<id>]`** — `<id>` matches the project ID format. Optional. Multiple `[blocked-by:...]` markers are not supported in v1; only the first is preserved if a user types more than one.
- **`<title>`** — the rest of the line, trimmed. Required, non-empty. When the bullet has a corresponding `backlog/items/<id>.md` file, the title ends with the literal suffix ` (see ./items/<id>.md)` (single space before the open paren, path relative to `backlog/` with an explicit `./` prefix so editors recognize it as a clickable path). The suffix is part of the title — round-tripped verbatim by the parser and managed by `vat sync` (see the [sync LLD](./sync.md)). It is not a marker.

Markers are always front-loaded in the order shown. `vat sync` normalizes order if a user shuffles them by hand. Other commands write markers in canonical position directly.

### Bullet line parsing rules

- Marker brackets are matched greedily from the left of the bullet body. As soon as the parser encounters a token that doesn't match a known marker pattern, the rest of the line is the title.
- Unknown `[...]` tokens at the front are treated as part of the title (so users can type `- [TODO] something` without VAT consuming the bracket).
- Whitespace between markers is normalized to a single space on serialize.
- Empty bullets (`-` with no title) are a parse error; `vat sync` prints a warning, leaves the line untouched, and skips the entry.

### Notes

"Notes" are the lines between a bullet and the next bullet (or end of parsed region), excluding any trailing blank lines that immediately precede the next bullet. On `vat sync` notes are extracted into `backlog/items/<id>.md` and the bullet line is left as a single line in `backlog.md` whose title ends with ` (see ./items/<id>.md)`.

### Reserved-state rules

A bullet may have `[in-progress]` without `[by:...]` or vice versa; both forms count as "claimed" for the purposes of `vat start`'s refusal. `vat start` always writes both markers together; partial states only appear via hand-edit.

## `backlog/items/<id>.md`

YAML frontmatter plus markdown body:

```
---
id: <full-id>
---

<body>
```

- The frontmatter `id` field MUST equal the filename stem.
- The body is the verbatim notes content, with the indentation that the notes had under the bullet stripped (the minimum common leading whitespace across all note lines, after dropping leading/trailing blank lines).
- On re-sync with new notes appended, VAT appends a single blank line to the existing body, then the new notes (with the same indentation-stripping rule applied to the new notes alone).
- File created lazily — only when there are notes. Deleted on `vat done`.

## `backlog/.used-ids`

Plain text, newline-delimited, one full ID per line (e.g., `foo-7k2`). Order is append-order. No comments or blank lines. VAT appends each newly-assigned ID and each ID being deleted by `vat done` (the latter is redundant if sync already added it but is kept for safety). Deduplication on read.

The file is committed. If missing, VAT treats it as empty and creates it on first write.

## `backlog/vat.toml`

```toml
[project]
id = "foo"   # exactly 3 characters, Crockford base32 alphabet
```

- `project.id` is required. Validated on every command; an invalid or missing prefix is a hard error with a pointer to `vat init`.
- The file may contain other `[section]` blocks in the future; unknown keys are preserved on rewrite (write only the keys we own).

## `~/.config/vat/config.toml`

```toml
[user]
name = "jared"
```

- Path follows XDG: `$XDG_CONFIG_HOME/vat/config.toml`, falling back to `~/.config/vat/config.toml`.
- `user.name` is optional in the file; commands that require it (`vat start`) error with a pointer to `vat config set user.name <name>` if missing.

## ID alphabet & generation

A small shared primitive (likely `src/base32.rs`) backs every place an ID or prefix is validated, parsed, or generated.

**Alphabet.** `0123456789ABCDEFGHJKMNPQRSTVWXYZ` (32 chars, Crockford — no `I`, `L`, `O`, `U`). Inputs are accepted in either case; everything VAT writes is lowercase.

**Strict input.** Validation rejects any character outside the canonical alphabet, including the ambiguous `I`/`L`/`O`/`U`. We do not silently fold `I/L → 1` or `O → 0` per Crockford's decoder hint — the project prefix and suffix are short, user-typed identifiers and a typo should be a hard error pointing at the bad character, not a quiet rewrite.

**Module surface.** Two `pub(crate)` operations:

- `validate(s, expected_len) -> Result<(), Base32Error>` — checks length and per-character membership in the alphabet (case-insensitive). The `expected_len` is passed by callers (`3` for both prefix and suffix today) so the magic number stays visible at call sites and tests can hit the `WrongLength` path directly.
- `random(n, &mut impl RngCore) -> String` — generates `n` lowercase Crockford base32 characters from a caller-supplied RNG. Always lowercase. Returns an owned `String` — the allocation is negligible against the 100-retry collision loop in `vat sync`. RNG is injected so tests can pass a seeded RNG and assert exact output; production callers pass `rand::thread_rng()`.

**Error type.** `Base32Error` is a `thiserror`-derived enum with `WrongLength { expected, got }` and `InvalidChar { ch, pos }` variants. `pos` is a 0-based char index (not byte index) so it aligns with printed glyph positions even if the input contains non-ASCII characters; the renderer can `+1` if it wants a 1-based "column N" message. Variants exist so callers (`vat init`, `vat config set project.id`) can match on `InvalidChar` to render a caret under the bad character. The project-wide error-handling pattern is documented in [cli.md](./cli.md#error-handling).

**No decoding to bytes.** VAT never decodes Crockford base32 to bytes — IDs are opaque tokens, not encoded numbers. The module exposes alphabet membership and random-character generation only.

**Dependencies.** Hand-rolled alphabet table (~15 lines). New crate dependencies: `rand` (RNG), `thiserror` (error derive). `anyhow` will be added when `main` wires up top-level error handling, but isn't needed by this module directly.

**Where collision-handling lives.** The 100-retry loop on suffix collisions is owned by `vat sync` (see [sync LLD](./sync.md)), not by this module. `random` is dumb — it generates and returns; the caller decides whether the result collides with `.used-ids`.

## File IO and line endings

All file reads and writes flow through a single helper module (`src/file_io.rs`). Commands and parsers never call `std::fs` directly — they go through this module so line-ending policy lives in exactly one place.

**Read path.** The module exposes `read_to_string(path) -> io::Result<String>`. Before returning, it normalizes all line-ending conventions to `\n`: CRLF (`\r\n`) pairs collapse to `\n`, and any remaining bare `\r` (lone or trailing) also becomes `\n`. Parsers downstream may assume LF-only input.

**Write path.** The module exposes `write(path, contents) -> io::Result<()>`, which writes bytes as composed by the caller. Serializers always produce `\n`-terminated strings; the IO layer never injects `\r`. The write-path invariant is enforced by convention in the LLD rather than by a runtime check.

**Surface scope for v1.** Just `read_to_string` and `write`. Atomic-write semantics (tempfile + rename) are out of scope for this module today; tracked separately if they become necessary.

## Decisions & alternatives

- **First `---` after the frontmatter as the parsed/freeform boundary.** Simpler than a magic comment. Markdown-native. The frontmatter (if present) consumes its own pair of `---` delimiters first, then the next `---` line in the file is the body's boundary. Cost: someone using `---` for a section break inside the parsed region truncates their backlog. Documented as a known restriction.
- **Optional frontmatter for versioning.** Provides an upgrade path without forcing existing users to add ceremony. Supports a "refuse to operate on newer schema" check so a stale CLI doesn't silently corrupt a forward-versioned file. Considered a sentinel comment (`<!-- vat-version: 1 -->`); rejected as less standard and harder to extend with other metadata later.
- **Crockford base32 for both prefix and suffix.** Same alphabet everywhere — easier to validate, no I/L/O/U confusion. RFC4648 base32 was rejected for the ambiguous character set.
- **Markers front-loaded, fixed order.** Easier parsing (can detect markers before reaching the title) and easier visual scanning. Free-form marker placement was rejected for parser complexity.
- **Notes go in a separate file rather than staying inline.** Keeps `backlog.md` scannable as a flat list. Cost: two files to look at for a task with notes; mitigated by `[id]` being the obvious lookup key.
- **Tombstone file rather than git-history scan.** Cheap, explicit, decoupled from git internals. Cost: a second source of truth that can drift if hand-edited; accepted because writers are limited and the file is append-only.
- **Strict Crockford input (no `I/L/O` folding).** Crockford's decoder hint says lenient decoders should fold `I/L → 1` and `O → 0`. We don't, because the inputs here are short user-typed identifiers where a typo is more likely than intentional use of an ambiguous glyph; a hard error is more helpful than a silent rewrite. Cost: a user who types `Iol` for their prefix gets an error instead of `101`.
- **Hand-rolled alphabet, no `crockford` crate.** The surface is two functions over a 32-char table; pulling a crate would dwarf the implementation. Cost: we own ~15 lines of alphabet code.
- **Injected RNG.** `random` takes `&mut impl RngCore` rather than calling `thread_rng()` internally, so collision-retry tests in `vat sync` can drive deterministic sequences. Cost: every caller threads an RNG through; in practice only `vat sync` calls it.
- **Normalize all line endings on read, not just CRLF.** A file saved with bare-CR line endings (rare, but possible from legacy exports or odd paste sources) would otherwise parse as a single giant line and confuse every downstream parser. Cost: a note body that deliberately contains a `\r` (e.g., terminal output with progress bars) loses fidelity on round-trip. Accepted — the failure mode of leaving bare CR untouched is worse than the rare data-fidelity loss.
- **Single IO module rather than per-command IO.** Every read goes through one normalization point so no command, parser, or future helper has to remember the rule. Cost: a thin indirection over `std::fs`; trivial.
