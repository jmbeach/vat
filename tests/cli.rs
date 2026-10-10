//! Black-box tests for the CLI shell contract shared by every command:
//! argument parsing, help/version, error rendering, and output streams.
//! Spawns the real binary (`CARGO_BIN_EXE_vat`) in an isolated sandbox.

use std::path::PathBuf;
use std::process::Output;

use tempfile::TempDir;

struct World {
    _tmp: TempDir,
    work: PathBuf,
    xdg: PathBuf,
    home: PathBuf,
}

impl World {
    fn new() -> World {
        let tmp = TempDir::new().expect("create tempdir");
        let work = tmp.path().join("work");
        let xdg = tmp.path().join("xdg");
        let home = tmp.path().join("home");
        for d in [&work, &xdg, &home] {
            std::fs::create_dir_all(d).expect("create sandbox subdir");
        }
        World {
            _tmp: tmp,
            work,
            xdg,
            home,
        }
    }

    fn vat(&self, args: &[&str]) -> Output {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_vat"));
        cmd.args(args)
            .current_dir(&self.work)
            .env_clear()
            .env("XDG_CONFIG_HOME", &self.xdg)
            .env("HOME", &self.home);
        if let Some(path) = std::env::var_os("PATH") {
            cmd.env("PATH", path);
        }
        cmd.output().expect("vat binary runs")
    }

    fn vat_ok(&self, args: &[&str]) -> Output {
        let out = self.vat(args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "`vat {}` failed: {}",
            args.join(" "),
            stderr(&out)
        );
        out
    }

    /// An initialized project with `user.name` set, so ID-taking commands get
    /// as far as ID lookup.
    fn initialized() -> World {
        let w = World::new();
        w.vat_ok(&["init", "abc"]);
        w.vat_ok(&["config", "set", "user.name", "tester"]);
        w
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout is utf-8")
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("stderr is utf-8")
}

/// Assert a failed command rendered exactly `expected` (minus the trailing
/// newline) to stderr, wrote nothing to stdout, and exited with `code`.
fn assert_error(out: &Output, code: i32, expected: &str) {
    assert_eq!(out.status.code(), Some(code), "stderr: {}", stderr(out));
    assert_eq!(stderr(out), format!("{expected}\n"));
    assert_eq!(stdout(out), "", "nothing on stdout on failure");
}

// @spec CLI-ARG-001
#[test]
fn help_lists_exactly_the_public_subcommands() {
    let out = World::new().vat_ok(&["--help"]);
    let help = stdout(&out);
    let commands: Vec<&str> = help
        .lines()
        .skip_while(|l| *l != "Commands:")
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(
        commands,
        [
            "init", "sync", "start", "block", "unblock", "done", "config", "help"
        ]
    );
}

// @spec CLI-ARG-002
#[test]
fn clap_rejections_are_usage_errors_with_exit_2() {
    let w = World::new();
    let cases: &[&[&str]] = &[
        &[],
        &["bogus"],
        &["--bogus"],
        &["start"],
        &["block", "abc-123"],
        &["config"],
        &["config", "get"],
    ];
    for args in cases {
        let out = w.vat(args);
        assert_eq!(out.status.code(), Some(2), "`vat {}`", args.join(" "));
        assert_eq!(stdout(&out), "", "`vat {}` stdout", args.join(" "));
        assert!(
            stderr(&out).contains("Usage:"),
            "`vat {}` stderr: {}",
            args.join(" "),
            stderr(&out)
        );
    }
}

// @spec CLI-ARG-003
#[test]
fn invalid_or_unknown_ids_are_command_errors_not_usage_errors() {
    let w = World::initialized();
    let cases: &[&[&str]] = &[
        &["start", "not-an-id"],
        &["start", "abc-zzz"],
        &["done", "abc-zzz"],
        &["unblock", "abc-zzz"],
        &["block", "abc-zzz", "abc-yyy"],
    ];
    for args in cases {
        let out = w.vat(args);
        assert_eq!(out.status.code(), Some(1), "`vat {}`", args.join(" "));
        let err = stderr(&out);
        assert!(
            err.starts_with("error: "),
            "`vat {}`: {err}",
            args.join(" ")
        );
        assert!(!err.contains("Usage:"), "`vat {}`: {err}", args.join(" "));
    }
}

// @spec CLI-HELP-001
#[test]
fn help_flags_print_to_stdout_and_exit_0() {
    let w = World::new();
    let cases: &[&[&str]] = &[
        &["--help"],
        &["-h"],
        &["start", "--help"],
        &["config", "set", "-h"],
        &["help"],
        &["help", "sync"],
    ];
    for args in cases {
        let out = w.vat_ok(args);
        assert!(
            stdout(&out).contains("Usage:"),
            "`vat {}` stdout: {}",
            args.join(" "),
            stdout(&out)
        );
        assert_eq!(stderr(&out), "", "`vat {}` stderr", args.join(" "));
    }
}

// @spec CLI-VER-001
#[test]
fn version_flags_print_crate_version() {
    let w = World::new();
    for flag in ["--version", "-V"] {
        let out = w.vat_ok(&[flag]);
        assert_eq!(
            stdout(&out),
            format!("vat {}\n", env!("CARGO_PKG_VERSION")),
            "{flag}"
        );
    }
}

// @spec CLI-ERR-001, CLI-ERR-002
#[test]
fn init_failure_renders_with_error_prefix() {
    let w = World::new();
    w.vat_ok(&["init", "abc"]);
    let out = w.vat(&["init", "abc"]);
    assert_error(
        &out,
        1,
        "error: backlog/ already exists; vat is initialized",
    );
}

// @spec CLI-ERR-001, CLI-ERR-002
#[test]
fn sync_failure_renders_with_error_prefix() {
    let out = World::new().vat(&["sync"]);
    assert_error(
        &out,
        1,
        "error: backlog/backlog.md not found; run `vat init`",
    );
}

// @spec CLI-ERR-001, CLI-ERR-002
#[test]
fn config_failure_renders_with_error_prefix() {
    let w = World::initialized();
    let out = w.vat(&["config", "get", "nope"]);
    assert_error(&out, 1, "error: unknown config key: nope");
}

// @spec CLI-ERR-001, CLI-ERR-003
#[test]
fn cause_chain_is_joined_without_repeating_embedded_causes() {
    let w = World::initialized();
    let out = w.vat(&["config", "set", "project.id", "ab"]);
    assert_error(
        &out,
        1,
        "error: invalid project.id: vat.toml [project].id is invalid: \
         must be exactly 3 characters, got 2; run `vat init`",
    );
}

// @spec CLI-ERR-003
#[test]
fn init_invalid_prefix_does_not_repeat_the_cause() {
    let out = World::new().vat(&["init", "ab"]);
    assert_error(
        &out,
        1,
        "error: vat.toml [project].id is invalid: \
         must be exactly 3 characters, got 2; run `vat init`",
    );
}

// @spec CLI-OUT-001
#[test]
fn warnings_go_to_stderr_not_stdout() {
    let w = World::initialized();
    let backlog = w.work.join("backlog/backlog.md");
    let mut text = std::fs::read_to_string(&backlog).expect("read backlog.md");
    text.push_str("\n- [abc-123]\n- Real task\n");
    std::fs::write(&backlog, text).expect("write backlog.md");

    let out = w.vat_ok(&["sync"]);
    assert!(
        stderr(&out).contains("warning: bullet #1 has no title"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains("warning"),
        "stdout: {}",
        stdout(&out)
    );
}

// @spec CLI-OUT-002
#[test]
fn vat_output_contains_no_ansi_escapes() {
    let w = World::new();
    let runs = [
        w.vat(&["sync"]),
        w.vat(&["init", "abc"]),
        w.vat(&["init", "abc"]),
        w.vat(&["config", "set", "user.name", "tester"]),
        w.vat(&["config", "get", "project.id"]),
        w.vat(&["start", "abc-zzz"]),
    ];
    for out in &runs {
        assert!(!out.stdout.contains(&0x1b), "stdout: {}", stdout(out));
        assert!(!out.stderr.contains(&0x1b), "stderr: {}", stderr(out));
    }
}
