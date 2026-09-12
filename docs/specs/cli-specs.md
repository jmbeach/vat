# EARS Specs: CLI shell

Requirements for the VAT binary's outer shell — argument parsing, help/version, error rendering, output conventions, shell completions, and exit codes. Per-command behavior lives in [commands-specs.md](./commands-specs.md). See [CLI LLD](../llds/cli.md) for design rationale.

Status: `[x]` implemented, `[ ]` active gap, `[D]` deferred.

## Argument parsing

- [x] **CLI-ARG-001** — The system shall parse subcommands as single lowercase verbs (`init`, `sync`, `start`, `block`, `unblock`, `done`, `config`).
- [x] **CLI-ARG-002** — IDs shall be passed positionally; the system shall validate them in the command body rather than in the argument parser so that error messages are uniform with other validation paths.
- [x] **CLI-ARG-003** — When an unknown subcommand or flag is passed, the system shall print a usage error to stderr and exit with code 2.
- [x] **CLI-ARG-004** — `vat config` shall expose its own `get` and `set` subcommands.

## Help and version

- [x] **CLI-HELP-001** — The system shall support `vat --help` and `vat <subcommand> --help`, printing usage information to stdout and exiting 0.
- [x] **CLI-HELP-002** — The system shall support `vat --version`, printing the crate version (from `Cargo.toml`) to stdout and exiting 0.

## Error rendering

- [x] **CLI-ERR-001** — All error messages shall be printed to stderr.
- [x] **CLI-ERR-002** — When a command propagates an `anyhow::Error`, the system shall print the full cause chain to stderr before exiting.
- [x] **CLI-ERR-003** — At I/O boundaries, the system shall add `.context(...)` breadcrumbs so the user sees a human-readable operation name alongside the OS error.
- [x] **CLI-ERR-004** — Where a typed error variant carries enough detail for a richer message (e.g., an invalid character with its position), the command shall match on the variant and print the richer form rather than the generic chain.

## Output conventions

- [x] **CLI-OUT-001** — Human-readable output (success messages, query results) shall be written to stdout.
- [x] **CLI-OUT-002** — Warnings and diagnostic messages shall be written to stderr.
- [D] **CLI-OUT-003** — The system shall not colorize output in v1; colorization is deferred until users request it.

## Interactive prompt

- [x] **CLI-PROMPT-001** — When `vat init` is invoked with no prefix argument, the system shall prompt the user interactively on stdout and read the prefix from stdin.
- [x] **CLI-PROMPT-002** — When stdin is closed or unreadable during the interactive prompt, the system shall print an error to stderr and exit 1.
