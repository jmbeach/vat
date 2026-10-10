# Arrow: cli

CLI shell — argument parsing, help/version, error rendering, output streams, exit codes.

## Status

**OK** — verified 2026-10-10. `docs/specs/cli-specs.md` now formalizes the shell contract (10 CLI-* specs, all implemented). This pass closed three gaps: `vat --version` was rejected by clap (CLI-VER-001); `vat init` / `vat sync` rendered errors without the shared `error: ` prefix (CLI-ERR-001); cause chains repeated text that leaf errors already embed (CLI-ERR-003). Every dispatcher now fails through `fail()` in `src/main.rs`. Exit-code classification (CMD-EXIT-001..003) now covers every command, `cmd_init` and `cmd_sync` included.

## References

### HLD
- docs/high-level-design.md (§ System architecture)

### LLD
- docs/llds/cli.md

### EARS
- docs/specs/cli-specs.md (10 active specs: 10 implemented, 0 gaps)
- docs/specs/commands-specs.md (CMD-EXIT-001 to 003 — exit-code classification; owned by the `commands` arrow, table described in the CLI LLD)

### Tests
- tests/cli.rs — black-box CLI contract tests against the real binary (CLI-ARG-*, CLI-HELP-001, CLI-VER-001, CLI-ERR-*, CLI-OUT-*)
- tests/e2e_lifecycle.rs — black-box lifecycle tests (`init` → `sync` → `start` → `done`); exit codes (CMD-EXIT-001/002)
- tests/completions.rs — black-box `vat completions <shell>` exit-code/output tests
- src/main.rs (inline `#[cfg(test)]` — `classify_exit_code` per error variant, CMD-EXIT-002/003)

### Code
- src/main.rs — `Cli` / `Commands` / `ConfigAction` (CLI-ARG-*, CLI-HELP-001, CLI-VER-001, CLI-OUT-002); `fail()` + `render_error()` (CLI-ERR-001..003); `classify_exit_code()` / `classify_cause()` (CMD-EXIT-001..003)
- src/sync.rs — warning emission to stderr (CLI-OUT-001)
- src/errors.rs — `UserError`: typed wrapper for user-facing errors; exit-code classification anchor (CMD-EXIT-002)

## Architecture

**Purpose:** Outer shell of the `vat` binary. Argument parsing via clap derive macros, error propagation (thiserror-derived errors in leaf modules, anyhow in command bodies), one rendering/exit path for every command failure, stdout/stderr conventions.

**Key Components:**
1. `src/main.rs` — `Cli` struct (with `version`), `Commands` enum, one `cmd_*` dispatcher per subcommand, `fail()` (render + classify + exit), `classify_exit_code()`
2. Leaf error types — `ConfigError`, `UserConfigError`, `TombstoneError`, `UnsupportedVersion`, `UserError`, `InitError`, `SyncError` — matched exhaustively by `classify_cause` so each variant makes an explicit exit-code choice

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Argument parsing | CLI-ARG-001 to 003 | 3 | 0 | 0 |
| Help and version | CLI-HELP-001, CLI-VER-001 | 2 | 0 | 0 |
| Error rendering | CLI-ERR-001 to 003 | 3 | 0 | 0 |
| Output streams | CLI-OUT-001 to 002 | 2 | 0 | 0 |

**Summary:** 10 of 10 CLI specs implemented; 0 deferred; 0 gaps. Exit-code specs CMD-EXIT-001..003 are tracked in the `commands` arrow.

## Key Findings

1. **Exit code 2 is shared** between internal errors (`classify_exit_code`) and clap usage errors (CLI-ARG-002). Accepted for v1 — see the LLD's Decisions section.
2. **`#[error(transparent)]` hides wrapped errors from the cause chain.** `SyncError`'s transparent variants forward `source()` past the wrapped error, so `classify_cause` unwraps them directly instead of relying on the chain walk. Any new transparent wrapper needs the same treatment.
3. **Leaf errors embed their source in their own message.** Rather than rework every error type, `render_error` skips causes whose text is already in the line (CLI-ERR-003).

## Work Required

None — all active specs implemented.
