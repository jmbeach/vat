# LLD: The `vat` skill

The `vat` skill (`.claude/skills/vat/SKILL.md`) is a prose implementation of VAT that an agent follows directly, with no binary to install. It is a first-class implementation alongside the Rust binary (see [HLD § Implementations](../high-level-design.md)). Both target the same EARS specs; the file grammar lives in the [backlog-format LLD](./backlog-format.md), per-command behavior in the [commands LLD](./commands.md) and [sync LLD](./sync.md).

This LLD covers what is specific to the skill: its reason for existing, the fidelity contract that keeps it equivalent to the binary, and the one behavior the skill owns and the binary deliberately does not — detecting a **nested-repo backlog** and running its mutating commands through an atomic claim loop.

## Context and Design Philosophy

The binary is the primary distribution, but a coding agent often cannot run it — a remote agent in a fresh container has no `cargo`, no release binary, no time to build one. The skill closes that gap: hand the agent the whole `SKILL.md` and it operates a VAT backlog using only file reads/writes and `git`. This makes the skill the natural implementation for **agent fleets** — many agents sharing one backlog, each claiming tasks — which is exactly where a nested-repo backlog matters.

Two principles govern the skill:

1. **Fidelity to the specs, not to the binary's code.** The skill implements the same EARS specs (`FMT-*`, `SYNC-*`, `CMD-*`) the binary does. For any input it processes without running the atomic claim loop, it produces file state byte-identical to the binary's. The skill is prose, so its "tests" are the binary's Rust test suite citing the shared spec IDs; the skill itself carries no executable tests.
2. **Never write outside the VAT file set.** The skill's `allowed-tools` are `Read, Write, Edit, Bash`. `Bash` exists for `git`, which the skill runs only when operating a nested-repo backlog. The skill mutates exactly the VAT-owned files and runs `git` only inside `backlog/`; it touches nothing else in the working tree.

## Implementation surface

The skill implements every command (`init`, `sync`, `start`, `block`, `unblock`, `done`, `config`) against the same file formats as the binary. That behavior is specified elsewhere and not duplicated here. The skill's own segment (`SKILL-*`) specifies only:

- **Nested-repo detection** — recognizing a self-versioned backlog (`SKILL-DETECT-*`).
- **The atomic claim loop** — how mutating commands run against a nested-repo backlog (`SKILL-LOOP-*`).
- **The atomicity guard** — when the loop is safe to run vs when the command falls back to a local edit (`SKILL-GUARD-*`).
- **Terminal preconditions** — the per-command condition that ends the retry loop (`SKILL-TERM-*`).
- **The fidelity / file-boundary contract** above (`SKILL-IMPL-*`).

## Detecting a nested-repo backlog

A `backlog/` is sometimes its own git repository — a submodule, or a standalone clone an agent pulls — rather than being tracked by the outer project repo. The binary is unaware of the distinction: it edits the markdown either way and leaves git to the collaborator. The skill is the only implementation that looks, and it looks purely at filesystem layout, with no config flag:

- **No `backlog/.git`** — the backlog is tracked by the outer project repo. The skill edits files in place and does no git; concurrency is the outer repo's git merge, byte-for-byte as the binary behaves.
- **`backlog/.git` exists** — the backlog is its own repository. The skill runs every mutating command through the atomic claim loop below, because an autonomous agent has no human to commit, push, and resolve a rejected push by hand.

Detection is a single test for `backlog/.git` (a directory, or a file in the submodule/worktree case). Non-mutating commands (`config get`, and `init`, which runs before any `backlog/` exists) never enter the loop.

## The atomic claim loop (nested-repo backlogs)

A **mutating command** is any command that writes a file under `backlog/`: `sync`, `start`, `block`, `unblock`, `done`, and `config set project.id`. The **claim-loop commands** — `sync`, `start`, `block`, `unblock`, `done` — run the full retry loop below when `backlog/.git` exists; each runs `read → decide → mutate → write` inside it. `config set project.id` is excluded from the loop (see [§ `config set project.id`](#config-set-projectid-is-not-a-claim-loop-command)). All `git` runs from inside `backlog/`.

```
attempt = 0
loop:
  1. refresh:  git fetch, then git reset --hard <remote-tracking-branch>
               (safe: the atomicity guard has ensured the tree is clean AND synced — see below)
  2. re-read:  parse the now-fresh backlog files
  3. terminal precondition check (per command):
        - satisfied as success  -> report success, stop (no further mutation)
        - satisfied as loss      -> report the loss, stop
        - not satisfied          -> continue
  4. mutate:   apply the command's decision to the files (the same edit the binary makes)
  5. commit:   git add -A && git commit -m "<fixed per-command message>"
  6. push:     git push
        - success                         -> report success, stop   (first push wins)
        - rejected / non-fast-forward     -> contention: discard local commit and retry
                                             (git reset --hard <remote-tracking-branch>),
                                             attempt += 1; if attempt >= MAX, report
                                             max-attempts exhausted and stop; else backoff
                                             (sleep base * 2^attempt + jitter) and loop
        - any other push failure          -> FAIL FAST: surface the raw git error, stop
```

**First push wins.** On a rejected push — and **only** on `rejected` / `non-fast-forward`, i.e. genuine contention — the loser does not rebase or merge. It `reset --hard`s to the winner's state and re-runs the whole decision (steps 1–6) on fresh state. Because the decision is recomputed from scratch each round, there is never a textual merge conflict, not even a false one between two unrelated tasks edited on nearby lines.

**Fail fast on non-contention failures.** A push that fails for network, auth, or quota reasons is not a lost race. The command surfaces the raw git error immediately and does not consume retries — so a "lost claim" report always means a real lost race. A failing `refresh` (e.g. the remote is unreachable) likewise fails fast; the skill does not silently degrade to a local edit, which would let an agent believe a claim landed when it did not.

**Backoff.** Between contention retries the command sleeps `base * 2^attempt` plus random jitter, capped at `MAX` attempts.

**No-op short-circuit.** If a claim-loop command's mutate step leaves the `backlog/` tree byte-identical — most commonly `sync` with nothing to id or extract (the `SYNC-WRITE` byte-identical skip) — the command reports `unchanged` and makes no commit and no push, so the loop never adds an empty commit to the backlog remote.

### Terminal preconditions

The terminal precondition is checked on fresh state (step 3), before mutating. It is the lock telling the command its outcome:

| Command | Terminal precondition (on fresh state) | Result |
|---|---|---|
| `start <id>` | the task is already claimed by any user (`[by:<name>]`, or `[in-progress]` from a hand-edit) | loss — report `lost claim: <id> already claimed by <name>`, stop |
| `done <id>` | the task is absent **and** `<id>` is in `.used-ids` | success — already done, stop |
| `done <id>` | the task is absent and `<id>` is **not** in `.used-ids` | error — `unknown id: <id>` (a typo, not a finished task), stop |
| `unblock <id>` | the task has no `[blocked-by:...]` | success — no-op, stop |
| `block <id> <b>` | the task already has `[blocked-by:<b>]` for the same blocker | success — no-op, stop |
| `sync` | (none — sync has no terminal precondition) | always proceeds to mutate |

`start` reports a loss for *any* existing claim, exactly as `CMD-START-002` errors for any existing claim — there is no "already mine, success" case, because the loop never produces a self-claimed refreshed state: a rejected claim is `reset --hard`ed away before re-read, so a claim seen on fresh state is always someone else's (or your own from a *prior* completed `vat start`, which `CMD-START-002` already rejects). `config set project.id` does not appear here — it is not a claim-loop command.

A terminal precondition only ends the loop early; it never replaces a command's normal local validation (e.g. `start` still errors on `unknown id`, `block` still errors on `unknown blocker`). Those local errors stop the command without entering or continuing the loop.

Two boundaries on the terminal preconditions:

- **`done` on an already-absent task only guarantees the *removal*, not the bookkeeping.** When VAT itself removes a task it also tombstones the id in `.used-ids` and clears dependents' `[blocked-by:<id>]`. If the bullet was removed out-of-band (a hand-edit, an odd merge) rather than by `vat done`, that bookkeeping was never performed, and `done`'s success-stop does not retro-apply it. This matches VAT's standing posture (sync does not self-heal dangling references either) — out-of-band edits are the user's to inspect.
- **`sync` is non-deterministic across retries, by design.** On contention `sync` re-decides on fresh state and may hand out *different* random ids than a discarded attempt. This is safe: nothing external has referenced the discarded ids (they were never pushed), and collision-avoidance re-reads the winner's `.used-ids` each round.

### Fixed commit messages

Each command commits with a fixed message so the backlog remote's history is auditable:

| Command | Commit message |
|---|---|
| `sync` | `vat sync` |
| `start <id>` | `vat start <id>` |
| `block <id> <b>` | `vat block <id> <b>` |
| `unblock <id>` | `vat unblock <id>` |
| `done <id>` | `vat done <id>` |
| `config set project.id <v>` | `vat config set project.id <v>` |

## The atomicity guard

The loop's `refresh` and contention-retry both `reset --hard <remote-tracking-branch>`, which is destructive. The guard ensures that reset can only ever discard the skill's *own* in-loop commit, never inherited work.

`reset --hard @{u}` is safe exactly when the inherited backlog state contains nothing the remote lacks: the working tree is **clean** *and* every local commit is **already on the remote** (`HEAD` is an ancestor of `@{u}`). A clean tree alone is not enough — a clean tree can still sit on top of an unpushed local commit, which `reset --hard` would silently destroy. So the guard, evaluated once at entry, is **clean AND synced**:

```
git -C backlog status --porcelain          # any output            → dirty   → fallback
git -C backlog fetch                        # fails (unreachable)   → fail fast (no upstream → see below)
git -C backlog merge-base --is-ancestor HEAD @{u}
        # exit 0 → HEAD ⊆ upstream → clean & synced → run the atomic claim loop
        # exit 1 → local commits the remote lacks → fallback
```

- **Clean & synced → run the loop.** The entry `fetch` doubles as the first `refresh` fetch (no extra round-trip), and the ancestor check is computed against the freshly-updated upstream so a stale tracking ref cannot cause a false fallback.
- **Dirty *or* ahead → local-edit-only fallback.** If `backlog/` has any uncommitted/untracked change, **or** carries a local commit the remote lacks — the user is mid-grooming, staging several tasks, or committed backlog work without pushing — the command does **not** enter the loop. It makes its change on disk exactly as the binary would and performs **no** `commit`/`reset`/`push`. Inherited in-flight work, committed or not, is never touched; the user syncs the whole batch themselves (e.g. via the `backlog-sync` skill). The fallback is reported with its reason (`not pushed; backlog/ has other uncommitted changes` or `not pushed; backlog/ has unpushed local commits`) so the deferral is never silent.

**Why "ahead" commits at entry are always foreign work.** When driving a nested-repo backlog the skill only ever commits *inside* the loop and immediately pushes it (or `reset --hard`s it away on contention). The skill never leaves a committed-but-unpushed state of its own. So any unpushed commit present *at entry* came from outside the skill and must be preserved. The ancestor check therefore guards the *inherited* state only — inside the loop the skill's own commit makes `HEAD` ahead by one, and that commit is the skill's to discard, so the in-loop `reset --hard` stays correct.

**No upstream / detached HEAD.** If `@{u}` does not resolve (the backlog repo has no configured upstream, or HEAD is detached), the skill cannot establish the synced invariant and **fails fast** with the raw git error rather than guessing — a `backlog/.git` whose owner has not wired a remote is a misconfiguration to surface, not to silently treat as an ordinary in-project backlog.

## `config set project.id` is not a claim-loop command

`config set project.id` re-prefixes a project — a rare, administrative operation guarded locally by `CMD-CFG-005` (it refuses if any id in `backlog.md` or `.used-ids` still uses a different prefix). Agents racing to re-prefix a project is not a real scenario, so the contention machinery is unwarranted, and worse than unwarranted: re-running the re-prefix decision against a winner's fresh state is exactly the kind of destructive re-decide the loop's reset semantics are *not* meant for. So this command is excluded from the loop.

On a nested-repo backlog, with the atomicity guard satisfied, it runs a **single push**: edit `vat.toml` → commit (`vat config set project.id <v>`) → `git push`, once. There is no refresh, no reset, no re-decide. A rejected or otherwise failed push **fails fast** with the raw git error (exit 1) and the user re-runs after pulling. If `project.id` already equals the requested value, it succeeds without writing, committing, or pushing. The atomicity guard still applies: on a dirty-or-ahead `backlog/` it falls back to the local-edit-only path like any other mutating command.

## Decisions & Alternatives

| Decision | Chosen | Alternatives considered | Rationale |
|---|---|---|---|
| Who runs the claim loop | The skill only; the binary stays git-agnostic | The binary also detects `backlog/.git` and runs the loop | The binary is a pure markdown mutator (HLD Decision #1) — a human driving it is their own retry loop; only the autonomous skill needs to internalize git, so a nested-repo backlog is the skill's awareness, not a VAT-wide mode |
| Loop command set | `sync`, `start`, `block`, `unblock`, `done`; `config set project.id` excluded (single push) | All mutating commands loop uniformly | Concurrent re-prefixing is not a real scenario; re-deciding a re-prefix against a winner's fresh state is destructive, so the loop's reset semantics are wrong for it |
| Contention resolution | `reset --hard` to winner + re-decide from scratch | rebase/merge the loser's commit | Recomputing the decision on fresh state avoids textual merge conflicts entirely, including false conflicts between disjoint tasks on nearby lines |
| Atomicity guard | clean **and synced** at entry (`HEAD` ⊆ `@{u}`); else local-edit-only | clean tree only; "the edit is the sole dirty file"; stash unrelated work around the loop | `reset --hard` also destroys unpushed commits, so a clean tree alone is unsafe; awareness is decided before the edit, so the inherited state is the only honest signal; stashing adds failure modes (stash conflicts on reset) |
| Non-contention failures | fail fast with raw git error | retry; or degrade to a silent local edit | A lost-claim report must mean a real lost race; a silent local edit would let an agent think it claimed a task it did not |
| Nested-repo detection | filesystem layout (`backlog/.git`) | config flag in `vat.toml`; CLI `--remote` | Zero ceremony; the layout *is* the intent — a separately-versioned backlog wants server-less coordination |
| Skill status | first-class, permanent implementation | stopgap retired when the binary ships | Zero-install operation for remote agents is a capability the binary cannot match; the skill is the fleet implementation |

## Open Questions & Future Decisions

### Resolved
1. ✅ Atomicity guard is **clean AND synced at entry** (`HEAD` is an ancestor of `@{u}`), with a local-edit-only fallback when `backlog/` is dirty *or* carries unpushed local commits.
2. ✅ Non-contention git failures — including an unreachable remote and a `backlog/.git` with no upstream / detached HEAD — **fail fast** with the raw git error; no silent local degrade.
3. ✅ A nested-repo backlog is detected by `backlog/.git` presence, not configuration.
4. ✅ `start` on a task already claimed by **me** succeeds as a no-op (already yours); only a claim by **another** user is a loss.
5. ✅ `done` on an already-absent task succeeds, but only guarantees removal — out-of-band deletions do not get retro-tombstoned or auto-unblocked. This is scoped to the loop (`.used-ids` consulted on refreshed state); the shared `CMD-*`/binary `done` is unchanged.
6. ✅ Exit/report semantics for the loop: contention exhausted at `MAX` → user-facing error (exit 1, `CMD-EXIT-002`); non-contention git failure (network/auth/quota/unreachable remote) → also user-facing error (exit 1, `CMD-EXIT-002`), carrying the raw git message.
7. ✅ The claim loop is the skill's alone — the binary stays unaware of a nested-repo backlog and never runs git. This is a permanent division of labor, not a gap pending binary parity.
8. ✅ `start` has no "already mine, success" case — any existing claim on fresh state is a loss, consistent with `CMD-START-002`; the loop never produces a self-claimed refreshed state.
9. ✅ `config set project.id` is excluded from the claim loop — single push, fail fast on rejection (concurrent re-prefixing is out of scope).
10. ✅ A claim-loop command whose mutate leaves the tree byte-identical (e.g. no-op `sync`) reports `unchanged` and makes no commit/push.

### Deferred
1. Backoff constants in a future binary implementation. The skill pins `MAX = 5` attempts and `base = 0.5s` exponential backoff with `[0, 0.5]s` jitter; the design only fixes the *shape* (capped exponential backoff with jitter), so a binary could choose different values.

## References

- [HLD § Implementations and § Backlog as a nested repo](../high-level-design.md) — the two implementations and how a nested-repo backlog is handled.
- [HLD Decision #7](../high-level-design.md) — the first-push-wins claim loop rationale.
- [commands LLD](./commands.md), [sync LLD](./sync.md), [backlog-format LLD](./backlog-format.md) — the command and format behavior the skill shares with the binary.
- [backlog-sync skill] — the manual chore-PR flow used to sync a dirty backlog in the local-edit-only fallback.
