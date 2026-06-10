# Arrow: cli

CLI shell — argument parsing, error handling strategy, output conventions, exit codes.

## Status

**MAPPED** — last audited 2026-06-10 (git SHA `17e8914`). Clap skeleton wired; thiserror+anyhow error pattern established. No dedicated EARS spec file; behavioral exit codes live in `commands-specs.md`. Exit code 2 (internal error) is defined in the LLD but never emitted.

## References

### HLD
- docs/high-level-design.md (§ System architecture)

### LLD
- docs/llds/cli.md

### EARS
- docs/specs/commands-specs.md (CMD-EXIT-001 to 003 — exit code specs; no dedicated cli-specs.md)

### Tests
- None for CLI shell itself (leaf-module error types are tested in their own modules)

### Code
- src/main.rs — `Cli`, `Commands`, `ConfigAction` enums; dispatch match; error printing

## Architecture

**Purpose:** Outer shell of the `vat` binary. Argument parsing via clap derive macros, error propagation (thiserror-derived errors in leaf modules, anyhow in main), stdout/stderr conventions, and exit codes.

**Key Components:**
1. `src/main.rs` — top-level `Cli` struct, `Commands` enum, dispatch, error printing to stderr
2. Leaf error types — `Base32Error`, `TombstoneError`, `UserConfigError`, `ConfigError`, `SyncError` (thiserror-derived, in their respective modules) — these are the typed errors that clap callers match against before propagating via anyhow

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Exit codes | CMD-EXIT-001 to 003 | 0 | 0 | 3 |

**Summary:** 0 of 3 exit-code specs formally verified; no deferred. Exit code 0 and 1 are in practice used by `cmd_config.rs` but are not annotated with `@spec CMD-EXIT-*`. Exit code 2 is defined in the LLD but never emitted.

*Note: CLI argument-parsing and error-rendering behaviors are specified only in LLD prose (`docs/llds/cli.md`), not as EARS requirements. No `docs/specs/cli-specs.md` exists.*

## Key Findings

1. **No dedicated EARS spec file** — `docs/llds/cli.md` has no matching `docs/specs/cli-specs.md`. Argument-parsing behavior (subcommand structure, ID positional args, clap conventions) and error-rendering behavior are documented only in LLD prose. Exit-code specs landed in `commands-specs.md` rather than a dedicated CLI spec. This is a gap in the intent chain.

2. **Exit code 2 never emitted** — `docs/llds/cli.md` and `docs/llds/commands.md` both define exit code 2 for internal errors (file I/O failures, unexpected parse failures), but `src/main.rs` and `src/cmd_config.rs` only exit with 0 or 1. No `@spec CMD-EXIT-003` annotation exists anywhere. All `anyhow::Error` propagation currently falls through to exit 1.

3. **Clap skeleton is complete** — All subcommands (`init`, `sync`, `start`, `block`, `unblock`, `done`, `config get`, `config set`) are wired with correct argument types. Help and version derive from clap defaults. The shell does not need changes to support new command implementations.

## Work Required

### Should Fix
1. Create `docs/specs/cli-specs.md` with EARS requirements for: argument parsing, error rendering format, output conventions, and the exit code table. Captures intent currently in LLD prose only.
2. Emit exit code 2 for internal errors (`anyhow::Error` paths in main.rs). Add `@spec CMD-EXIT-003` annotation once the spec is formalized.

### Nice to Have
3. Add `@spec CMD-EXIT-001` / `CMD-EXIT-002` annotations to `src/main.rs` and `src/cmd_config.rs` once exit code handling is complete.
