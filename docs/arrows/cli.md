# Arrow: cli

The binary's outer shell — clap argument parsing, error handling and rendering, exit codes, help/version, and output conventions.

## Status

**MAPPED** — last audited 2026-06-07 (git SHA `fe4be98`). Clap subcommand scaffold is in place in `src/main.rs`; all eight command handlers are stubs that print "not yet implemented" and exit(1). No `@spec` annotations in `main.rs` yet (appropriate until handlers are implemented).

## References

### HLD
- docs/high-level-design.md (§ Commands, § System architecture)

### LLD
- docs/llds/cli.md

### EARS
- (no dedicated spec file; exit-code specs CMD-EXIT-001..003 live in docs/specs/commands-specs.md and are counted in the `commands` segment)

### Tests
- (none yet)

### Code
- src/main.rs — `Cli`, `Commands`, `ConfigAction` clap structs; `main()` dispatch; eight `cmd_*` stubs

## Architecture

**Purpose:** Entry point and command dispatcher. Uses `clap` derive API to define the full CLI surface; delegates to leaf modules and (once implemented) command-handler logic.

**Key Components:**
1. `Cli` / `Commands` / `ConfigAction` — clap derive structs defining the subcommand tree
2. `main()` — match dispatch routing each subcommand variant to its `cmd_*` handler
3. Eight `cmd_*` stub functions — `cmd_init`, `cmd_sync`, `cmd_start`, `cmd_block`, `cmd_unblock`, `cmd_done`, `cmd_config_get`, `cmd_config_set`; all currently print to stderr and call `std::process::exit(1)`

## Spec Coverage

No dedicated EARS spec file for this segment. Exit-code specs (CMD-EXIT-001..003) live in `commands-specs.md` and are counted in the `commands` segment.

| Category | Notes |
|----------|-------|
| Arg parsing | Clap subcommand scaffold complete; no dedicated specs |
| Error handling | `thiserror` + `anyhow` split described in LLD; not yet wired in main |
| Exit codes | CMD-EXIT-001..003 in commands-specs.md; 0 implemented |
| Help/version | `--help` via clap default; `--version` not yet exposed (no `version` attribute in main.rs); no explicit specs |
| Shell completions | Deferred (backlog item vat-k1b) |

## Key Findings

1. **All stubs exit(1) unconditionally** — `src/main.rs:92–130`. The LLD calls for `anyhow::Result<()>` propagation with `.context(...)` at I/O boundaries; this is not yet wired. Stubs should be replaced with `anyhow`-returning functions.
2. **No `@spec` annotations in main.rs** — appropriate for now. Once command handlers are implemented, add `// @spec CMD-EXIT-001` (or similar) at the exit-code enforcement point in `main`.
3. **LLD references two deferred backlog items** — `vat-c9s` (full exit-code table) and `vat-k1b` (shell completions via `clap_complete`); both remain open.
4. **`--version` is not yet exposed** — `src/main.rs`'s `#[command(...)]` block sets only `name` and `about`; there is no `version` attribute, so clap does not wire up `--version` today. `--help` works (clap default). Adding the one-word `version` attribute (which pulls the version from `Cargo.toml` at compile time) would expose it when desired; no spec needed.

## Work Required

### Must Fix
1. Replace `std::process::exit(1)` stubs with `anyhow::Result<()>`-propagating handlers (prerequisite for clean error rendering per cli.md § Rendering)
2. Implement exit code table per CMD-EXIT-001..003 (currently all errors map to exit 1)

### Nice to Have
1. Shell completions via `clap_complete` (deferred — vat-k1b)
