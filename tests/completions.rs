//! End-to-end tests for `vat completions <shell>` exit codes and output,
//! exercising the real binary.

use std::process::Command;

fn vat(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_vat"))
        .args(args)
        .output()
        .expect("vat binary runs")
}

// @spec CMD-COMP-001
#[test]
fn valid_shell_writes_script_to_stdout_and_exits_zero() {
    let out = vat(&["completions", "bash"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!out.stdout.is_empty(), "completion script goes to stdout");
}

// @spec CMD-COMP-004
#[test]
fn invalid_shell_exits_with_code_2_and_usage_error() {
    let out = vat(&["completions", "badshell"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "nothing is written to stdout");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid value 'badshell'"),
        "stderr should carry a usage error, got: {stderr}"
    );
}

// @spec CMD-COMP-002
#[test]
fn shells_outside_the_supported_set_are_rejected() {
    for shell in ["elvish", "powershell"] {
        let out = vat(&["completions", shell]);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{shell} is not in the supported set"
        );
    }
}
