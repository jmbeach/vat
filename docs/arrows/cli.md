# Arrow: cli

CLI shell: argument parsing, error handling and rendering, exit codes, output conventions, help/version, shell completions.

## Status

**MAPPED** — last audited 2026-06-04 (git SHA `426964053f024c0e1380a365543da31798536bb7`). Clap derive structure in src/main.rs is complete; all subcommand variants are wired to stub handlers. No dedicated spec file exists for this LLD. Exit-code and cross-cutting specs live in commands-specs.md (CMD-EXIT-*, CMD-CC-*).

## References

### HLD
- docs/high-level-design.md (§ System architecture)

### LLD
- docs/llds/cli.md

### EARS
- No dedicated cli-specs.md. Related specs: docs/specs/commands-specs.md (CMD-EXIT-001–003, CMD-CC-001–003)

### Tests
- None (no cli-specific test coverage)

### Code
- src/main.rs — Cli/Commands/ConfigAction clap structs; main() dispatch; stub handlers

## Architecture

**Purpose:** Binary entry point. Parses arguments with clap, dispatches to per-command handlers, defines error-rendering conventions and exit codes.

**Key Components:**
1. `Cli` / `Commands` / `ConfigAction` — clap derive structs (src/main.rs:17–74)
2. `main()` dispatch — routes Commands enum variants to cmd_* functions (src/main.rs:76–90)
3. Error handling model — `thiserror` in leaf modules; `anyhow` at the binary boundary (described in LLD, not yet wired since handlers are stubs)
4. Exit codes — 0 success, 1 user-facing error, 2 internal error (CMD-EXIT-001–003; not yet enforced since stubs all exit 1)

## Spec Coverage

No dedicated spec file. Coverage of CMD-EXIT-* and CMD-CC-* is tracked under the `commands` segment.

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Cross-cutting | CMD-CC-001 (in commands-specs.md) | 1 | 0 | 2 |
| Exit codes | CMD-EXIT-001–003 (in commands-specs.md) | 0 | 0 | 3 |

**Summary:** Clap argument structure is complete. Error-handling and exit-code machinery is designed but not yet wired (handlers are stubs). No cli-specific specs file.

## Key Findings

1. **No cli-specs.md** — The CLI LLD covers exit codes, error rendering, output conventions, and shell completions but has no dedicated EARS spec file. Exit-code specs are embedded in commands-specs.md; the rest are undocumented in spec form.
2. **main.rs has no @spec annotations** — The clap dispatch structure implements the argument-parsing contract described in cli.md but carries no `@spec` links. Once command handlers are wired and CMD-CC-001 is called per handler, the relevant spec IDs should be annotated.
3. **Shell completions deferred** — LLD references backlog item `vat-k1b`; design is undecided.
4. **Exit codes not yet enforced** — All stubs exit with code 1. CMD-EXIT-002 (user error → 1) and CMD-EXIT-003 (internal error → 2) will need to be distinguishable once handlers are implemented.

## Work Required

### Must Fix
1. Create `docs/specs/cli-specs.md` covering: exit-code table (CMD-EXIT-*), error-rendering conventions, output conventions (stdout vs stderr), help/version behavior — move CMD-EXIT-* specs there from commands-specs.md or cross-reference

### Should Fix
2. Annotate src/main.rs clap structs with `@spec` once the cli spec file exists
3. Wire `anyhow` error propagation and exit-code differentiation when handlers are implemented

### Nice to Have
4. Decide on shell completions approach (vat-k1b backlog item) and document in LLD
