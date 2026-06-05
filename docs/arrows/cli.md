# Arrow: cli

The binary's outer shell — argument parsing, error handling strategy, exit codes, output conventions, help/version.

## Status

**MAPPED** — last audited 2026-06-05 (git SHA `426964053f024c0e1380a365543da31798536bb7`). Structure is known and clap-based argument parsing is fully wired in `src/main.rs`. No dedicated EARS spec file exists; exit-code specs live in `commands-specs.md`.

## References

### HLD
- docs/high-level-design.md (§ Commands)

### LLD
- docs/llds/cli.md

### EARS
- No dedicated cli-specs.md — exit codes are specified as CMD-EXIT-001 to CMD-EXIT-003 in docs/specs/commands-specs.md

### Tests
- No dedicated CLI-level integration tests; argument parsing is exercised implicitly via clap's derive macros

### Code
- src/main.rs (clap `Parser` + `Subcommand` derive; command dispatch; all command stubs)

## Architecture

**Purpose:** Provides the user-facing CLI surface: argument parsing, subcommand routing, and error presentation conventions. Does not own business logic.

**Key Components:**
1. `src/main.rs` — `Cli` struct with `#[derive(Parser)]`, `Commands` enum with `#[derive(Subcommand)]`, `ConfigAction` sub-enum; `fn main()` dispatches to command stubs

## Spec Coverage

No dedicated EARS spec file. Exit-code specs are in `commands-specs.md`:

| Category | Spec IDs | Implemented | Gaps | Deferred |
|----------|----------|-------------|------|----------|
| Exit codes (CMD-EXIT) | CMD-EXIT-001 to CMD-EXIT-003 | 0 | 3 | 0 |

**Summary:** 0 of 3 exit-code specs implemented (command stubs all exit(1) unconditionally; proper exit codes require command body implementation in the commands segment).

## Key Findings

1. **No cli-specs.md** — the CLI LLD describes exit codes, output conventions, and shell completions but there is no corresponding EARS spec file. Exit codes are split into `commands-specs.md` (CMD-EXIT-*). Output conventions and shell completion behavior are undocumented as EARS.

2. **Argument parsing fully implemented** — `src/main.rs` correctly wires all subcommands (init, sync, start, block, unblock, done, config get/set) via clap derive. Backlog item `vat-k1b` (shell completions) and `vat-c9s` (exit code table) are referenced in the LLD as deferred.

3. **thiserror + anyhow split not yet exercised** — the error-handling strategy (leaf modules use thiserror, main uses anyhow) is designed in cli.md but not yet exercised because all commands are stubs.

## Work Required

### Should Fix
1. Create `docs/specs/cli-specs.md` with EARS requirements for exit codes (migrating CMD-EXIT-* or cross-referencing), output conventions, and help/version behavior — so CLI-level behavior is verifiable independently of command implementations
