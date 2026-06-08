# Arrow: cli

CLI shell — argument parsing, error handling strategy, exit codes, output conventions, help/version. Shared machinery for all commands.

## Status

**PARTIAL** — last audited 2026-06-08 (git SHA `e2c7ad8cf75a7da4a970a1eedfb8b6e5784d4c14`). The `clap`-derived command structure is fully scaffolded in `src/main.rs`. All command dispatch arms exist. All command bodies are stubs that print "not yet implemented" and exit 1. No EARS specs are written for this segment specifically; CLI behavior requirements appear in `commands-specs.md` (CMD-CC-*, CMD-EXIT-*).

## References

### HLD
- docs/high-level-design.md (§ Commands table, § Key design decisions)

### LLD
- docs/llds/cli.md

### EARS
- No dedicated EARS spec file for this segment.
- Related: docs/specs/commands-specs.md § Cross-cutting (CMD-CC-001..003) and § Exit codes (CMD-EXIT-001..003)

### Tests
- src/main.rs (inline tests, if any)

### Code
- src/main.rs — `Cli` + `Commands` + `ConfigAction` clap derives; `main()` dispatch; stub command bodies

## Architecture

**Purpose:** The binary's outer shell. Owns argument parsing (via `clap` v4 derive), the split between typed leaf-module errors (`thiserror`) and top-level propagation (`anyhow`), exit code conventions, and stdout/stderr routing.

**Key Components:**
1. `Cli` / `Commands` enum — clap-derived argument parser; one variant per subcommand
2. `ConfigAction` enum — clap-derived sub-subcommands for `vat config`
3. `main()` — dispatch table routing parsed subcommand variants to command functions
4. Command stubs — `cmd_init`, `cmd_sync`, `cmd_start`, `cmd_block`, `cmd_unblock`, `cmd_done`, `cmd_config_get`, `cmd_config_set`

## Spec Coverage

No dedicated spec file. CLI-adjacent specs live in commands-specs.md:

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Cross-cutting (CMD-CC) | CMD-CC-001..003 | 1 | 0 | 2 |
| Exit codes (CMD-EXIT) | CMD-EXIT-001..003 | 0 | 0 | 3 |

**Summary:** 1 of 6 relevant specs implemented (CMD-CC-001 via `backlog_file.rs` version check); 5 active gaps. Exit code specs (CMD-EXIT-001..003) require all command bodies to be wired before they can be verified.

## Key Findings

1. **All command bodies are stubs** — Every `cmd_*` function in `main.rs` is `eprintln!("...: not yet implemented"); std::process::exit(1)`. The dispatch table and argument shapes are correct; the implementations need to call the library modules.

2. **No dedicated EARS spec file** — CLI output conventions (no color, human output to stdout, diagnostics to stderr) and shell completions are design decisions in the LLD but not captured as verifiable EARS requirements. This is a gap if the project wants them testable; acceptable if the team is comfortable treating the LLD as sufficient.

3. **Exit code for "not yet implemented" stubs** — Current stubs exit 1, which per CMD-EXIT-002 is the user-facing-error code. Once commands are implemented, the exit code semantics will need to be verified correctly for each case (0, 1, or 2).

## Work Required

### Must Fix
1. Wire each `cmd_*` stub to its corresponding library module implementation

### Should Fix
2. Verify CMD-EXIT-001..003 exit codes as each command is implemented

### Nice to Have
3. Consider authoring EARS specs for CLI-specific behavior (output conventions, shell completions) if testability is desired
