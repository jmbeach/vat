# Arrow: cli

CLI shell — argument parsing, error handling strategy, output conventions, exit codes.

## Status

**MAPPED** — re-verified 2026-09-06 (HEAD `c265c95`); previously 2026-06-21 (HEAD `aab182c`). Shell completions wired (vat-f7v): hidden `Completions` subcommand in `src/main.rs`, implemented by `src/cmd_completions.rs`. Clap skeleton wired; thiserror+anyhow error pattern established. No dedicated EARS spec file; behavioral exit codes live in `commands-specs.md`. `classify_exit_code()` is now wired for `cmd_config`, `cmd_start`, `cmd_block`, `cmd_unblock`, and `cmd_done`; `cmd_init` and `cmd_sync` still exit 1 for all errors. The CLI shell is covered end-to-end by `tests/e2e_lifecycle.rs`.

## References

### HLD
- docs/high-level-design.md (§ System architecture)

### LLD
- docs/llds/cli.md

### EARS
- docs/specs/commands-specs.md (CMD-EXIT-001 to 003 — exit code specs; no dedicated cli-specs.md)

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
| Exit codes | CMD-EXIT-001 to 003 | 2 | 0 | 1 |

**Summary:** 2 of 3 exit-code specs implemented; 1 gap; no deferred. CMD-EXIT-001 and CMD-EXIT-002 hold for every command. CMD-EXIT-003 is marked `[x]` in `commands-specs.md` but holds only for `cmd_config` operations — `cmd_init` and `cmd_sync` exit 1 for internal errors too (see Key Finding #2 and the `commands` drift entry in `index.yaml`).

*Note: CLI argument-parsing and error-rendering behaviors are specified only in LLD prose (`docs/llds/cli.md`), not as EARS requirements. No `docs/specs/cli-specs.md` exists.*

## Key Findings

1. **No dedicated EARS spec file** — `docs/llds/cli.md` has no matching `docs/specs/cli-specs.md`. Argument-parsing behavior (subcommand structure, ID positional args, clap conventions) and error-rendering behavior are documented only in LLD prose. Exit-code specs landed in `commands-specs.md` rather than a dedicated CLI spec. This is a gap in the intent chain.

2. **Exit codes partially wired** — `classify_exit_code()` at `src/main.rs:247` classifies errors by chain-searching for typed variants (`ConfigError`, `UserConfigError`, `UnsupportedVersion`, `UserError`) and maps them to exit 1 (user-facing) or 2 (internal/IO). `UserError` in `src/errors.rs` lifts untyped `bail!` messages into the classification scheme. All three exit codes are `@spec`-annotated. Wired through `cmd_config_get`/`cmd_config_set`, `cmd_start`, `cmd_block`, `cmd_unblock`, and `cmd_done`. `cmd_init` and `cmd_sync` still hardcode `std::process::exit(1)` for all errors (CMD-EXIT-003 gap).

3. **Clap skeleton is complete** — All subcommands (`init`, `sync`, `start`, `block`, `unblock`, `done`, `config get`, `config set`) are wired with correct argument types. Help and version derive from clap defaults. The shell does not need changes to support new command implementations.

## Work Required

### Should Fix
1. Create `docs/specs/cli-specs.md` with EARS requirements for: argument parsing, error rendering format, output conventions, and the exit code table. Captures intent currently in LLD prose only.
2. Thread `classify_exit_code` through `cmd_init` and `cmd_sync` so CMD-EXIT-003 applies to every command, not just cmd_config operations.
