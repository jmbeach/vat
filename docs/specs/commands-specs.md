# EARS Specs: Commands other than sync

Requirements for `vat init`, `vat start`, `vat block`, `vat unblock`, `vat done`, and `vat config`. See [commands LLD](../llds/commands.md).

Status: `[x]` implemented, `[ ]` active gap, `[D]` deferred.

## Cross-cutting

- [x] **CMD-CC-001** — Every command that reads `backlog.md` shall first verify that the file's frontmatter `version` does not exceed the CLI's supported major version, and shall abort with an error before any other processing if it does.
- [x] **CMD-CC-002** — When a bullet-mutating command cannot find a bullet matching the supplied `<id>`, the system shall abort with an error and shall not write to any file.
- [x] **CMD-CC-003** — When a bullet-mutating command writes a bullet, the system shall emit markers in the canonical order defined by FMT-MARK-004.
- [x] **CMD-CC-004** — When a bullet-mutating command finds a bullet line whose leading `[id]` marker matches the supplied `<id>` but which fails to parse, the system shall abort with an error describing the parse failure (not a generic "unknown id") and shall not write to any file.

## `vat init`

- [x] **CMD-INIT-001** — When `backlog/` already exists, `vat init` shall abort with an error.
- [x] **CMD-INIT-002** — When invoked with `vat init <prefix>`, the system shall use `<prefix>` as the project ID prefix.
- [x] **CMD-INIT-003** — When invoked with no argument, `vat init` shall prompt the user interactively for the project ID prefix.
- [x] **CMD-INIT-004** — `vat init` shall reject any prefix that is not exactly 3 ASCII alphanumeric characters (FMT-PFX-001).
- [x] **CMD-INIT-005** — On success, `vat init` shall create `backlog/`, `backlog/vat.toml` containing `[project] id = "<prefix>"`, `backlog/backlog.md` containing only a `version: 1` frontmatter block, an empty `backlog/.used-ids`, and `backlog/README.md`.
- [x] **CMD-INIT-006** — `backlog/README.md` shall describe what VAT is, how to obtain it, the purpose of each file in `backlog/`, and the basic workflow. (Template baked into the binary as `readme_template::BACKLOG_README_TEMPLATE`; `vat init` renders and writes it via `readme_template::render`.)
- [x] **CMD-INIT-007** — After init, no VAT command shall read, validate, or rewrite `backlog/README.md`.

## `vat start <id>`

- [x] **CMD-START-001** — When `user.name` is unset in the user config, `vat start` shall abort with an error pointing the user at `vat config set user.name <name>`.
- [x] **CMD-START-002** — When the target bullet has either an `[in-progress]` marker or a `[by:...]` marker, `vat start` shall abort with an error indicating the existing claim.
- [x] **CMD-START-003** — On success, `vat start` shall add both `[in-progress]` and `[by:<user.name>]` markers to the target bullet.
- [x] **CMD-START-004** — On success, `vat start` shall print a confirmation message naming the claimed `<id>`, consistent with `vat init`'s success output.

## `vat block <id> <blocker-id>`

- [x] **CMD-BLOCK-001** — When `<id>` equals `<blocker-id>`, `vat block` shall abort with an error.
- [x] **CMD-BLOCK-002** — When no well-formed bullet matches `<blocker-id>`, `vat block` shall abort with an error (`unknown blocker: <blocker-id>`).
- [x] **CMD-BLOCK-002a** — When a bullet's leading `[id]` marker matches `<blocker-id>` but the bullet fails to parse, `vat block` shall abort with an error describing the parse failure (not a generic `unknown blocker`) and shall not write to any file.
- [x] **CMD-BLOCK-003** — When the target bullet already has `[blocked-by:<blocker-id>]` matching the supplied blocker, `vat block` shall succeed without modifying any file.
- [x] **CMD-BLOCK-004** — When the target bullet has `[blocked-by:<other>]` for a different blocker, `vat block` shall replace it with `[blocked-by:<blocker-id>]`.
- [x] **CMD-BLOCK-005** — When the target bullet has no `[blocked-by:...]` marker, `vat block` shall add `[blocked-by:<blocker-id>]` in canonical position.
- [x] **CMD-BLOCK-006** — `vat block` in v1 shall not detect blocker cycles.

## `vat unblock <id>`

- [x] **CMD-UNBLOCK-001** — When the target bullet has no `[blocked-by:...]` marker, `vat unblock` shall succeed without modifying any file.
- [x] **CMD-UNBLOCK-002** — When the target bullet has a `[blocked-by:...]` marker, `vat unblock` shall remove that marker.

## `vat done <id>`

- [x] **CMD-DONE-001** — `vat done` shall remove the entire bullet line for `<id>` from `backlog.md`.
- [x] **CMD-DONE-002** — When `backlog/items/<id>.md` exists, `vat done` shall delete it.
- [x] **CMD-DONE-003** — `vat done` shall append `<id>` to `backlog/.used-ids` if it is not already present.
- [x] **CMD-DONE-004** — `vat done` shall remove every `[blocked-by:<id>]` marker from any other bullet in the parsed region.
- [x] **CMD-DONE-005** — `vat done` shall succeed even when the target bullet has its own `[blocked-by:...]` marker.

## `vat config`

- [x] **CMD-CFG-001** — `vat config get user.name` shall print the value from the user config to stdout and exit 0, or exit 0 with no output if unset; exit 1 only on hard errors (I/O failure, unknown key).
- [x] **CMD-CFG-002** — `vat config get project.id` shall print the value from `backlog/vat.toml` to stdout and exit 0, or exit 0 with no output if unset; exit 1 only on hard errors (I/O failure, unknown key).
- [x] **CMD-CFG-003** — `vat config set user.name <value>` shall write the value to the user config, creating the file and parent directories if needed.
- [x] **CMD-CFG-004** — `vat config set project.id <value>` shall validate `<value>` as 3 ASCII alphanumeric characters (FMT-PFX-001).
- [x] **CMD-CFG-005** — `vat config set project.id <value>` shall abort with an error if any IDs in `backlog.md` or `backlog/.used-ids` use a different prefix.
- [x] **CMD-CFG-006** — `vat config set` shall reject keys other than `user.name` and `project.id` with an error.

## `vat completions <shell>`

- [x] **CMD-COMP-001** — `vat completions <shell>` shall write a shell completion script for `<shell>` to stdout and exit 0.
- [x] **CMD-COMP-002** — The supported shells shall be exactly `bash`, `zsh`, and `fish`; passing any of these values shall produce non-empty output, and any other shell (including `elvish` and `powershell`, which `clap_complete` itself supports) shall be rejected.
- [x] **CMD-COMP-003** — The `completions` subcommand shall not appear in the output of `vat --help`, nor as a completable subcommand in any generated completion script.
- [x] **CMD-COMP-004** — When `<shell>` is not a recognised shell name, the system shall exit with code 2 and print a usage error to stderr.
- [x] **CMD-COMP-005** — When writing the completion script to stdout fails, the system shall print an error to stderr and exit with code 2, except for a broken pipe, which shall terminate the command silently with exit code 0.

## Exit codes

- [x] **CMD-EXIT-001** — On success, every command shall exit with code 0.
- [x] **CMD-EXIT-002** — On user-facing errors (unknown ID, missing config, validation failure, version mismatch), every command shall exit with code 1.
- [x] **CMD-EXIT-003** — On internal errors (file IO failure, unexpected parse failure), every command shall exit with code 2.

## Out of scope for v1

- [D] **CMD-LOCK-001** — Concurrent-process locking on the backlog directory.
- [D] **CMD-FORCE-001** — `--force` overrides on guarded commands.
- [D] **CMD-DRYRUN-001** — `--dry-run` mode on mutating commands.
- [D] **CMD-INIT-ADOPT-001** — Adopting an existing `backlog.md` when `backlog/` is already present.
