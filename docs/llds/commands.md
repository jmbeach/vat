# LLD: Single-entry and config commands

Covers every command other than `vat sync`. These are smaller — most are a parse, a single-line edit, and a write. See [backlog-format LLD](./backlog-format.md) for the file grammar and [sync LLD](./sync.md) for the structural-mutation command.

## Common machinery

All commands that read `backlog.md` first run the **version check**: parse the optional YAML frontmatter, and if `version` is greater than the CLI's supported major (currently 1), abort with the standard upgrade message before doing any other work.

All bullet-mutating commands (`start`, `block`, `unblock`, `done`) share a helper:

```
fn find_entry(id) -> (parsed_region, entry_index)
```

It loads `backlog.md`, parses the parsed region, and locates the task entry whose bullet has the given `[id]`. If not found, error with `unknown id: <id>` and exit non-zero with no writes.

After mutation, the helper serializes the parsed region back through the same emitter `sync` uses, so canonical marker order is preserved by construction.

## `vat init`

1. If `backlog/` exists, error: `backlog/ already exists; vat is initialized`.
2. Determine project prefix: argument (`vat init <prefix>`) takes precedence; otherwise prompt interactively.
3. Validate prefix: exactly 3 chars, all in Crockford base32 alphabet (`0123456789ABCDEFGHJKMNPQRSTVWXYZ`, case-insensitive — store lowercase).
4. Create `backlog/`, write `backlog/vat.toml`, create `backlog/backlog.md` containing only a YAML frontmatter block with `version: 1`, create empty `backlog/.used-ids`, and write `backlog/README.md`.

   The initial `backlog.md` looks like:

   ```
   ---
   version: 1
   ---
   ```

   `backlog/README.md` is written from a static template baked into the binary via `include_str!` (`readme_template::BACKLOG_README_TEMPLATE` in `src/readme_template.rs`, sourced from `src/templates/README.md.tmpl`). `readme_template::render(prefix)` substitutes the literal `{prefix}` placeholder with the project prefix; `init` writes the rendered string. It explains:

   - What VAT is and where to get it (link to the project repo / install command).
   - The purpose of the `backlog/` directory and a one-line description of each file: `backlog.md`, `vat.toml`, `.used-ids`, `items/`.
   - The basic workflow: jot bullets in `backlog.md`, run `vat sync`, claim with `vat start`, complete with `vat done`.
   - That the README itself is not managed by VAT after init — users may edit or delete it freely.

   VAT writes the README only at init time. It is never read, validated, or rewritten by any subsequent command.

## `vat start <id>`

1. Resolve user.name from `~/.config/vat/config.toml`. If missing, error with `set user.name first: vat config set user.name <name>`.
2. `find_entry(id)`.
3. If the bullet has either `[in-progress]` or `[by:...]`, error: `<id> already claimed by <name>` (or `<id> already in progress` if only `[in-progress]` is present from a hand-edit).
4. Add both `[in-progress]` and `[by:<user.name>]` markers in canonical position.
5. Write the file back.

## `vat block <id> <blocker-id>`

1. Self-block guard: error if `id == blocker-id` (case-insensitive). This is a pure-argument check independent of file state, so it runs before any lookup — the error names the real mistake (the same id typed twice) even when that id is absent from the backlog.
2. `find_entry(id)`.
3. Verify `<blocker-id>` matches a **well-formed** bullet in the parsed region. If not, error: `unknown blocker: <blocker-id>`. The bullet must parse so the blocker id is known to be a valid `<3>-<3>` id before it is written into a `[blocked-by:...]` marker; a marker pointing at an id the emitter would reject must never be produced.
4. If the entry already has `[blocked-by:<blocker-id>]` (same blocker), no-op (success) — no write.
5. Otherwise set `[blocked-by:<blocker-id>]` in canonical position, replacing any existing `[blocked-by:<other>]` (v1 supports a single blocker per task), and write.

## `vat unblock <id>`

1. `find_entry(id)`.
2. If no `[blocked-by:...]` marker present, no-op (success, exit 0).
3. Strip the marker. Write.

## `vat done <id>`

1. `find_entry(id)`.
2. Remove the entire bullet line (and any blank line immediately following, if it would leave a double blank).
3. If `backlog/items/<id>.md` exists, delete it.
4. Append `<id>` to `.used-ids` if not already present.
5. Walk the remaining parsed region; for every other entry, if it has `[blocked-by:<id>]`, strip that marker. (Auto-unblock per HLD §5.)
6. Write `backlog.md`.

The auto-unblock pass is the one place a `done` mutates more than the matching bullet line, but the mutation is bounded (only `[blocked-by:<id>]` markers, only on other entries) and falls naturally out of the same parse-mutate-emit machinery.

## `vat config get <key>`

- Supported keys: `user.name`, `project.id`.
- `user.name` reads from global config; `project.id` reads from `backlog/vat.toml`.
- Print the value to stdout, or print nothing and exit non-zero if unset.

## `vat config set <key> <value>`

- `user.name`: writes to global config, creating `~/.config/vat/config.toml` and parent dirs if needed.
- `project.id`: writes to `backlog/vat.toml`. Validates the value (3 chars, Crockford base32). Refuses if any IDs in `backlog.md` or `.used-ids` use the old prefix — changing prefix mid-project would orphan IDs. v1 has no escape hatch; if a user really needs to rewrite the prefix they can edit `vat.toml` directly.
- Other keys: error with `unknown config key: <key>`.

## v1 limits and non-behaviors

- **`vat done` on a blocked task is allowed.** No warning. The user accepted the work was done; the blocker arrow is removed by the auto-unblock pass anyway.
- **No cycle detection in `vat block`.** A → blocked-by → B → blocked-by → A is allowed. Markers are visible in the file; humans will notice. Detection can be added later.
- **`vat init` refuses if `backlog/` exists.** No "adopt existing file" mode in v1. Users with an existing `backlog.md` can hand-create `vat.toml` and `.used-ids`.
- **No file locking.** Two `vat` processes racing on the same repo can clobber each other's writes — same risk as two editors saving the same file. Resolution is post-hoc via git.
- **No `--force`, `--dry-run`, or other flag overrides.** Commands either succeed or refuse; users wanting to override invariants edit the underlying files by hand.

## Exit codes

- `0`: success (including no-op cases).
- `1`: user-facing error (unknown id, missing config, validation failure).
- `2`: internal error (file IO, parse failure that shouldn't happen).

## Decisions & alternatives

- **Single blocker per task.** v1 supports one `[blocked-by:...]` marker. Multiple blockers were considered but add complexity to both the marker parser and the `done` auto-unblock pass with little benefit at this scale. Can be lifted later.
- **`vat done` deletes the item file rather than archiving it.** Aligns with HLD: git history is the record. Considered moving to `backlog/items/.archive/<id>.md` but that's a second source of truth that nothing reads.
- **`vat config set project.id` is guarded with no escape hatch.** Because IDs embed the prefix, changing it mid-project produces stale IDs in tombstones and in commit messages. The command refuses outright once any IDs are in use; users with a legitimate need can edit `vat.toml` by hand.
- **`vat unblock` is a no-op if not blocked.** Considered erroring; chose silent success because it's the more user-friendly behavior for a command that is essentially "ensure no blocker."
