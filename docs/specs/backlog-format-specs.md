# EARS Specs: Backlog file format

Requirements for parsing and serializing `backlog/backlog.md` and related files. See [backlog-format LLD](../llds/backlog-format.md).

Status: `[x]` implemented, `[ ]` active gap, `[D]` deferred.

## Frontmatter

- [x] **FMT-FM-001** — When `backlog.md` begins with a line consisting solely of `---`, the system shall treat the content up to the next line consisting solely of `---` as YAML frontmatter.
- [ ] **FMT-FM-002** — When the frontmatter contains a `version` key whose integer value is greater than the CLI's supported major version, the system shall abort the command with an error message naming the file's version and the CLI's supported version, and shall not write to any file.
- [x] **FMT-FM-003** — When the frontmatter is absent or omits the `version` key, the system shall treat the file as version 1.
- [x] **FMT-FM-004** — When writing `backlog.md`, the system shall preserve unknown frontmatter keys verbatim.
- [ ] **FMT-FM-005** — When `vat init` creates `backlog.md`, the system shall write a frontmatter block containing `version: 1`.

## Body regions

- [ ] **FMT-RGN-001** — When `backlog.md` contains a line consisting solely of `---` after any frontmatter, the system shall treat content above that line as the parsed region and content from that line onward as the freeform region.
- [ ] **FMT-RGN-002** — When `backlog.md` contains no `---` separator after any frontmatter, the system shall treat the entire body as the parsed region.
- [ ] **FMT-RGN-003** — When writing `backlog.md`, the system shall preserve the freeform region byte-for-byte.
- [ ] **FMT-RGN-004** — The system shall not recognize `***` or `___` as a region separator in v1.

## Parsed region structure

- [ ] **FMT-PARSE-001** — The system shall recognize a task entry as a line starting at column 0 with `- ` (hyphen followed by single space).
- [ ] **FMT-PARSE-002** — The system shall not recognize lines starting with `*` or `+` as task entries.
- [ ] **FMT-PARSE-003** — The system shall treat any non-bullet line following a bullet, up until the next bullet at column 0 or the end of the parsed region, as notes belonging to that bullet.
- [ ] **FMT-PARSE-004** — The system shall treat any content in the parsed region appearing before the first bullet line as preamble.
- [ ] **FMT-PARSE-005** — When writing `backlog.md`, the system shall emit the preamble verbatim at the top of the parsed region.

## Crockford base32 utility

These requirements govern the shared `base32` module used wherever IDs or prefixes are validated or generated. See [LLD § ID alphabet & generation](../llds/backlog-format.md#id-alphabet--generation).

- [x] **FMT-B32-001** — The system shall recognize the Crockford base32 alphabet as the 32 characters `0123456789ABCDEFGHJKMNPQRSTVWXYZ` (case-insensitive on input).
- [x] **FMT-B32-002** — When validating a string against the Crockford base32 alphabet, the system shall accept characters in either case.
- [x] **FMT-B32-003** — When validating a string whose length differs from the expected length, the system shall return an error identifying the expected and actual lengths, and shall not check character membership.
- [x] **FMT-B32-004** — When validating a string whose length matches the expected length and which contains a character outside the Crockford base32 alphabet, the system shall return an error identifying the offending character and its 0-based char index within the string.
- [x] **FMT-B32-005** — The system shall reject the characters `I`, `L`, `O`, and `U` (in either case) as invalid Crockford base32 characters, without folding them to `1` or `0`.
- [x] **FMT-B32-006** — When generating a random Crockford base32 string of length `n`, the system shall emit `n` characters drawn uniformly from the canonical alphabet, all in lowercase.
- [x] **FMT-B32-007** — When generating a random Crockford base32 string, the system shall draw all randomness from a caller-supplied random number generator.

## Bullet line markers

- [ ] **FMT-MARK-001** — A bullet's `[id]` marker shall match `<3-char-prefix>-<3-char-suffix>` where both segments use the Crockford base32 alphabet.
- [ ] **FMT-MARK-002** — A bullet's `[by:<name>]` marker shall accept names matching `[A-Za-z0-9_.-]+`.
- [ ] **FMT-MARK-003** — A bullet's `[blocked-by:<id>]` marker shall accept ids matching the same format as FMT-MARK-001.
- [ ] **FMT-MARK-004** — When serializing a bullet, the system shall emit markers in the canonical order: `[id]`, `[in-progress]`, `[by:<name>]`, `[blocked-by:<id>]`, then the title.
- [ ] **FMT-MARK-005** — When serializing a bullet, the system shall separate adjacent markers with a single space.
- [ ] **FMT-MARK-006** — When parsing a bullet, the system shall treat unrecognized `[...]` tokens at the front of the body as part of the title.
- [ ] **FMT-MARK-007** — In v1 the system shall preserve only the first `[blocked-by:...]` marker if multiple are present on a single bullet.

## Empty and malformed bullets

- [ ] **FMT-PARSE-006** — When a bullet line has no title text after markers, the system shall print a warning, leave the line untouched, and skip it for ID assignment and notes extraction.

## Item files

- [ ] **FMT-ITEM-001** — When the system creates an item file at `backlog/items/<id>.md`, it shall write a YAML frontmatter block with `id: <id>` followed by the body content.
- [ ] **FMT-ITEM-002** — When the system appends notes to an existing item file, it shall preserve the existing frontmatter unchanged and append a single blank line followed by the new notes content.
- [ ] **FMT-ITEM-003** — The system shall not validate that an existing item file's frontmatter `id` matches the filename.

## Tombstone file

- [x] **FMT-TOMB-001** — `backlog/.used-ids` shall be a newline-delimited list of full IDs.
- [x] **FMT-TOMB-002** — When `backlog/.used-ids` is missing, the system shall treat it as empty and create it on first write.
- [x] **FMT-TOMB-003** — The system shall deduplicate IDs when reading `backlog/.used-ids`.
- [x] **FMT-TOMB-004** — When reading `backlog/.used-ids`, the system shall reject any line that, after trimming surrounding ASCII whitespace, does not match the Crockford `<3>-<3>` ID format, identifying the 1-based line number of the offending content.
- [x] **FMT-TOMB-005** — When reading `backlog/.used-ids`, the system shall normalize each ID to lowercase before insertion into the returned set.
- [x] **FMT-TOMB-006** — When appending to `backlog/.used-ids` and the existing file does not end with a `\n` byte, the system shall write a leading `\n` before the appended content.
- [x] **FMT-TOMB-007** — When appending to `backlog/.used-ids` and the parent `backlog/` directory does not exist, the system shall return a distinct error identifying the missing project directory and shall not create the directory.
- [x] **FMT-TOMB-008** — When appending IDs to `backlog/.used-ids`, the system shall write each supplied ID as its own line in input order, performing no deduplication against existing contents or within the batch; an empty input shall leave the filesystem unchanged.
- [x] **FMT-TOMB-009** — When reading `backlog/.used-ids` and the parent `backlog/` directory does not exist, the system shall return the same distinct missing-project-directory error as the writer (FMT-TOMB-007), rather than treating the file as empty. A missing file within an existing `backlog/` remains empty per FMT-TOMB-002.

## Project config

- [x] **FMT-CFG-001** — `backlog/vat.toml` shall contain `[project]` with `id` set to a 3-character Crockford base32 string.
- [x] **FMT-CFG-002** — When `vat.toml` is missing or `project.id` is invalid, the system shall abort with an error pointing the user at `vat init`.
- [x] **FMT-CFG-003** — When writing `vat.toml`, the system shall preserve unknown sections and keys.

## Global config

- [ ] **FMT-USR-001** — User config shall live at `$XDG_CONFIG_HOME/vat/config.toml`, falling back to `~/.config/vat/config.toml` when `XDG_CONFIG_HOME` is unset.
- [ ] **FMT-USR-002** — `user.name` shall be optional in the user config file.

## Line endings and whitespace

- [ ] **FMT-WS-001** — When reading any VAT-managed file, the system shall normalize CRLF line endings to LF.
- [ ] **FMT-WS-002** — When serializing a bullet line, the system shall strip trailing whitespace.
