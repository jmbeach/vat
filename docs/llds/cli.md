# LLD: CLI shell

Defines the binary's outer shell — the parts every command shares: argument parsing, error handling and rendering, exit codes, output conventions, help/version, and shell completions. Per-command behavior lives in [commands.md](./commands.md). On-disk file formats live in [backlog-format.md](./backlog-format.md). See [HLD](../high-level-design.md) for context.

## Argument parsing

VAT uses [`clap`](https://docs.rs/clap) v4 with the `derive` feature. The top-level enum lives in `src/main.rs` (already scaffolded); each subcommand variant carries its own positional and flag arguments. Naming conventions:

- Subcommands are single lowercase verbs (`init`, `sync`, `start`, `block`, `unblock`, `done`, `config`).
- IDs are passed positionally as `<id>` strings; validation happens in the command body, not in clap, so error messages are uniform with other validation paths.
- `vat config` has its own `Subcommand` enum (`get`, `set`).

## Error handling

VAT splits error handling along the boundary between *leaf modules* and *the binary*:

- **Leaf modules** define typed errors with [`thiserror`](https://docs.rs/thiserror). Each module's domain errors are an enum with rich variants — e.g., `Base32Error::InvalidChar { ch, pos }`, `ConfigError::MissingProjectId`. Variants exist so callers can match on them when the kind of failure changes the rendering (e.g., `InvalidChar` lets `vat init` print a caret under the offending character).
- **The binary** uses [`anyhow`](https://docs.rs/anyhow). `fn main() -> anyhow::Result<()>` and `?`-propagation throughout. At I/O boundaries — `read_to_string`, `write`, `parse_toml` — we add `.context("reading vat.toml")` so the user sees a breadcrumb trail, not just a leaf error.

`anyhow::Error` has a blanket `From<E: std::error::Error>`, so any `thiserror`-derived enum auto-converts via `?`. We get typed errors where matching matters and ergonomic propagation everywhere else.

### Rendering

Errors print to stderr. The default `anyhow` rendering ("error: <top>\n\nCaused by:\n  <chain>") is acceptable for v1; we don't bring in `color-eyre` yet. Where a leaf error variant carries enough information for a richer message (e.g., `Base32Error::InvalidChar { ch, pos }`), the command code matches on the variant *before* propagating, prints a friendlier message, and then exits — rather than letting `anyhow` print the generic chain.

## Exit codes

(See backlog item `vat-c9s`. Detail to fill in when that task is designed.)

Sketch:

- `0` — success
- `1` — user-facing error (unknown id, missing config, validation failure). This is the catch-all for `anyhow`-bubbled errors.
- `2` — usage error from clap (bad flags, missing arg). clap chooses this exit code itself.

`commands.md` already commits to `1` for user-facing errors; this LLD owns the full table when filled in.

## Output conventions

- **Human output** goes to stdout. Single-line success messages where useful (e.g., `vat sync` prints `assigned 3 ids`); silence on no-op is fine.
- **Diagnostics, warnings, errors** go to stderr.
- **No colorization in v1** — keeps output greppable in pipelines and avoids a `termcolor`-style dep. Revisit if users ask.

## Help & version

Default clap behavior is sufficient: `vat --help`, `vat <subcmd> --help`, `vat --version`. The crate's `Cargo.toml` `version` is the source of truth; `clap`'s `#[command(version)]` derives the flag.

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
- **Validation in command bodies, not in clap.** Keeps error rendering uniform — every "bad input" path goes through the same typed-error machinery rather than splitting between clap's auto-generated messages and ours.
