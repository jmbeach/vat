# Arrow: cli

The binary's outer shell: argument parsing (clap), error handling split (thiserror in modules / anyhow in main), exit codes, output conventions, help/version, and future shell completions.

## Status

**MAPPED** — last audited 2026-06-03 (git SHA `52bbfb58a6f7f999969da68bef55b38bd59fb744`). LLD is complete and detailed. No EARS spec file exists yet. `main.rs` has the clap enum scaffolded and all command dispatch wired, but every `cmd_*` function is a stub that prints "not yet implemented" and exits 1.

## References

### HLD
- docs/high-level-design.md (§ Commands)

### LLD
- docs/llds/cli.md

### EARS
- (none — no `docs/specs/cli-specs.md` exists yet)

### Tests
- (none — no behavior to test in stubs)

### Code
- src/main.rs — Cli struct, Commands enum, ConfigAction enum, main(), cmd_* stubs

## Architecture

**Purpose:** Parses the command line, dispatches to the correct command function, renders errors to stderr, and sets the process exit code.

**Key Components:**
1. `Cli` struct + `Commands` enum (clap derive) — subcommand routing for init, sync, start, block, unblock, done, config
2. `ConfigAction` enum — nested subcommand for `vat config get` / `vat config set`
3. Error rendering — anyhow in main with `.context(...)` at I/O boundaries; leaf modules use thiserror
4. Exit codes — 0 success, 1 user-facing error, 2 clap usage error (to be formalized in a spec)

## Spec Coverage

No EARS spec file exists for this segment. Behavioral requirements (exit codes, output conventions) are documented in prose in `docs/llds/cli.md`.

**Summary:** 0 specs tracked; spec file needs to be created.

## Key Findings

1. **No EARS spec file** — `docs/llds/cli.md` fully describes CLI behavior, but no `docs/specs/cli-specs.md` exists. Exit codes, output conventions, and help/version behavior are untracked by structured requirements. Recommend creating the spec file; see Work Required.

2. **No @spec annotations in main.rs** — The clap scaffolding and dispatch loop have no `@spec` links. Once commands are implemented, annotations should be added at each `cmd_*` entry point linking to CMD-* and SYNC-* spec IDs.

3. **Shell completions deferred** — `docs/llds/cli.md` references backlog item `vat-k1b` for `clap_complete` shell completions; this is deferred and not tracked in the spec file.

4. **Exit code table is a sketch** — The LLD notes exit code 2 for internal errors but `cli.md` says "detail to fill in when that task is designed" (referencing backlog item `vat-c9s`). Once formalized, the exit-code table belongs in `cli-specs.md`.

## Work Required

### Must Fix
1. Create `docs/specs/cli-specs.md` — capture exit codes, output conventions, error rendering, and help/version as EARS requirements (no design decision needed; the LLD already has the prose)
2. Add `@spec` annotations to `cmd_*` functions in `main.rs` as each is implemented

### Should Fix
3. Formalize exit-code table (currently a sketch; backlog item `vat-c9s`)

### Nice to Have
4. Shell completions via `clap_complete` (backlog item `vat-k1b`)
