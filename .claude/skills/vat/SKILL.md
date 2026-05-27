---
name: vat
description: Manage a VAT backlog — assign IDs to new bullets (sync), claim tasks (start), block/unblock, mark done, and read/write project & user config. Use whenever the user says "vat sync", "claim foo-7k2", "mark X done", "ID that bullet", "extract notes", or otherwise asks for any operation on `backlog/backlog.md`, `backlog/items/`, `backlog/vat.toml`, `backlog/.used-ids`, or `~/.config/vat/config.toml`. Supports both local mode (backlog tracked in the project repo) and remote-backed mode (backlog is a managed clone of a dedicated repo, with race-free `git push` claims across many concurrent agents). Stopgap implementation while the Rust binary is built — drop this skill into any project to use VAT before the binary lands.
argument-hint: <subcommand> [args...]
allowed-tools: Read, Write, Edit, Bash
---

# VAT skill

VAT (Versioned Addressable Tasks) is a tiny per-project backlog tool. State is plain markdown in the repo; this skill is a stopgap implementation that performs the same operations the eventual Rust binary will.

## Invocation

The user's request is: **$ARGUMENTS**

If `$ARGUMENTS` is empty, the request is in the surrounding conversation (e.g., the user said "claim foo-7k2" or hand-edited `backlog/backlog.md` and asked you to tidy it). Map the request to one of the procedures below.

## What VAT is

A backlog file at `backlog/backlog.md` holds a flat list of `-` bullets. The user jots bullets as they think of them, then runs `vat sync` to assign each bullet a short stable ID (`foo-7k2`) and move any indented notes into `backlog/items/<id>.md`. Once a bullet has an ID, it's *addressable*: `vat start foo-7k2` claims it, `vat done foo-7k2` removes it. Concurrency across collaborators is delegated to git merge.

Files VAT owns:

```
backlog/
  backlog.md          # flat list of bullets (canonical state)
  vat.toml            # project config — project.id (3-char prefix)
  .used-ids           # newline-delimited tombstone list of every id ever issued
  README.md           # one-shot human explainer (written by `vat init` only)
  items/
    <id>.md           # per-task notes (only when notes exist)
~/.config/vat/config.toml   # user config — user.name (or $XDG_CONFIG_HOME/vat/config.toml)
```

Nothing outside this set is ever written.

## Storage modes

VAT runs in one of two modes, **auto-detected** by testing `test -d backlog/.git` (a dedicated clone always has `.git` as a *directory*; `test -e` would false-positive on a git worktree, whose `.git` is a *file*):

### Local mode (default — present behaviour)

`backlog/` is a plain directory tracked by the **project's own repo**. No network, ever. Concurrency across collaborators is resolved by ordinary git merge of the project repo, exactly as the HLD describes. None of the refresh/push machinery below applies — the procedures run as written.

### Remote-backed mode

`backlog/` is a **managed clone of a dedicated backlog git repo** — it has its own `backlog/.git`, and the project repo **gitignores** `backlog/`. The backlog therefore lives outside the project's git history and governance: no submodule, no pinned commit, no approval gate on the project repo, always the freshest copy.

In this mode every command first **refreshes** the clone from its remote (so it reads the freshest state), and every *mutating* command **commits and pushes** inside an **atomic claim loop**. `git push` is the compare-and-swap: the first push wins, and a single `backlog.md` is enough to make `vat start` a race-free claim across an unbounded number of concurrent agents — no server, no daemon.

`backlog/.git` is owned by git, never hand-written. In remote-backed mode VAT may also append a `/backlog/` line to the **project** `.gitignore` (at `vat init --remote` time only).

Remote-backed mode requires `git` to be available in `PATH`.

## When to invoke

- `vat sync` / "sync the backlog" / "assign IDs to new bullets" / "extract notes for that bullet"
- `vat start <id>` / "claim foo-7k2" / "I'll take foo-7k2"
- `vat block <id> <blocker-id>` / `vat unblock <id>`
- `vat done <id>` / "mark foo-7k2 done"
- `vat init [<prefix>]` / "set up vat in this repo"
- `vat config get|set <key> [<value>]`

If the user hand-edited `backlog/backlog.md` and asks you to "tidy" or "commit" it, run `sync` first.

## File formats

### `backlog/backlog.md`

**Optional YAML frontmatter** at the very top, delimited by lines that consist solely of `---`:

```
---
version: 1
---
```

- `version` (integer): schema major version. `vat init` writes `1`. If the file's `version` exceeds `1`, abort every command with: `backlog file is version <N>; this skill supports up to version 1. Please upgrade vat.`
- Missing/empty frontmatter is treated as version 1.
- Unknown frontmatter keys are preserved on rewrite.

**Body regions.** After any frontmatter, the body has two regions separated by the **first** standalone `---` line (the line consists solely of `---`, optionally surrounded by whitespace). Other CommonMark thematic-break forms (`***`, `___`) are NOT recognized.

- **Parsed region**: above the separator. VAT reads, mutates, and writes only this region.
- **Freeform region**: from the separator onward (including the separator line itself). Preserve byte-for-byte.
- If no separator exists, the entire file is the parsed region.

**Parsed region grammar.** A preamble (any content — blank lines, headings, paragraphs — before the first bullet) followed by a sequence of task entries.

A **task entry** is:
1. A **bullet line**: starts at column 0 with `- ` (hyphen + single space).
2. Optional **note lines**: any subsequent lines that aren't a bullet line, up to the next bullet at column 0 or end of parsed region.

Only `-` is a bullet marker. Lines starting with `*` or `+` are notes (or preamble if no bullet has appeared yet). A paragraph at column 0 between two bullets attaches to the *first* bullet, not the second.

Preamble is preserved byte-for-byte and emitted at the top of the parsed region on rewrite.

### Bullet line canonical form

```
- [<id>] [in-progress] [by:<name>] [blocked-by:<id>] <title>
```

- `[<id>]` — required after `vat sync`. Format `<project>-<suffix>`: 3-char Crockford base32 prefix from `vat.toml` + `-` + 3 chars Crockford base32. Pre-sync, may be absent.
- `[in-progress]` — literal, optional.
- `[by:<name>]` — `<name>` matches `[A-Za-z0-9_.-]+`. Optional.
- `[blocked-by:<id>]` — `<id>` matches the project ID format. Optional. Multiple `[blocked-by:...]` markers not supported in v1; preserve only the first.
- `<title>` — rest of the line, trimmed. Required, non-empty. When the bullet has a corresponding `backlog/items/<id>.md`, the title ends with the literal suffix ` (see ./items/<id>.md)` (single leading space, `./` prefix). The suffix is part of the title — round-tripped verbatim, managed by `vat sync`. NOT a marker.

Markers are always front-loaded in the order shown. `vat sync` normalizes order and spacing; other commands write markers in canonical position directly.

**Bullet parsing rules:**
- Markers matched left-to-right. As soon as the parser hits a `[...]` token that doesn't match a known marker pattern, the rest of the line is the title.
- Unknown bracketed tokens at the front are part of the title (so `- [TODO] thing` keeps `[TODO]` in the title).
- Whitespace between markers normalized to a single space on serialize. Trailing whitespace stripped.
- Empty bullet (`-` with nothing after) → warning, leave line untouched, skip the entry.

**Notes** are the lines between a bullet and the next bullet (or end of parsed region). On `vat sync`, notes are extracted into `backlog/items/<id>.md` and the bullet collapses to one line.

### `backlog/items/<id>.md`

```
---
id: <full-id>
---

<body>
```

- Frontmatter `id` MUST equal the filename stem (not validated on append — filename is the source of truth).
- Body is the verbatim notes, with the minimum common leading whitespace stripped, leading/trailing blank lines trimmed.
- On re-sync with new notes appended: append a single blank line, then the new notes (indentation-stripping applied to the new notes alone). Frontmatter untouched.
- Created lazily; deleted on `vat done`.

### `backlog/.used-ids`

Plain text, newline-delimited, one full ID per line (e.g., `foo-7k2`). Append-order. No comments or blank lines. Dedup on read. Append on every new ID assignment and on every `vat done`. If missing, treat as empty.

### `backlog/vat.toml`

```toml
[project]
id = "foo"   # exactly 3 characters, Crockford base32 alphabet
```

- `project.id` is required. Validate on every command; missing/invalid is a hard error pointing at `vat init`.
- Preserve unknown sections/keys on rewrite.

### `~/.config/vat/config.toml`

```toml
[user]
name = "jared"
```

- Path: `$XDG_CONFIG_HOME/vat/config.toml` if set, else `~/.config/vat/config.toml`.
- `user.name` is optional in the file; commands that require it (`vat start`) error with a pointer to `vat config set user.name <name>`.

## Crockford base32 alphabet

`0123456789abcdefghjkmnpqrstvwxyz` — 32 chars, no `i`/`l`/`o`/`u`. Inputs accepted in either case; everything VAT writes is lowercase.

**Strict validation.** Reject any character outside the alphabet, including ambiguous `I`/`L`/`O`/`U`. Do NOT silently fold `I/L → 1` or `O → 0` per Crockford's lenient decoder hint — for short user-typed identifiers, a hard error pointing at the bad character is more helpful than a quiet rewrite.

VAT never decodes Crockford base32 to bytes — IDs are opaque tokens.

## Common machinery

Before reading `backlog/backlog.md`:

1. Detect optional YAML frontmatter at the very top.
2. If `version > 1`, abort with no writes.
3. Split body at the first standalone `---` line (after frontmatter) into parsed / freeform regions.
4. Parse parsed region into preamble + entries.

All writes are whole-file rewrites. Normalize CRLF → `\n` on read. Output always ends with a single trailing `\n`.

All-or-nothing: if any error occurs during parsing/ID-generation/validation, no files are mutated.

### Remote refresh (remote-backed mode only)

Before reading state in **any** command, sync the clone to the freshest remote state:

```bash
BR=$(git -C backlog symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's@^origin/@@')
BR=${BR:-main}
git -C backlog fetch --quiet origin "$BR"
git -C backlog reset --hard --quiet "origin/$BR"
```

`reset --hard` is safe because the clone is a throwaway working copy VAT fully owns — it holds nothing that isn't (or won't be) on the remote. That is exactly why the project `.gitignore`s `backlog/`: no other tooling depends on its working tree.

### Atomic push / claim loop (remote-backed mode only)

Every **mutating** command runs its read→decide→mutate→write step *inside* this loop:

```
MAX = 5
attempt = 0
loop:
    refresh                       # fetch + reset --hard origin/$BR  → freshest state
    re-read backlog state from the refreshed files
    evaluate the command's preconditions on THIS fresh state:
        if a terminal precondition now holds (see table) →
            report that terminal result and STOP. Do NOT push, do NOT retry.
    apply the local mutation (write files)
    git -C backlog add -A
    git -C backlog commit --quiet -m "<commit message>"   # see message table below
    run  git -C backlog push origin HEAD:$BR  (capture exit code + stderr)
    if push succeeded:
        report success; STOP
    else if stderr matches "rejected" or "non-fast-forward":   # contention: remote head moved
        attempt += 1
        if attempt >= MAX:
            report "could not sync <id> after MAX attempts; try again" (exit 2); STOP
        sleep a random jitter in [0, min(2^attempt × 0.1s, 2s)]   # disperse herds
        continue                   # refresh re-runs the decision on the winner's state
    else:                          # infrastructure: network/auth/quota — retrying is pointless
        report "push failed (not a race): <raw git stderr>" (exit 2); STOP
```

Distinguishing the two push-failure classes matters: a contention rejection (`rejected` / `non-fast-forward`) is a normal race and retries; anything else (network unreachable, auth rejected, quota) fails fast with the raw git error, so the user isn't misled into thinking they lost a race after MAX wasted attempts.

**First push wins; losers re-decide; never hand-merge.** On a rejected push VAT does *not* rebase the stale edit — it `reset --hard`s to the winner's state and re-runs the command's decision from scratch. So there is never a textual merge conflict, not even a false one between two *different* tasks edited on nearby lines. The only thing that ends a retry early is a **terminal precondition** — which is precisely the lock telling you that you lost.

**Per-command terminal preconditions** (re-checked after each refresh; if true, stop without pushing):

| Command | Terminal precondition on fresh state | Terminal report |
|---|---|---|
| `start <id>` | bullet already has `[in-progress]` or `[by:…]` | `lost claim: <id> already claimed by <name>` (exit 1) |
| `done <id>` | `<id>` no longer present (done elsewhere) | `already done: <id>` (exit 0) |
| `block`/`unblock` | request already satisfied (idempotent) | the command's normal no-op success |
| `sync` | — none; always re-runs against fresh content | — |

**Commit messages** (the `<commit message>` in the loop) are fixed per command, so the remote history is predictable and auditable:

| Command | Commit message |
|---|---|
| `sync` | `vat sync` |
| `start <id>` | `start <id>` |
| `done <id>` | `done <id>` |
| `block <id> <blocker>` | `block <id> <blocker>` |
| `unblock <id>` | `unblock <id>` |
| `init` (scaffold) | `chore: vat init` |

`sync` has no terminal precondition: each retry re-runs the full sync against the fresh content. IDs it generates may differ between discarded attempts (random); only the pushed attempt's IDs reach `.used-ids`, so this is harmless.

**Non-mutating reads** (`vat config get`, plain listing) only **refresh**, then read — no commit, no push, no loop.

## Procedures

In **remote-backed mode**, every mutating procedure below executes inside the **atomic claim loop** above: the loop owns refresh / commit / push / retry, and the procedure body *is* the "re-read + decide + mutate + write" step (with its terminal precondition from the table). In **local mode** the procedures run exactly as written, with no network.

### `vat sync`

The only command that mutates the structure of `backlog.md`. Idempotent.

1. Load `backlog/vat.toml`; require `project.id`. Missing/invalid → abort with init pointer.
2. Read `backlog.md` (missing → abort with init pointer). Run version check. Split frontmatter / parsed / freeform.
3. Parse parsed region into preamble + entries.
4. Read `.used-ids` (missing → empty). Build `used` = those ids ∪ every id present on a bullet in the parsed region.
5. Two bullets sharing the same `[id]` → abort with no writes; message points at the offending lines.
6. For each entry in order:
   - **No `[id]`**: generate `<project.id>-<3 random Crockford chars>`; retry while in `used`; cap 100 retries (else abort with no writes). Add to `used` and to a `to_append` list. Insert `[id]` at the front of the bullet.
   - **`[id]` with prefix ≠ `project.id`**: warn, leave marker as-is.
   - Normalize markers to canonical order with single-space separators.
   - **Notes handling**:
     - Strip the minimum common leading whitespace across note lines; trim leading/trailing blank lines.
     - If the result is non-empty:
       - If `backlog/items/<id>.md` does not exist: create it with frontmatter `---\nid: <id>\n---\n` then a blank line then the stripped notes then a trailing newline.
       - Else: append `\n<stripped>\n` to the existing body, leaving frontmatter untouched.
     - Drop the note lines from the entry regardless (so the entry serializes as one line).
     - If notes were only whitespace, drop them but do NOT create or modify any item file.
   - **Pointer suffix**:
     - If `backlog/items/<id>.md` exists (pre-existing or just written) and the title doesn't already end with ` (see ./items/<id>.md)`, append it.
     - If the file does NOT exist, do not add the suffix and do not strip an existing one. Sync never removes information.
7. Serialize: original frontmatter (preserve unknown keys verbatim) + preamble (verbatim) + one normalized line per entry + freeform region (verbatim, including the `---` separator line).
8. **No-op**: if serialized output is byte-identical to input, skip the write to `backlog.md`.
9. Otherwise write `backlog.md`. Then append each new id (from `to_append`) on its own line to `.used-ids`. Create `backlog/items/` lazily if any item-file write needs it.

**Idempotence.** After one successful sync: every entry has an `[id]`; no entry has note lines; markers are canonical; assigned ids are in `.used-ids`; entries whose id has an item file end with the pointer suffix. A second run finds nothing to do and skips the write.

**Edge behaviors:**
- Bullet has id, no notes → pass-through (only marker reorder).
- Bullet has id, with notes → append to existing item file (or create); collapse to one line.
- Bullet without id, with notes → assign id; create item file.
- Bullet has id but its `(see ./items/<id>.md)` suffix is missing/hand-edited away → re-append (only if the item file exists).
- Item file does not exist but a `(see ...)` suffix is present (user manually deleted file) → leave suffix alone.
- Empty bullet (`-` alone) → warn, skip, do NOT consume the lines after it as notes.
- Item file exists but no bullet references it → leave alone (no GC in v1).
- Two bullets with the same id → abort, no writes.
- No `---` separator → entire file is parsed; do NOT add one.
- Dangling `[blocked-by:X]` whose X isn't present → leave alone. (Only `vat done` strips blockers.)
- Trailing whitespace on bullet lines → stripped on serialize.

### `vat start <id>`

1. Load user config. Require `user.name`. If missing: `set user.name first: vat config set user.name <name>`.
2. Run version check; locate the entry by `[id]`. Not found → `unknown id: <id>`.
3. If bullet has `[in-progress]` or `[by:...]` → `<id> already claimed by <name>` (or `... already in progress` if only `[in-progress]` present from a hand-edit). Both forms count as "claimed". In remote-backed mode this is the loop's **terminal precondition**: always report `lost claim: <id> already claimed by <name>` — uniformly, regardless of which markers are present (a hand-edited `[in-progress]` on the remote is still a remote win) — and stop (no push, no retry). This is the first-push-wins lock signalling you lost.
4. Insert `[in-progress]` and `[by:<user.name>]` in canonical position.
5. Re-serialize the parsed region and write. (Remote-backed mode: the surrounding claim loop then commits with message `start <id>` and pushes; a rejected push refreshes and re-runs from step 2.)

### `vat block <id> <blocker-id>`

1. Locate entry. Refuse if `id == blocker-id`.
2. `<blocker-id>` must appear somewhere in the parsed region (any bullet). Else `unknown blocker: <blocker-id>`.
3. Already `[blocked-by:<blocker-id>]` (same blocker) → no-op success.
4. Existing `[blocked-by:<other>]` → replace.
5. Else insert in canonical position. Write.

(v1 supports a single blocker per task. No cycle detection.)

### `vat unblock <id>`

1. Locate entry. If no `[blocked-by:...]`, no-op success (exit 0).
2. Strip the marker. Write.

### `vat done <id>`

1. Locate entry.
2. Remove the bullet line. If removing it would leave a double blank, also drop one of the surrounding blank lines.
3. If `backlog/items/<id>.md` exists, delete it.
4. Append `<id>` to `.used-ids` if not already present.
5. Walk remaining entries; strip `[blocked-by:<id>]` from any that reference this id. (Auto-unblock.)
6. Write `backlog.md`.

`vat done` on a blocked task is allowed (no warning).

### `vat init [<prefix>]` / `vat init --remote <url> [<prefix>]`

Common guard: if `backlog/` exists → `backlog/ already exists; vat is initialized`.

#### Local form: `vat init [<prefix>]`

1. Resolve prefix: argument takes precedence; otherwise prompt the user.
2. Validate prefix: exactly 3 chars, all in Crockford base32 (case-insensitive); store lowercase.
3. Create `backlog/` and write:
   - `backlog/vat.toml` containing `[project]\nid = "<prefix>"\n`.
   - `backlog/backlog.md` containing only:
     ```
     ---
     version: 1
     ---
     ```
   - `backlog/.used-ids` empty.
   - `backlog/README.md` from the template below.

#### Remote-backed form: `vat init --remote <url> [<prefix>]`

1. Clone the dedicated backlog repo into `backlog/`: `git clone <url> backlog`.
2. Ensure the **project** repo ignores the clone: if none of `backlog`, `backlog/`, or `/backlog/` already appears in the project `.gitignore`, append a line `/backlog/` (create `.gitignore` if absent). **Do not append** if `.gitignore` contains a negation for the path (e.g. `!backlog/`): appending after a negation silently overrides it (last gitignore rule wins) and could drop files the user meant to track — instead warn the user and leave `.gitignore` untouched. This is the only time VAT writes outside the file set.
3. Determine whether the clone is empty (a fresh remote: `git -C backlog rev-parse HEAD 2>/dev/null` fails) or already a VAT backlog:
   - **Empty remote** → `<prefix>` is **required** (error `vat init --remote needs a 3-char prefix for a fresh backlog` if absent). Validate it, then scaffold the same four files as the local form *inside* `backlog/`, and commit + push them via the atomic claim loop (commit message `chore: vat init`).
   - **Non-empty remote** → it is already a VAT backlog; do **not** scaffold. Refresh it, then verify `backlog/vat.toml` has a valid `project.id`; if not, abort with `cloned repo is not a VAT backlog (missing project.id)`. Ignore any `<prefix>` argument (warn if it conflicts with the repo's `project.id`).

**Partial-failure cleanup:** step 2 may have already mutated `.gitignore` before step 3 runs. If step 3 aborts (e.g. `cloned repo is not a VAT backlog`), `backlog/` is left on disk in an undefined state — the user must `rm -rf backlog/` (and revert the `.gitignore` line) before retrying, otherwise the next `vat` command will detect `backlog/.git` and enter remote-backed mode against a broken clone.

From this point all commands auto-detect remote-backed mode via `backlog/.git`.

**`backlog/README.md` template** (substitute `<prefix>`):

```markdown
# Backlog

This directory is managed by [VAT](https://github.com/jmbeach/vat) (Versioned Addressable Tasks) — a tiny tool for capturing tasks as plain markdown.

## Files

- `backlog.md` — the flat list of tasks. Jot bullets here as you think of them.
- `vat.toml` — project config (the 3-char ID prefix for this repo: `<prefix>`).
- `.used-ids` — tombstone list of every ID ever issued. Committed. Don't hand-edit.
- `items/<id>.md` — per-task notes; created when a task has notes, deleted when the task is `done`.

## Workflow

1. Type new bullets into `backlog.md`:
   ```
   - rewrite the cache layer
       why: the LRU is thrashing on hot keys
   ```
2. Run `vat sync` to assign IDs and tuck notes into `items/<id>.md`.
3. Claim a task with `vat start <id>`. Mark it complete with `vat done <id>`.

This README is written once at init time. VAT never reads or rewrites it — feel free to edit or delete.
```

This README is written once at init and never read or rewritten by VAT after that.

### `vat config get <key>`

Supported keys: `user.name`, `project.id`. Print the value to stdout, or print nothing and exit non-zero if unset.

### `vat config set <key> <value>`

- `user.name`: writes to global config (creating `~/.config/vat/config.toml` and parent dirs as needed). Format `[user]\nname = "<value>"\n`. Preserve unknown sections/keys.
- `project.id`: writes to `backlog/vat.toml`. Validate (3 chars, Crockford base32). **Refuse** if any id in `backlog.md` or `.used-ids` uses the old prefix — changing prefix mid-project would orphan ids. No `--force`. Users with a real need can edit `vat.toml` directly.
- Other keys → `unknown config key: <key>`.

## Files this skill is allowed to touch

Exactly these — never anything else:

- `backlog/backlog.md`
- `backlog/items/<id>.md`
- `backlog/items/` (created lazily)
- `backlog/.used-ids`
- `backlog/vat.toml`
- `backlog/README.md` (init only)
- `~/.config/vat/config.toml` (or `$XDG_CONFIG_HOME/vat/config.toml`)
- `backlog/.git` — only via `git` (clone/fetch/reset/commit/push), never hand-edited (remote-backed mode)
- the project `.gitignore` — a single appended `/backlog/` line, `vat init --remote` only (remote-backed mode)

## Output

- Success: one terse line, e.g.:
  - `assigned 3 ids: foo-7k2, foo-9hf, foo-b2x`
  - `started foo-7k2`
  - `done foo-7k2 (cleared 2 blockers)`
  - `unchanged` (sync no-op)
- Warnings (empty bullet, foreign-prefix id) — surface to the user but do not abort.
- Errors → terse message + no writes. Use the exact wording specified above where given (e.g., `unknown id: <id>`).

## Exit-code semantics (for parity with the binary)

- `0` — success (including no-op cases).
- `1` — user-facing error (unknown id, missing config, validation failure).
- `2` — internal error (file IO, parse failure that shouldn't happen).
