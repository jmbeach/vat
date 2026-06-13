use std::io::{self, Write};

use clap::{CommandFactory as _, ValueEnum};
use clap_complete::{Generator as _, Shell};

use crate::Cli;

/// Shells `vat completions` accepts. Deliberately narrower than
/// `clap_complete::Shell`: the contract is exactly bash/zsh/fish, and a
/// semver-compatible `clap_complete` upgrade must not widen it silently.
// @spec CMD-COMP-002
#[derive(Clone, Copy, ValueEnum)]
pub enum SupportedShell {
    Bash,
    Zsh,
    Fish,
}

impl From<SupportedShell> for Shell {
    fn from(shell: SupportedShell) -> Self {
        match shell {
            SupportedShell::Bash => Shell::Bash,
            SupportedShell::Zsh => Shell::Zsh,
            SupportedShell::Fish => Shell::Fish,
        }
    }
}

// @spec CMD-COMP-001, CMD-COMP-002, CMD-COMP-005
pub fn run(shell: SupportedShell) -> io::Result<()> {
    // Hold the stdout lock for the whole script rather than re-acquiring it
    // per write inside the generator.
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    run_impl(shell, &mut lock)
}

// Testable inner implementation with an injectable writer.
fn run_impl(shell: SupportedShell, writer: &mut dyn Write) -> io::Result<()> {
    let mut cmd = visible_command();
    let bin_name = cmd.get_name().to_string();
    // Mirrors clap_complete::generate(), but via try_generate() so a write
    // failure (broken pipe, full disk) propagates instead of panicking.
    cmd.set_bin_name(bin_name);
    cmd.build();
    Shell::from(shell).try_generate(&cmd, writer)
}

// clap_complete's generators iterate all subcommands and ignore `hide`, so
// generating from `Cli::command()` directly would advertise the hidden
// `completions` subcommand as tab-completable. clap offers no way to remove
// a subcommand, so rebuild the root with only the visible ones.
// @spec CMD-COMP-003
fn visible_command() -> clap::Command {
    let full = Cli::command();
    let mut cmd = clap::Command::new(full.get_name().to_string());
    if let Some(about) = full.get_about() {
        cmd = cmd.about(about.clone());
    }
    for arg in full.get_arguments() {
        cmd = cmd.arg(arg.clone());
    }
    cmd.subcommands(full.get_subcommands().filter(|s| !s.is_hide_set()).cloned())
}

#[cfg(test)]
mod tests {
    use std::io;

    use clap::CommandFactory as _;

    use super::{SupportedShell, run_impl};
    use crate::Cli;

    // Exercises the production path (`run_impl`) with a captured buffer; the
    // public `run` only adds the stdout lock.
    fn completions_output(shell: SupportedShell) -> String {
        let mut buf = Vec::new();
        run_impl(shell, &mut buf).expect("generating completions to a buffer succeeds");
        String::from_utf8(buf).expect("completion output is utf8")
    }

    fn assert_completions_well_formed(shell: SupportedShell, label: &str) {
        let out = completions_output(shell);
        assert!(!out.is_empty(), "{label} completions should be non-empty");
        assert!(
            out.contains("vat"),
            "{label} completions should reference the binary name"
        );
        assert!(
            !out.contains("completions"),
            "{label} completions should not advertise the hidden completions subcommand"
        );
        assert!(
            out.contains("sync"),
            "{label} completions should advertise visible subcommands"
        );
    }

    // @spec CMD-COMP-001, CMD-COMP-002, CMD-COMP-003
    #[test]
    fn bash_completions_are_well_formed() {
        assert_completions_well_formed(SupportedShell::Bash, "bash");
    }

    // @spec CMD-COMP-001, CMD-COMP-002, CMD-COMP-003
    #[test]
    fn zsh_completions_are_well_formed() {
        assert_completions_well_formed(SupportedShell::Zsh, "zsh");
    }

    // @spec CMD-COMP-001, CMD-COMP-002, CMD-COMP-003
    #[test]
    fn fish_completions_are_well_formed() {
        assert_completions_well_formed(SupportedShell::Fish, "fish");
    }

    // @spec CMD-COMP-003
    #[test]
    fn completions_subcommand_is_absent_from_help_output() {
        let help = Cli::command().render_help().to_string();
        assert!(
            !help.contains("completions"),
            "`vat --help` should not mention the completions subcommand"
        );
    }

    struct FailingWriter;

    impl io::Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    // @spec CMD-COMP-005
    #[test]
    fn write_failure_propagates_as_error_instead_of_panicking() {
        let err = run_impl(SupportedShell::Bash, &mut FailingWriter)
            .expect_err("a failing writer should surface as Err");
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }
}
