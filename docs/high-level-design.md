# VAT — High-Level Design

**VAT** (Versioned Addressable Tasks) is a minimal CLI for maintaining a backlog as plain markdown files inside a project's repository. Its job is twofold: (1) assign short, memorable, stable IDs to tasks the user has jotted down, and (2) coordinate work assignment and status across collaborators using the markdown file itself as the source of truth.

## Problem statement

When you're in flow on a codebase, ideas for follow-up work arrive constantly — a small refactor, a missing endpoint, a UI tweak. The cost of capturing them in a "real" tool (Jira, Linear, GitHub Issues) is high enough that most people either don't capture them at all or accumulate scratch notes that go stale. Existing markdown-in-repo approaches (a `TODO.md`, a `BACKLOG.md`) solve the capture problem but produce a wall of unstructured text that gets harder to scan and hand off as it grows.

VAT is designed for **fast capture, then deferred structuring**. You jot a one-line bullet — or a bullet plus a few notes — directly into `backlog/backlog.md` without leaving your editor. Later, at your leisure, you run `vat sync` and every new bullet gets a short stable ID and any notes get tucked into their own file. From that point the task is *addressable*: you can hand it to a teammate or a coding agent (`work on foo-7k2`) and everything they need is at a known path.

Coordination across collaborators (claiming work, avoiding double-assignment) falls out of the same model — once tasks have stable IDs and live in version-controlled markdown, `vat start` and git merge are enough. But that's a byproduct of the capture-and-structure design, not the primary motivation.

## Goals

- **Frictionless capture.** Adding a task is just typing a `-` bullet in a markdown file. No CLI invocation required at the moment of capture.
- **Deferred structuring.** A single command (`vat sync`) turns raw bullets into addressable tasks with IDs and per-item files, on the user's schedule.
- **Clean handoff.** Once synced, any task is reachable by a short stable ID (`foo-7k2`) — easy to paste into a chat with a coding agent or a teammate.
- **Small working surface.** One flat list in `backlog/backlog.md`; one optional notes file per task in `backlog/items/<id>.md`.
- **Lightweight.** No daemons, no servers, no required network calls. Markdown is the source of truth.
- **Safe to run repeatedly.** Every command is idempotent on a stable input.

## Non-goals

- Issue tracking with comments, attachments, history, or rich state machines.
- Cross-repo task aggregation, dashboards, or reporting.
- Real-time collaboration. Race resolution is delegated to git merge.
- Sub-tasks, dependencies beyond a single `blocked-by` link, priorities, due dates, or labels.
- A web UI or any kind of server.

## Target users

- Small teams or solo developers who want a backlog colocated with the code.
- AI agents collaborating on a codebase that need a low-ceremony way to claim and complete tasks.

## System architecture

VAT is a single Rust binary. There is no daemon, no index, and no database. State is the contents of these files:

```
backlog/
  backlog.md           # the flat list of tasks (source of truth)
  vat.toml             # project config (e.g., project ID prefix)
  .used-ids            # tombstone list of IDs ever assigned (one per line)
  items/
    foo-7k2.md         # optional; only when a task has notes
    foo-9hf.md
~/.config/vat/config.toml   # global config (user name)
```

Every command is a one-shot read → mutate → write cycle on these files. Concurrency between collaborators is resolved by git: claims and edits land as line-level diffs that merge cleanly when disjoint and produce conflicts when not.

### Implementations

VAT has two first-class implementations of the same intent:

- **The Rust binary** (`src/`) — the primary distribution.
- **The `vat` skill** (`.claude/skills/vat/SKILL.md`) — a prose implementation an agent follows directly. A remote coding agent is handed the whole skill and operates a VAT backlog with zero install (no `cargo`, no binary), which the binary cannot offer. Its design lives in [the skill LLD](llds/skill.md).

Both implementations target the same EARS specs. Where a spec is implemented by only one of them, its status reflects that.

### Backlog as a nested repo

A `backlog/` can itself be a git repository — a submodule, or a standalone clone an agent pulls — rather than being tracked by the outer project repo. This is a property of the backlog's layout, not a mode VAT switches into:

- **The binary is unaware of it.** It edits the markdown files in place regardless; committing and pushing the backlog repo is the collaborator's concern, exactly as for any backlog (Decision #1). A human running the binary against a nested-repo backlog is their own retry loop — they push, and on a rejected push they pull and re-run.
- **The skill detects it** (`backlog/.git` present) and, because an autonomous agent has no human to drive git, runs an **atomic claim loop** against the backlog's own remote (see Decision #7). This is what makes parallel agents practical — each agent claims tasks against a shared backlog remote without a coordinating server.

The two can share one backlog: a human on the binary and an agent on the skill coordinate at the git remote, not in the tool.

### File formats

**`backlog/backlog.md`** — markdown. Everything above the first `---` horizontal rule is parsed; everything below is freeform. Parsed content is a flat list of top-level `-` bullets. Nested bullets and indented prose under a bullet are "notes" — they're moved into the item file on `vat sync`.

**Bullet line canonical form:**

```
- [foo-7k2] [in-progress] [by:jared] [blocked-by:foo-9hf] Title text here
```

Markers always front-loaded, in canonical order: `[id]`, `[in-progress]`, `[by:<name>]`, `[blocked-by:<id>]`, then the title. Only `[id]` is required after `vat sync`. `vat sync` normalizes order and spacing.

**`backlog/items/<id>.md`** — markdown with frontmatter:

```
---
id: foo-7k2
---

Notes content moved here from backlog.md.
```

**`backlog/.used-ids`** — newline-delimited list of every ID ever issued (active or deleted). Committed. Read by `vat sync` to avoid reuse.

**`backlog/vat.toml`** — project config:

```toml
[project]
id = "foo"  # 3-char prefix prepended to all IDs in this repo
```

**`~/.config/vat/config.toml`** — global per-user config:

```toml
[user]
name = "jared"
```

## ID scheme

`<project>-<suffix>` — e.g., `foo-7k2`.

- **Project prefix**: 3 characters, set once in `backlog/vat.toml`. Globally disambiguates IDs across repos so `foo-7k2` in a commit message is unambiguous.
- **Suffix**: 3 characters of Crockford base32 (no ambiguous chars: no I/L/O/U). ~32k IDs per project.
- **Generation**: random; on `vat sync`, retry against the union of (currently-present IDs, `.used-ids` tombstones) until a free one is found. Tombstones ensure deleted IDs are never reused, so external references (`fixes foo-7k2` in a closed PR) remain unambiguous forever.

## Commands

| Command | Effect |
|---|---|
| `vat init` | Create `backlog/`, prompt for project prefix, write `vat.toml` and an empty `backlog.md`. |
| `vat sync` | Scan `backlog.md`. Assign IDs to bullets that lack them. Move notes-under-bullet into `items/<id>.md` (creating or appending). Normalize marker order. Does not touch dangling `[blocked-by:X]` references. |
| `vat start <id>` | Add `[in-progress] [by:<user>]` to the matching bullet. Refuses if either marker is already present. |
| `vat block <id> <blocker-id>` | Add `[blocked-by:<blocker-id>]` to the bullet. |
| `vat unblock <id>` | Remove the `[blocked-by:...]` marker. |
| `vat done <id>` | Delete the bullet from `backlog.md`. Delete `items/<id>.md` if it exists. Append `<id>` to `.used-ids` if not already there. Auto-clear any `[blocked-by:<id>]` markers on other bullets. |
| `vat config set <key> <value>` | Set a config value. `user.name` writes to global config; `project.id` writes to project config. |
| `vat config get <key>` | Read a config value. |

`vat sync` is the only command that may mutate the *structure* of `backlog.md` (reorder lines, move content to item files, normalize markers). `start`/`block`/`unblock`/`done` only touch the single matching bullet line (and `done` also removes it).

## Key design decisions

### 1. Markdown is the source of truth, git is the concurrency primitive

Conflict detection for "who claimed this task" is delegated entirely to git merge. `vat start` does a local check (refuses if `[in-progress]` or `[by:...]` is already present) and writes the line. If two collaborators on different branches both run `vat start foo-7k2`, the merge produces a normal conflict on that line, which the human resolves.

**Consequence:** Zero network code paths. No "service" to maintain. Offline-first.
**Consequence:** Cross-branch races aren't caught until merge — collaborators may waste work on a contested task. Acceptable for a lightweight tool; teams that want stronger guarantees can use a PR-based claim flow.

### 2. Explicit `vat sync`, not implicit-on-every-command

Only `vat sync` mutates structurally. This makes diffs predictable and gives the user control over when their backlog file is rewritten.

**Consequence:** A user who jots down 5 new bullets must remember to run `vat sync` to ID them. Acceptable; the alternative (every `vat *` command rewrites the file) makes diffs unpredictable.

### 3. Tombstones, not git history, for ID reuse prevention

`.used-ids` is a committed file appended on every ID assignment. `vat sync` reads it as part of collision avoidance. We deliberately don't grep git history (slow, couples sync to git internals).

**Consequence:** Tombstone file is a second source of truth and can drift if hand-edited. Mitigated by being append-only and the only writers being `vat sync` and `vat done`.

### 4. Item files are optional, deleted on done

`backlog/items/<id>.md` exists only if the task had notes. On `vat done`, the file is deleted. The git history is the permanent record.

**Consequence:** Reduces file count; no empty stub files.
**Consequence:** Recovering a deleted task requires `git log` / `git show`, not a `closed/` directory listing. Matches the "lightweight" goal.

### 5. Auto-unblock only on `vat done`, not on `vat sync`

`vat done <id>` walks the backlog and strips `[blocked-by:<id>]` from any other bullets. `vat sync` does **not** do this self-heal — if a `[blocked-by:X]` reference is dangling because of a hand-edit or odd merge, `sync` leaves it alone.

**Consequence:** A done task automatically unblocks dependents (matches user expectation). But sync is purely about IDs and notes, not about silently mutating semantic relationships, so a dangling reference is preserved for the user to inspect rather than disappearing on the next sync.

### 6. Front-loaded markers in canonical order

Markers all live at the front of the bullet, in the order `[id] [in-progress] [by:X] [blocked-by:Y]`. A user scanning the backlog sees status before reading the title; a blocked task is visually obvious.

**Consequence:** `vat sync` must normalize order if a user hand-edits markers out of position. Easy.

### 7. First-push-wins claim loop for nested-repo backlogs

When the backlog is its own git repo, the `vat` skill claims and completes tasks against a shared backlog remote without a server and without textual merge conflicts. (The binary is unaware of the nested repo and never does this — a human driving the binary is their own retry loop.) Every atomic, mutating task command the skill runs (`sync`, `start`, `block`, `unblock`, `done`) goes through an **atomic claim loop** — `refresh → re-read → check terminal precondition → mutate → commit → push`, with all git run from inside `backlog/`:

- **First push wins.** On a rejected push — and *only* on `rejected` / `non-fast-forward`, i.e. genuine contention — the loser does **not** rebase or merge. It `reset --hard`s to the winner's state and **re-runs the decision from scratch** on the fresh state. There is never a textual merge conflict, not even a false one between two tasks edited on nearby lines.
- **Atomicity guard.** The loop's `reset --hard` is destructive, so it only runs when `backlog/` is **clean *and* synced at entry** — the skill's own mutation is the sole change it will ever discard on a reset. If `backlog/` has *any* uncommitted or untracked changes when the command starts (the user is mid-grooming, staging several new tasks, etc.), **or** carries a local commit the remote lacks (the branch is ahead of origin — which `reset --hard` would also destroy), the skill falls back to a **local edit**: it makes the change on disk and does no commit/reset/push, so unrelated in-flight work is never blown away. The user syncs that batch themselves (e.g. the `backlog-sync` skill).
- **Fail fast on non-contention errors.** Network/auth/quota push failures surface the raw git error immediately and do not consume retries — a lost-race report must mean a real lost race.
- **Terminal precondition ends retries early.** Per command: `start` on an already-claimed task on fresh state → `lost claim: <id> already claimed by <name>`, stop. `done` on an already-absent task → success. `sync` has no terminal precondition. The terminal condition *is* the lock telling you the outcome.
- **Backoff + jitter** between retries, capped at a MAX attempt count.
- **Fixed per-command commit messages** (`vat sync`, `start <id>`, `done <id>`, …) so the backlog remote's history is auditable.
- **`config set project.id` is outside the loop.** Re-prefixing is a rare administrative operation guarded locally (`CMD-CFG-005`); the skill commits and pushes it once and fails fast on a rejected push rather than re-deciding the re-prefix against a winner's fresh state.

**Consequence:** Safe, server-less parallel task claiming for agent fleets.
**Consequence:** Only the *atomic* case (clean backlog tree, single skill mutation) is auto-pushed; batched human grooming stays local until explicitly synced. The guard keeps the destructive `reset --hard` path from ever touching unrelated work.
**Consequence:** The loop requires the backlog remote to be reachable; when `refresh` or `push` fails for a non-contention reason (network/auth/quota), the command fails fast with the raw git error rather than silently editing locally, so an agent never believes a claim landed when it did not.

## Trade-offs explicitly considered

| Decision | Picked | Rejected | Why |
|---|---|---|---|
| Concurrency (any backlog) | Local check + git merge | Git-aware claim, lockfile dir | Lightweight; no network; markdown stays the truth |
| Skill claiming on a nested-repo backlog | Atomic first-push-wins loop (reset + re-decide) | Textual merge / rebase the loser's change | No false conflicts between disjoint tasks; server-less parallel claiming; guard keeps reset from touching unrelated work |
| Ingest model | Explicit `vat sync` | Implicit on every command | Predictable diffs |
| ID scheme | `<3-char-project>-<3-char-base32>` | Word pairs, sequential | Typeable, scoped across repos, merges cleanly |
| ID reuse | Tombstone file | Reuse freely; git history scan | Cheap, robust referenceability |
| Item file on done | Delete | Keep as record | Git is the record |
| Blocker on done | Auto-unblock | Leave dangling | Matches user expectation, cheap |

## Open / deferred

- `vat add "<title>"` — inline create-and-assign. Deferred; user said may add later.
- Additional statuses (`review`, `done` as a state, etc.) — deferred; current model deletes on done.
- Sub-tasks — explicitly out; nested bullets are notes.
- Cross-repo references — out of scope, but the project-prefix scheme leaves the door open.
