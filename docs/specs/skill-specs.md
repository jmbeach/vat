# EARS Specs: The `vat` skill

Requirements for what is specific to the `vat` skill as an implementation: binary-first delegation, the fidelity contract, nested-repo detection, the atomicity guard, the atomic claim loop, and per-command terminal preconditions. See [skill LLD](../llds/skill.md). The command and format behavior the skill shares with the binary is specified in the `FMT-*`, `SYNC-*`, and `CMD-*` segments, not here.

Throughout: a **nested-repo backlog** is one where `backlog/.git` exists (the backlog is its own git repository); otherwise the backlog is tracked by the outer project repo. The binary is unaware of the distinction and never runs git; only the skill detects it and runs the atomic claim loop. A **mutating command** is any command that writes a file under `backlog/`: `sync`, `start`, `block`, `unblock`, `done`, `config set project.id`. Of these, the **claim-loop commands** — `sync`, `start`, `block`, `unblock`, `done` — run the full atomic claim loop; `config set project.id` is excluded from the loop (concurrent re-prefixing is out of scope) and instead commits and pushes once (see § `config set project.id`).

Status: `[x]` implemented, `[ ]` active gap, `[D]` deferred.

## Binary-first delegation

- [x] **SKILL-BIN-001** — Before performing a requested operation, the skill shall test whether the `vat` binary is available on `PATH` (e.g. via `command -v vat`); when it is available the skill shall delegate the operation to the binary, and when it is not available the skill shall execute the corresponding prose procedure.
- [x] **SKILL-BIN-002** — When the `vat` binary is available, the skill shall delegate by running exactly one binary invocation per requested operation (`vat sync`, `vat start <id>`, `vat block <id> <blocker-id>`, `vat unblock <id>`, `vat done <id>`, `vat init [<prefix>]`, `vat config get <key>`, `vat config set <key> <value>`) and shall report the binary's result (its stdout/stderr, surfacing a non-zero exit as the error).
- [x] **SKILL-BIN-003** — For any requested operation and backlog state, the file state produced by delegating to the binary shall be byte-identical to the file state the skill's prose procedure would produce (both implement the same `FMT-*`/`SYNC-*`/`CMD-*` specs).
- [x] **SKILL-BIN-004** — While the backlog is a nested repo and the `vat` binary is available, the skill shall still perform nested-repo detection, the atomicity guard, the atomic claim loop, and the terminal-precondition checks itself, substituting `vat <command>` for every local file-mutation step (the loop's mutate step and the guard's local-edit-only fallback); the binary shall never run `git`.
- [x] **SKILL-BIN-005** — When the `vat` binary is not available, the skill shall behave exactly as specified by the prose procedures and the other `SKILL-*` specs, with no change to file or git behavior.

## Fidelity and file boundary

- [ ] **SKILL-IMPL-001** — The skill shall implement the same EARS specs (`FMT-*`, `SYNC-*`, `CMD-*`) as the Rust binary; for any input it processes without running the atomic claim loop, the skill shall produce file state byte-identical to the binary's.
- [ ] **SKILL-IMPL-002** — The skill shall write only to the VAT-owned file set (`backlog/backlog.md`, `backlog/items/<id>.md`, `backlog/.used-ids`, `backlog/vat.toml`, `backlog/README.md` at init, and the user config file), and shall run `git` only from inside `backlog/`.

## Nested-repo detection

- [ ] **SKILL-DETECT-001** — When `backlog/.git` exists (as a directory, or as a file in the submodule/worktree case), the skill shall run claim-loop commands through the atomic claim loop (`SKILL-LOOP-*`) and `config set project.id` through its single-push path (`SKILL-CFG-*`).
- [ ] **SKILL-DETECT-002** — When `backlog/.git` does not exist, the skill shall edit files in place and perform no git operations, exactly as the binary does.
- [ ] **SKILL-DETECT-003** — The skill shall recognize a nested-repo backlog by filesystem layout only (`backlog/.git`); no configuration key or flag shall select or override this.
- [ ] **SKILL-DETECT-004** — `config get` (non-mutating) and `init` (runs before `backlog/` exists) shall never run the atomic claim loop, whether or not the backlog is a nested repo.

## Atomicity guard

- [ ] **SKILL-GUARD-001** — While the backlog is a nested repo, when a mutating command starts, the skill shall evaluate the atomicity guard before mutating any file: the `backlog/` working tree is clean (no modified, staged, or untracked files) AND, after a `git fetch`, `HEAD` is an ancestor of the remote-tracking branch (`@{u}`).
- [ ] **SKILL-GUARD-002** — While the backlog is a nested repo, when the atomicity guard is satisfied, the skill shall run a claim-loop command inside the atomic claim loop (reusing the guard's fetch as the first refresh's fetch), and shall run `config set project.id` through its single-push path (§ `config set project.id`).
- [ ] **SKILL-GUARD-003** — While the backlog is a nested repo, if the `backlog/` working tree is dirty or `HEAD` has commits the remote-tracking branch lacks, then the skill shall fall back to a local edit: apply the command's file changes exactly as the binary would, perform no `git commit`/`reset`/`push`, and report that the change was not pushed together with the reason (uncommitted changes, or unpushed local commits).
- [ ] **SKILL-GUARD-004** — While the backlog is a nested repo, if `@{u}` does not resolve (no configured upstream, or detached `HEAD`), then the skill shall abort with the raw git error (exit semantics of CMD-EXIT-002) and shall not mutate any file.
- [ ] **SKILL-GUARD-005** — While the backlog is a nested repo, if the guard's `git fetch` fails (unreachable remote, auth failure), then the skill shall abort with the raw git error (exit semantics of CMD-EXIT-002) and shall not mutate any file.

## Atomic claim loop

- [ ] **SKILL-LOOP-001** — While the backlog is a nested repo with the atomicity guard satisfied, every claim-loop command shall run the sequence refresh → re-read → terminal-precondition check → mutate → commit → push, with every git command run from inside `backlog/`.
- [ ] **SKILL-LOOP-002** — The loop's refresh shall be `git fetch` followed by `git reset --hard` to the remote-tracking branch.
- [ ] **SKILL-LOOP-003** — The loop's commit shall stage all `backlog/` changes and use the command's fixed commit message: `vat sync`, `vat start <id>`, `vat block <id> <blocker-id>`, `vat unblock <id>`, or `vat done <id>`. (`config set project.id` is not a claim-loop command; its commit message is specified by SKILL-CFG-001.)
- [ ] **SKILL-LOOP-004** — When the loop's push succeeds, the skill shall report success and stop (first push wins).
- [ ] **SKILL-LOOP-005** — If the loop's push is rejected as `rejected`/`non-fast-forward` (contention), then the skill shall not rebase or merge; it shall `git reset --hard` to the remote-tracking branch and re-run the decision from scratch on the refreshed state.
- [ ] **SKILL-LOOP-006** — Between contention retries, the skill shall wait an exponentially increasing backoff with random jitter, and shall cap retries at a fixed maximum attempt count.
- [ ] **SKILL-LOOP-007** — If the maximum attempt count is exhausted by contention, then the skill shall report a user-facing error (exit semantics of CMD-EXIT-002) stating the command lost the race after the configured attempts.
- [ ] **SKILL-LOOP-008** — If the loop's push fails for any non-contention reason (network, auth, quota), then the skill shall fail fast: surface the raw git error without consuming a retry (exit semantics of CMD-EXIT-002).
- [ ] **SKILL-LOOP-009** — When `sync` re-runs after a contention retry, the skill shall recompute id assignment against the refreshed `backlog.md` and `.used-ids`; ids assigned in a discarded attempt impose no constraint on the re-run.
- [ ] **SKILL-LOOP-010** — When a claim-loop command's mutate step leaves the `backlog/` tree byte-identical (e.g. `sync` with nothing to assign or extract), the skill shall report `unchanged` and shall make no commit and no push.

## Terminal preconditions

- [ ] **SKILL-TERM-001** — The loop shall evaluate the command's terminal precondition on the refreshed state before mutating; when it is satisfied, the loop shall stop without mutating or pushing.
- [ ] **SKILL-TERM-002** — When `start <id>` finds the task on refreshed state already claimed by any user, the skill shall report `lost claim: <id> already claimed by <name>` and stop as a user-facing error (consistent with CMD-START-002; the loop never produces a self-claimed refreshed state, because a rejected claim is reset away).
- [ ] **SKILL-TERM-003** — When `done <id>` finds no task with `<id>` on refreshed state and `<id>` is present in `backlog/.used-ids`, the skill shall stop reporting success (already done); the success guarantees removal only — no retroactive tombstoning or unblocking of out-of-band deletions.
- [ ] **SKILL-TERM-004** — When `done <id>` finds no task with `<id>` on refreshed state and `<id>` is absent from `backlog/.used-ids`, the skill shall abort with `unknown id: <id>` (per CMD-CC-002).
- [ ] **SKILL-TERM-005** — When `unblock <id>` finds the task on refreshed state with no `[blocked-by:...]` marker, the skill shall stop reporting success.
- [ ] **SKILL-TERM-006** — When `block <id> <blocker-id>` finds the task on refreshed state already carrying `[blocked-by:<blocker-id>]` for the same blocker, the skill shall stop reporting success.
- [ ] **SKILL-TERM-007** — `sync` shall have no terminal precondition; it shall always proceed to mutate on refreshed state.
- [ ] **SKILL-TERM-008** — Terminal preconditions shall not replace a command's local validation; a validation failure on refreshed state (e.g. `unknown id`, `unknown blocker`) shall abort the command without retrying.

## `config set project.id` (single push, no claim loop)

Concurrent re-prefixing is out of scope; `config set project.id` does not run the contention loop. Its local validation (`CMD-CFG-004` format, `CMD-CFG-005` no mixed prefixes) applies before any of the below.

- [ ] **SKILL-CFG-001** — While the backlog is a nested repo with the atomicity guard satisfied, `config set project.id <value>` shall not run the claim loop; the skill shall edit `vat.toml`, commit with `vat config set project.id <value>`, and push once, and shall not refresh, reset, or re-decide on any push outcome.
- [ ] **SKILL-CFG-002** — While the backlog is a nested repo, if `config set project.id`'s single push is rejected or fails for any reason, the skill shall fail fast with the raw git error (exit semantics of CMD-EXIT-002); it shall not retry.
- [ ] **SKILL-CFG-003** — When `config set project.id <value>` finds `project.id` already equal to `<value>`, the skill shall succeed without writing, committing, or pushing.
