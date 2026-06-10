# EARS Specs: Commands other than sync

Requirements for `vat init`, `vat start`, `vat block`, `vat unblock`, `vat done`, and `vat config`. See [commands LLD](../llds/commands.md).

Status: `[x]` implemented, `[ ]` active gap, `[D]` deferred.

## Cross-cutting

- [x] **CMD-CC-001** — Every command that reads `backlog.md` shall first verify that the file's frontmatter `version` does not exceed the CLI's supported major version, and shall abort with an error before any other processing if it does.
- [ ] **CMD-CC-002** — When a bullet-mutating command cannot find a bullet matching the supplied `<id>`, the system shall abort with an error and shall not write to any file.
- [ ] **CMD-CC-003** — When a bullet-mutating command writes a bullet, the system shall emit markers in the canonical order defined by FMT-MARK-004.

## `vat init`

- [x] **CMD-INIT-001** — When `backlog/` already exists, `vat init` shall abort with an error.
- [x] **CMD-INIT-002** — When invoked with `vat init <prefix>`, the system shall use `<prefix>` as the project ID prefix.
- [x] **CMD-INIT-003** — When invoked with no argument, `vat init` shall prompt the user interactively for the project ID prefix.
- [x] **CMD-INIT-004** — `vat init` shall reject any prefix that is not exactly 3 characters in the Crockford base32 alphabet.
- [x] **CMD-INIT-005** — On success, `vat init` shall create `backlog/`, `backlog/vat.toml` containing `[project] id = "<prefix>"`, `backlog/backlog.md` containing only a `version: 1` frontmatter block, an empty `backlog/.used-ids`, and `backlog/README.md`.
- [x] **CMD-INIT-006** — `backlog/README.md` shall describe what VAT is, how to obtain it, the purpose of each file in `backlog/`, and the basic workflow. (Template baked into the binary as `readme_template::BACKLOG_README_TEMPLATE`; `vat init` renders and writes it via `readme_template::render`.)
- [x] **CMD-INIT-007** — After init, no VAT command shall read, validate, or rewrite `backlog/README.md`.

## `vat start <id>`

- [ ] **CMD-START-001** — When `user.name` is unset in the user config, `vat start` shall abort with an error pointing the user at `vat config set user.name <name>`.
- [ ] **CMD-START-002** — When the target bullet has either an `[in-progress]` marker or a `[by:...]` marker, `vat start` shall abort with an error indicating the existing claim.
- [ ] **CMD-START-003** — On success, `vat start` shall add both `[in-progress]` and `[by:<user.name>]` markers to the target bullet.

## `vat block <id> <blocker-id>`

- [ ] **CMD-BLOCK-001** — When `<id>` equals `<blocker-id>`, `vat block` shall abort with an error.
- [ ] **CMD-BLOCK-002** — When no bullet matches `<blocker-id>`, `vat block` shall abort with an error.
- [ ] **CMD-BLOCK-003** — When the target bullet already has `[blocked-by:<blocker-id>]` matching the supplied blocker, `vat block` shall succeed without modifying any file.
- [ ] **CMD-BLOCK-004** — When the target bullet has `[blocked-by:<other>]` for a different blocker, `vat block` shall replace it with `[blocked-by:<blocker-id>]`.
- [ ] **CMD-BLOCK-005** — When the target bullet has no `[blocked-by:...]` marker, `vat block` shall add `[blocked-by:<blocker-id>]` in canonical position.
- [ ] **CMD-BLOCK-006** — `vat block` in v1 shall not detect blocker cycles.

## `vat unblock <id>`

- [ ] **CMD-UNBLOCK-001** — When the target bullet has no `[blocked-by:...]` marker, `vat unblock` shall succeed without modifying any file.
- [ ] **CMD-UNBLOCK-002** — When the target bullet has a `[blocked-by:...]` marker, `vat unblock` shall remove that marker.

## `vat done <id>`

- [ ] **CMD-DONE-001** — `vat done` shall remove the entire bullet line for `<id>` from `backlog.md`.
- [ ] **CMD-DONE-002** — When `backlog/items/<id>.md` exists, `vat done` shall delete it.
- [ ] **CMD-DONE-003** — `vat done` shall append `<id>` to `backlog/.used-ids` if it is not already present.
- [ ] **CMD-DONE-004** — `vat done` shall remove every `[blocked-by:<id>]` marker from any other bullet in the parsed region.
- [ ] **CMD-DONE-005** — `vat done` shall succeed even when the target bullet has its own `[blocked-by:...]` marker.

## `vat config`

- [x] **CMD-CFG-001** — `vat config get user.name` shall print the value from the user config to stdout and exit 0, or exit 0 with no output if unset; exit 1 only on hard errors (I/O failure, unknown key).
- [x] **CMD-CFG-002** — `vat config get project.id` shall print the value from `backlog/vat.toml` to stdout and exit 0, or exit 0 with no output if unset; exit 1 only on hard errors (I/O failure, unknown key).
- [x] **CMD-CFG-003** — `vat config set user.name <value>` shall write the value to the user config, creating the file and parent directories if needed.
- [x] **CMD-CFG-004** — `vat config set project.id <value>` shall validate `<value>` as 3 Crockford base32 characters.
- [x] **CMD-CFG-005** — `vat config set project.id <value>` shall abort with an error if any IDs in `backlog.md` or `backlog/.used-ids` use a different prefix.
- [x] **CMD-CFG-006** — `vat config set` shall reject keys other than `user.name` and `project.id` with an error.

## Exit codes

- [x] **CMD-EXIT-001** — On success, every command shall exit with code 0.
- [x] **CMD-EXIT-002** — On user-facing errors (unknown ID, missing config, validation failure, version mismatch), every command shall exit with code 1.
- [x] **CMD-EXIT-003** — On internal errors (file IO failure, unexpected parse failure), every command shall exit with code 2.

## Out of scope for v1

- [D] **CMD-LOCK-001** — Concurrent-process locking on the backlog directory.
- [D] **CMD-FORCE-001** — `--force` overrides on guarded commands.
- [D] **CMD-DRYRUN-001** — `--dry-run` mode on mutating commands.
- [D] **CMD-INIT-ADOPT-001** — Adopting an existing `backlog.md` when `backlog/` is already present.
