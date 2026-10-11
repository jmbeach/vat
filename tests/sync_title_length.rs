//! Black-box tests for `vat sync`'s title-length check (SYNC-LEN-*) and the
//! `[sync].max_title_length` config key (FMT-CFG-005, CMD-CFG-007..009) as
//! seen through the real binary: exit codes, stderr, and on-disk effects.

use std::fs;
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
    /// An initialized project (`vat init abc`) with `user.name` set.
    fn initialized() -> World {
        let tmp = TempDir::new().expect("create tempdir");
        let work = tmp.path().join("work");
        let xdg = tmp.path().join("xdg");
        let home = tmp.path().join("home");
        for d in [&work, &xdg, &home] {
            fs::create_dir_all(d).expect("create sandbox subdir");
        }
        let w = World {
            _tmp: tmp,
            work,
            xdg,
            home,
        };
        w.vat_ok(&["init", "abc"]);
        w.vat_ok(&["config", "set", "user.name", "tester"]);
        w
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

    fn backlog(&self) -> PathBuf {
        self.work.join("backlog")
    }

    fn write(&self, rel: &str, content: &str) {
        fs::write(self.backlog().join(rel), content).expect("write backlog file");
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.backlog().join(rel)).expect("read backlog file")
    }

    /// Replace the body of `backlog.md` after its frontmatter.
    fn write_bullets(&self, bullets: &str) {
        self.write("backlog.md", &format!("---\nversion: 1\n---\n\n{bullets}"));
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout is utf-8")
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("stderr is utf-8")
}

// @spec SYNC-LEN-002, SYNC-LEN-007
#[test]
fn sync_over_limit_exits_1_and_leaves_files_untouched() {
    let w = World::initialized();
    w.write_bullets(&format!("- {}\n  notes here\n", "a".repeat(121)));
    let before = w.read("backlog.md");

    let out = w.vat(&["sync"]);

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "", "nothing on stdout on failure");
    let err = stderr(&out);
    assert!(err.starts_with("error: 1 new bullet has a title"), "{err}");
    assert!(!err.contains("error: error:"), "single error prefix: {err}");
    assert_eq!(w.read("backlog.md"), before);
    assert!(
        fs::read_dir(w.backlog().join("items"))
            .map(|d| d.count() == 0)
            .unwrap_or(true),
        "no item files written"
    );
    assert_eq!(
        fs::read_to_string(w.backlog().join(".used-ids")).unwrap_or_default(),
        "",
        "no ids tombstoned"
    );
}

// @spec SYNC-LEN-008
#[test]
fn sync_abort_suppresses_dropped_blocker_warning_but_keeps_malformed_warning() {
    let w = World::initialized();
    w.write_bullets(&format!(
        "- [abc-t1h]\n- [blocked-by:abc-f1w] [blocked-by:abc-h8x] blocked task\n- {}\n",
        "a".repeat(121)
    ));

    let out = w.vat(&["sync"]);

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("has no title"),
        "malformed warning still prints: {err}"
    );
    assert!(
        !err.contains("drops extra"),
        "no dropped-blocker warning: {err}"
    );
    let warn_at = err.find("has no title").unwrap();
    let error_at = err.find("error:").unwrap();
    assert!(warn_at < error_at, "warnings precede the error: {err}");
}

// @spec SYNC-LEN-006, CMD-CFG-009
#[test]
fn config_set_zero_disables_the_check_end_to_end() {
    let w = World::initialized();
    w.vat_ok(&["config", "set", "sync.max_title_length", "0"]);
    w.write_bullets(&format!("- {}\n", "a".repeat(300)));

    w.vat_ok(&["sync"]);

    assert!(w.read("backlog.md").contains("- [abc-"), "bullet got an id");
}

// @spec CMD-CFG-007
#[test]
fn config_get_prints_effective_limit() {
    let w = World::initialized();
    assert_eq!(
        stdout(&w.vat_ok(&["config", "get", "sync.max_title_length"])),
        "120\n"
    );
    w.vat_ok(&["config", "set", "sync.max_title_length", "80"]);
    assert_eq!(
        stdout(&w.vat_ok(&["config", "get", "sync.max_title_length"])),
        "80\n"
    );
}

// @spec FMT-CFG-005
#[test]
fn invalid_limit_in_vat_toml_fails_every_command_naming_the_key() {
    let w = World::initialized();
    w.write_bullets("- [abc-t1h] a task\n");
    let toml = w.read("vat.toml");
    w.write(
        "vat.toml",
        &format!("{toml}\n[sync]\nmax_title_length = -5\n"),
    );

    for args in [
        &["sync"][..],
        &["start", "abc-t1h"][..],
        &["config", "get", "project.id"][..],
    ] {
        let out = w.vat(args);
        assert_eq!(out.status.code(), Some(1), "`vat {}`", args.join(" "));
        let err = stderr(&out);
        assert!(
            err.contains("[sync].max_title_length"),
            "names the key: {err}"
        );
        assert!(!err.contains("vat init"), "no init pointer: {err}");
    }
}
