# Arrow: cli

The binary's outer shell — argument parsing (clap), error handling, exit codes, output conventions, help/version, and shell completions.

## Status

**PARTIAL** — last audited 2026-06-06 (git SHA `426964053f024c0e1380a365543da31798536bb7`). CLI structure and argument parsing are scaffolded; all command bodies are stubs. No EARS spec file exists for this segment — user decision required on whether to create one.

## References

### HLD
- docs/high-level-design.md (§ Commands)

### LLD
- docs/llds/cli.md

### EARS
- *(none — no cli-specs.md exists)*

### Tests
- *(none — CLI shell has no dedicated tests; argument parsing is exercised by clap's own machinery)*

### Code
- src/main.rs (argument parsing, dispatch table, all command stubs)

## Architecture

**Purpose:** Wire user-facing commands to the binary entry point; define error handling and exit code conventions shared by all commands.

**Key Components:**
1. `src/main.rs` — `Cli` struct, `Commands` enum, `ConfigAction` enum (clap `derive`), `main()` dispatch, stub command functions

## Spec Coverage

*(No EARS spec file for this segment.)*

**Note:** The LLD defers exit codes to backlog item `vat-c9s` and shell completions to `vat-k1b`. If EARS coverage of CLI-shell behaviors (error rendering, exit codes, help, completions) is desired, a `cli-specs.md` file should be created.

## Key Findings

1. **No EARS spec file** — `docs/llds/cli.md` exists but no `docs/specs/cli-specs.md`. Exit code table and shell-completion behavior are currently spec'd only in prose. This is a user decision: create `cli-specs.md` or leave CLI-shell behaviors spec-less.
2. **All command bodies are stubs** — Every `cmd_*` function in `main.rs` prints "not yet implemented" and exits 1. The dispatch table is correct; actual logic lives in `commands` and `sync` segments.

## Work Required

### Must Fix
*(none — CLI shell structure is complete for scaffolding phase)*

### Should Fix
1. Implement command bodies once `commands` and `sync` segments are implemented

### Nice to Have
2. Create `cli-specs.md` if EARS coverage of exit codes, error rendering, and shell completions is desired
