# Arrow: cli

CLI shell — argument parsing, error handling strategy, output conventions, exit codes.

## Status

**OK** — re-verified 2026-09-12 (HEAD `c265c95`). Shell completions wired; clap skeleton wired; thiserror+anyhow error pattern established. `docs/specs/cli-specs.md` created with 14 EARS requirements (CLI-ARG/HELP/ERR/OUT/PROMPT). Exit-code classification complete for all commands. The CLI shell is covered end-to-end by `tests/e2e_lifecycle.rs`.

## References

### HLD
- docs/high-level-design.md (§ System architecture)

### LLD
- docs/llds/cli.md

### EARS
- docs/specs/cli-specs.md (14 active specs: CLI-ARG-001 to 004, CLI-HELP-001 to 002, CLI-ERR-001 to 004, CLI-OUT-001 to 003, CLI-PROMPT-001 to 002; 1 deferred CLI-OUT-003)
- docs/specs/commands-specs.md (CMD-EXIT-001 to 003 — exit code specs cross-referenced)

### Tests
- tests/e2e_lifecycle.rs — black-box lifecycle tests (`init` → `sync` → `start` → `done`) spawning the real binary via `CARGO_BIN_EXE_vat`; assert stdout, exit codes (CMD-EXIT-001/002), and on-disk state with isolated `XDG_CONFIG_HOME`/`HOME` (vat-g4w)
- tests/completions.rs — black-box `vat completions <shell>` exit-code/output tests
- Leaf-module error types remain tested in their own modules

### Code
- src/main.rs — `Cli`, `Commands`, `ConfigAction` enums; dispatch match; error printing; `classify_exit_code()` (CMD-EXIT-001 to 003)
- src/errors.rs — `UserError`: typed wrapper for user-facing errors; exit-code classification anchor (CMD-EXIT-002)

## Architecture

**Purpose:** Outer shell of the `vat` binary. Argument parsing via clap derive macros, error propagation (thiserror-derived errors in leaf modules, anyhow in main), stdout/stderr conventions, and exit codes.

**Key Components:**
1. `src/main.rs` — top-level `Cli` struct, `Commands` enum, dispatch, error printing to stderr
2. Leaf error types — `Base32Error`, `TombstoneError`, `UserConfigError`, `ConfigError`, `SyncError` (thiserror-derived, in their respective modules) — these are the typed errors that clap callers match against before propagating via anyhow

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Argument parsing | CLI-ARG-001 to 004 | 4 | 0 | 0 |
| Help and version | CLI-HELP-001 to 002 | 2 | 0 | 0 |
| Error rendering | CLI-ERR-001 to 004 | 4 | 0 | 0 |
| Output conventions | CLI-OUT-001 to 003 | 2 | 1 | 0 |
| Interactive prompt | CLI-PROMPT-001 to 002 | 2 | 0 | 0 |

**Summary:** 14 of 15 CLI specs implemented; 1 deferred (CLI-OUT-003 colorization); 0 gaps.

## Key Findings

1. **`docs/specs/cli-specs.md` created** — Formalizes CLI behavioral requirements from LLD prose into 14 EARS specs covering argument parsing (CLI-ARG), help/version (CLI-HELP), error rendering (CLI-ERR), output conventions (CLI-OUT), and interactive prompt (CLI-PROMPT). CLI-OUT-003 (colorization) is explicitly deferred.

2. **Exit codes fully wired** — `classify_exit_code()` at `src/main.rs` classifies errors by chain-searching for typed variants. `cmd_init` uses a typed match on `InitError`; `cmd_sync` routes through `classify_sync_exit_code()` with exhaustive `SyncError` variant matching. Shared helpers `classify_config_error()` and `classify_tombstone_error()` ensure single source of truth per error type. All three exit codes are `@spec`-annotated with unit tests for every variant.

3. **Clap skeleton is complete** — All subcommands (`init`, `sync`, `start`, `block`, `unblock`, `done`, `config get`, `config set`) are wired with correct argument types. Help and version derive from clap defaults. The shell does not need changes to support new command implementations.

## Work Required

### Should Fix
1. Create `docs/specs/cli-specs.md` with EARS requirements for: argument parsing, error rendering format, output conventions, and the exit code table. Captures intent currently in LLD prose only.
2. Thread `classify_exit_code` through `cmd_init` and `cmd_sync` so CMD-EXIT-003 applies to every command, not just cmd_config operations.
