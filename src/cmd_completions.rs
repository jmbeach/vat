use std::io;

use clap::CommandFactory;
use clap_complete::{generate, Shell};

// @spec CMD-COMP-001, CMD-COMP-002
pub fn run<C: CommandFactory>(shell: Shell) {
    let mut cmd = C::command();
    let bin_name = cmd.get_name().to_string();
    generate(shell, &mut cmd, bin_name, &mut io::stdout());
}

#[cfg(test)]
mod tests {
    use clap_complete::Shell;

    use crate::Cli;

    fn completions_output(shell: Shell) -> String {
        use clap::CommandFactory as _;
        use clap_complete::generate;

        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();
        let mut buf = Vec::new();
        generate(shell, &mut cmd, bin_name, &mut buf);
        String::from_utf8(buf).expect("completion output is utf8")
    }

    // @spec CMD-COMP-001, CMD-COMP-002
    #[test]
    fn bash_completions_are_nonempty_and_mention_vat() {
        let out = completions_output(Shell::Bash);
        assert!(!out.is_empty(), "bash completions should be non-empty");
        assert!(out.contains("vat"), "bash completions should reference the binary name");
    }

    // @spec CMD-COMP-001, CMD-COMP-002
    #[test]
    fn zsh_completions_are_nonempty_and_mention_vat() {
        let out = completions_output(Shell::Zsh);
        assert!(!out.is_empty(), "zsh completions should be non-empty");
        assert!(out.contains("vat"), "zsh completions should reference the binary name");
    }

    // @spec CMD-COMP-001, CMD-COMP-002
    #[test]
    fn fish_completions_are_nonempty_and_mention_vat() {
        let out = completions_output(Shell::Fish);
        assert!(!out.is_empty(), "fish completions should be non-empty");
        assert!(out.contains("vat"), "fish completions should reference the binary name");
    }
}
