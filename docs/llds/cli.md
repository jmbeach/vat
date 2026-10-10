# LLD: CLI shell

Defines the binary's outer shell — the parts every command shares: argument parsing, error handling and rendering, exit codes, output conventions, help/version, and shell completions. Per-command behavior lives in [commands.md](./commands.md). On-disk file formats live in [backlog-format.md](./backlog-format.md). See [HLD](../high-level-design.md) for context.

## Argument parsing

VAT uses [`clap`](https://docs.rs/clap) v4 with the `derive` feature. The top-level enum lives in `src/main.rs`; each subcommand variant carries its own positional and flag arguments. Naming conventions:

- Subcommands are single lowercase verbs (`init`, `sync`, `start`, `block`, `unblock`, `done`, `config`).
- IDs are passed positionally as `<id>` strings; validation happens in the command body, not in clap, so error messages are uniform with other validation paths. A malformed or unknown ID is therefore a command error (exit 1), never a usage error (exit 2).
- `vat config` has its own `Subcommand` enum (`get`, `set`).
- Anything clap itself rejects — unknown subcommand, unknown flag, missing required argument, no subcommand at all — is a *usage error*: clap prints its own message plus a usage line to stderr and exits 2. VAT does not intercept or re-render these.

## Error handling

VAT splits error handling along the boundary between *leaf modules* and *the binary*:

- **Leaf modules** define typed errors with [`thiserror`](https://docs.rs/thiserror). Each module's domain errors are an enum with rich variants — e.g., `Base32Error::InvalidChar { ch, pos }`, `ConfigError::MissingProjectId`. Variants exist so callers can match on them when the kind of failure changes the rendering (e.g., `InvalidChar` lets `vat init` print a caret under the offending character).
- **The binary** uses [`anyhow`](https://docs.rs/anyhow) for propagation inside command bodies, with `.context(...)` at I/O boundaries so the user sees a breadcrumb trail, not just a leaf error. `main()` itself returns `()`: it dispatches to one small `cmd_*` function per subcommand, and each dispatcher is the single place that renders a failure and picks the exit code.

`anyhow::Error` has a blanket `From<E: std::error::Error>`, so any `thiserror`-derived enum auto-converts via `?` or `.into()`. Commands whose bodies return a typed error (`cmd_init::InitError`, `sync::SyncError`) are converted at the dispatcher, so every command goes through the same rendering and classification.

### Rendering

Every command failure is rendered by its dispatcher as exactly one line on stderr:

```
error: <message>[: <cause>]...
```

— `anyhow`'s alternate (`{:#}`) format, which joins the cause chain with `: `. One prefix for every command keeps failures greppable and makes the error line distinguishable from warnings that may precede it on stderr. Nothing is written to stdout on failure. We don't bring in `color-eyre` or multi-line `Caused by:` blocks in v1.

Where a leaf error variant carries enough information for a richer message, the command code matches on the variant *before* propagating and builds the friendlier text into the error itself — the dispatcher still prints it through the same `error: ` line.

## Exit codes

| Code | Meaning | Who decides |
|---|---|---|
| `0` | Success, including no-op cases | dispatcher (normal return) |
| `1` | User-facing error — the user can fix it by changing input or files (unknown id, missing config, validation failure, version mismatch, already initialized) | `classify_exit_code` |
| `2` | Internal error — file IO failure, unexpected parse failure, or anything unclassified | `classify_exit_code` |
| `2` | Usage error — clap rejected the command line | clap |

`classify_exit_code` (in `src/main.rs`) walks the error's cause chain and returns the first classification a known typed error makes; an error with no known typed link defaults to `2`. Each typed error enum is matched exhaustively, so a new variant forces an explicit exit-code decision. Wrapper errors declared `#[error(transparent)]` (e.g. most `SyncError` variants) forward `source()` *past* the wrapped error, so the chain never yields it; the classifier unwraps those variants directly rather than relying on the chain walk.

The behavioral exit-code requirements are `CMD-EXIT-001..003` in `docs/specs/commands-specs.md` (they predate this section and keep their IDs); the usage-error code is `CLI-ARG-002`.

**Code `2` is shared** between internal errors and clap usage errors. They are distinguishable by stderr shape (clap prints a `Usage:` line), and both mean "not something the user fixes by editing the backlog". Accepted for v1 rather than inventing a fourth code.

## Output conventions

- **Human output** goes to stdout: a command's result (`vat config get` prints the value) or a single-line success message where useful. Silence on success or no-op is fine. Per-command output content is owned by `commands.md` / `sync.md`, not this LLD.
- **Diagnostics, warnings, errors** go to stderr. Warnings (e.g., `vat sync`'s empty-bullet warnings) may precede an `error:` line or a successful exit.
- The one interactive prompt (`vat init` with no prefix) writes its prompt text to stdout, unterminated, so the cursor sits after it.
- **No colorization in v1** — text VAT itself writes contains no ANSI escape sequences, keeping output greppable and avoiding a `termcolor`-style dep. clap's own help and usage output keeps clap's default auto-detection (colored only on a terminal). Revisit if users ask.

## Help & version

Default clap behavior: `vat --help` / `-h` and `vat <subcmd> --help` print help to stdout and exit 0. `vat --version` / `-V` prints `vat <version>` to stdout and exits 0; the crate's `Cargo.toml` `version` is the source of truth, derived via `clap`'s `#[command(version)]`. The hidden `completions` subcommand stays out of help (CMD-COMP-003).

## Shell completions

VAT exposes completions via a hidden `vat completions <shell>` subcommand powered by [`clap_complete`](https://docs.rs/clap_complete). Supported shells: `bash`, `zsh`, `fish` — exactly these three, even though `clap_complete` itself also generates `elvish` and `powershell`.

- The subcommand is marked `#[command(hide = true)]` so it does not appear in `--help` output. `clap_complete`'s generators ignore `hide`, so the generation path additionally rebuilds the command tree from only the visible subcommands — `completions` never appears in a generated script either.
- On invocation, completions are written to stdout; the user pipes them to the appropriate location (e.g., `vat completions bash > /etc/bash_completion.d/vat`).
- The `shell` argument is a local `SupportedShell` `ValueEnum` (converted to `clap_complete::Shell` at the call site) so the accepted set is pinned to the spec and a `clap_complete` upgrade cannot widen it silently; unrecognised values produce clap's standard error with exit code 2.
- Generation goes through `Generator::try_generate` so write failures surface as errors (stderr + exit 2) instead of panicking; a broken pipe (e.g. `vat completions bash | head`) is treated as normal consumer behaviour and exits 0 silently.

## Decisions & alternatives

- **`thiserror` + `anyhow` split.** Leaf modules define typed errors with `thiserror` so callers can match on variants (e.g., to render a caret under a bad character); `main` uses `anyhow::Result<()>` with `.context(...)` at I/O boundaries for ergonomic propagation. Considered `anyhow`-only (loses variant matching) and `thiserror`-only with a hand-rolled top-level enum (more code, no benefit in a binary crate). Standard pattern in modern Rust CLIs.
- **No color in v1.** Greppable output and one fewer dep. Revisit if users ask.
- **Runtime completions subcommand, not build-time generation.** `clap_complete`'s build-time approach writes files to `OUT_DIR` during `cargo build`, requiring extra build.rs plumbing and complicating cross-compilation. A hidden runtime subcommand is simpler, self-contained, and lets release packagers run `vat completions bash` in a post-install script. Considered build-time generation (rejected: more complex, non-portable) and a top-level visible subcommand (rejected: clutters `--help` for the common case).
- **One `error: ` prefix for every command.** `vat init` and `vat sync` previously printed a bare message and a `vat sync: ` prefix respectively; unified so scripts and agents can match one shape. Considered per-command prefixes (`vat sync: ...`) — rejected: the command is already known to the caller, and two shapes means two patterns to match.
- **Exit code 2 shared by internal and usage errors.** See § Exit codes. Considered a distinct code (e.g. 64, `EX_USAGE`) for usage errors — rejected: clap's default is 2, overriding it buys little, and both cases are "not fixable by editing the backlog".
- **Validation in command bodies, not in clap.** Keeps error rendering uniform — every "bad input" path goes through the same typed-error machinery rather than splitting between clap's auto-generated messages and ours.
