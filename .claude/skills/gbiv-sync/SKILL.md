---
name: gbiv-sync
description: Copy the current user's assigned VAT tasks from `backlog/backlog.md` into `GBIV.md` at the repo root. Use when the user says "update gbiv", "refresh my tasks", "sync gbiv", or asks to mirror their claimed backlog items into a personal task list.
argument-hint: (no args)
allowed-tools: Read, Write, Bash
---

# gbiv-sync skill

Copies the tasks **assigned to the current user** (those with `[by:<user.name>]`) from `backlog/backlog.md` into `GBIV.md` at the repo root, with VAT-specific markers stripped so the file reads as a clean personal task list.

## Inputs

- `backlog/backlog.md` — VAT backlog (canonical state).
- `~/.config/vat/config.toml` (or `$XDG_CONFIG_HOME/vat/config.toml`) — must contain `[user] name = "<name>"`. Missing → abort with: `set user.name first: vat config set user.name <name>`.

## Output

- `GBIV.md` at the repo root. Only the **parsed region** (everything above the first standalone `---` line) is replaced; everything from the `---` separator onward is preserved byte-for-byte. Existing `[color]` tags in the parsed region are carried over onto the matching task (keyed by VAT id) so a re-sync never discards `gbiv-colorize`'s work. Created if missing — when creating, the file is just the bullet list with a single trailing newline (no separator added).

## Procedure

1. Read `user.name` from the user config. Abort if unset (see message above).
2. Read `backlog/backlog.md`. Detect optional YAML frontmatter; parse only the region above the first standalone `---` separator (if any).
3. From the parsed region, collect every bullet line (`- ` at column 0) whose markers include `[by:<user.name>]` (exact match, case-sensitive on the name).
4. For each matching bullet:
   - Strip the `[in-progress]` marker if present.
   - Strip the `[by:<user.name>]` marker.
   - Leave `[id]` and any `[blocked-by:<id>]` markers in place.
   - Rewrite the pointer suffix `(see ./items/<id>.md)` to `(see backlog/items/<id>.md)`. (Single replacement; the suffix appears at most once per line.)
   - Normalize whitespace between remaining tokens to single spaces; strip trailing whitespace.
5. If `GBIV.md` exists, split it at the first standalone `---` line into a parsed region (above) and a freeform region (the `---` line and everything after). Otherwise both regions are empty.
6. **Preserve color tags.** Scan the existing parsed region (from step 5) for bullets and build a map of `[id] → [color]`: for each bullet, if its leading marker after `- ` is a ROYGBIV color tag (`[red]`, `[orange]`, `[yellow]`, `[green]`, `[blue]`, `[indigo]`, `[violet]`) and the bullet also carries a `[<id>]` marker, record `id → color`. When emitting each task's bullet (step 7), if that task's `[id]` has a recorded color, prepend the `[color]` tag immediately after `- `, before the `[id]` marker. Tasks with no recorded color are emitted untagged. Color tags are never invented here — only carried over; assigning colors to new bullets is `gbiv-colorize`'s job.
7. Replace the parsed region with one bullet per matching task in original order — each carrying its preserved `[color]` tag (if any) from step 6 — on its own line, followed by a single blank line before the `---`. Preserve the freeform region byte-for-byte. If `GBIV.md` does not exist, write only the bullets with a single trailing newline. If no tasks match and `GBIV.md` does not exist, do not create it; report the no-tasks message and exit.

8. Report a one-line summary: `wrote GBIV.md (<N> tasks)` or `wrote GBIV.md (no tasks assigned)`.

## What this skill does NOT do

- Does not modify `backlog/backlog.md`, item files, `.used-ids`, or any VAT-owned file.
- Does not run `vat sync` or any other VAT command.
- Does not preserve the freeform region or preamble — `GBIV.md` is a derived view, not a mirror.
- Does not parse or filter on `[blocked-by:...]` — blocked tasks the user owns still appear.

## Files this skill is allowed to touch

- `GBIV.md` (repo root) — read and write.
- `backlog/backlog.md` — read only.
- `~/.config/vat/config.toml` — read only.

Nothing else.
