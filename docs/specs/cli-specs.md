# EARS Specs: CLI shell

Requirements for the parts of the `vat` binary every command shares: argument parsing, help and version, error rendering, and output streams. See [CLI LLD](../llds/cli.md). Exit-code classification for command errors is specified by `CMD-EXIT-001..003` in [commands-specs.md](./commands-specs.md).

Status: `[x]` implemented, `[ ]` active gap, `[D]` deferred.

## Argument parsing

- [x] **CLI-ARG-001** — `vat --help` shall list exactly the subcommands `init`, `sync`, `start`, `block`, `unblock`, `done`, `config`, and clap's built-in `help`.
- [x] **CLI-ARG-002** — When invoked with no subcommand, an unrecognised subcommand, an unrecognised flag, or a missing required argument, the system shall print a usage error to stderr, write nothing to stdout, and exit with code 2.
- [x] **CLI-ARG-003** — When a task-ID argument is syntactically well-formed for clap but invalid or unknown to VAT, the system shall report it as a command error (exit code 1 per CMD-EXIT-002), not as a usage error.

## Help and version

- [x] **CLI-HELP-001** — When invoked with `--help` or `-h`, at the top level or on any subcommand, the system shall print help text to stdout and exit with code 0.
- [x] **CLI-VER-001** — When invoked with `--version` or `-V`, the system shall print `vat <version>` to stdout, where `<version>` is the crate version from `Cargo.toml`, and exit with code 0.

## Error rendering

- [x] **CLI-ERR-001** — When a command fails, the system shall write the error to stderr as its final output, beginning `error: ` and followed by the error message and then each underlying cause, separated by `: `. Warnings emitted earlier in the same run may precede it.
- [x] **CLI-ERR-002** — When a command fails, the system shall write nothing to stdout.
- [x] **CLI-ERR-003** — When rendering a failed command's error, the system shall omit any underlying cause whose text already appears earlier in the rendered message.

## Output streams

- [x] **CLI-OUT-001** — The system shall write warnings and diagnostics to stderr, never to stdout.
- [x] **CLI-OUT-002** — Text the system writes itself (results, messages, warnings, errors) shall contain no ANSI escape sequences.
