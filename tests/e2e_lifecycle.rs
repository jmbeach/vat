//! Black-box end-to-end tests that spawn the compiled `vat` binary in a throwaway
//! temp directory and drive the documented lifecycle — `init` → `sync` → `start`
//! → `done` — asserting on stdout, exit codes, and the final on-disk state of
//! `backlog.md`, `backlog/items/`, and `backlog/.used-ids`.
//!
//! These exercise the real binary (via `CARGO_BIN_EXE_vat`, the same pattern as
//! `tests/completions.rs`) rather than calling library functions, so they pin the
//! CLI contract from the outside and are insulated from internal refactors.
//!
//! Every invocation runs with `XDG_CONFIG_HOME` and `HOME` pointed at temp dirs so
//! user-config resolution is deterministic and never reads or writes the
//! developer's real `~/.config/vat/config.toml`.
//!
//! @spec CMD-INIT-005, CMD-INIT-001, CMD-CFG-001, CMD-CFG-002, CMD-CFG-003
//! @spec SYNC-ID-001, SYNC-NOTES-001, SYNC-NOTES-002, CMD-START-001, CMD-START-002, CMD-START-003
//! @spec CMD-DONE-001, CMD-DONE-002, CMD-DONE-003, CMD-CC-002, CMD-EXIT-001, CMD-EXIT-002

use std::path::PathBuf;
use std::process::Output;

use tempfile::TempDir;

/// A throwaway project sandbox: a working directory the binary `cd`s into, plus
/// isolated `XDG_CONFIG_HOME`/`HOME` so user config is deterministic.
struct World {
    // Held to keep the temp tree alive for the test's duration.
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

    /// Run `vat <args>` with the working directory and config env fixed to this
    /// sandbox. Env vars are set explicitly so an inherited value from the
    /// developer's shell cannot leak in.
    fn vat(&self, args: &[&str]) -> Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_vat"))
            .args(args)
            .current_dir(&self.work)
            .env("XDG_CONFIG_HOME", &self.xdg)
            .env("HOME", &self.home)
            .output()
            .expect("vat binary runs")
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.work.join(rel)
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
    }

    /// Append raw text (e.g. fresh bullets) to `backlog/backlog.md`.
    fn append_backlog(&self, text: &str) {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(self.path("backlog/backlog.md"))
            .expect("open backlog.md for append");
        f.write_all(text.as_bytes()).expect("append to backlog.md");
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Extract the id from the first `- [<id>] ...` bullet in `backlog.md`.
fn first_bullet_id(backlog_md: &str) -> String {
    for line in backlog_md.lines() {
        if let Some(rest) = line.strip_prefix("- [")
            && let Some(end) = rest.find(']')
        {
            return rest[..end].to_string();
        }
    }
    panic!("no `- [id]` bullet found in:\n{backlog_md}");
}

// ---------------------------------------------------------------------------
// The full happy-path lifecycle: init → sync → start → done.
// ---------------------------------------------------------------------------

// @spec CMD-INIT-005, CMD-CFG-003, SYNC-ID-001, SYNC-NOTES-001, SYNC-NOTES-002
// @spec CMD-START-003, CMD-DONE-001, CMD-DONE-002, CMD-DONE-003, CMD-EXIT-001
#[test]
fn full_lifecycle_init_sync_start_done() {
    let w = World::new();

    // `start` needs a configured user.name; set it first (CMD-CFG-003). The
    // config lands under the sandbox's XDG dir, not the developer's home.
    let out = w.vat(&["config", "set", "user.name", "alice"]);
    assert_eq!(out.status.code(), Some(0), "config set: {}", stderr(&out));

    // --- init (CMD-INIT-005) ---
    let out = w.vat(&["init", "abc"]);
    assert_eq!(out.status.code(), Some(0), "init: {}", stderr(&out));
    assert!(
        stdout(&out).contains("initialized backlog/ with prefix abc"),
        "init stdout: {:?}",
        stdout(&out)
    );

    // Initial on-disk state: backlog.md is just the version frontmatter, the
    // tombstone is empty, and vat.toml records the prefix.
    assert_eq!(w.read("backlog/backlog.md"), "---\nversion: 1\n---\n");
    assert_eq!(w.read("backlog/.used-ids"), "");
    assert!(
        w.read("backlog/vat.toml").contains("id = \"abc\""),
        "vat.toml: {:?}",
        w.read("backlog/vat.toml")
    );
    assert!(w.path("backlog/README.md").exists(), "README.md created");

    // --- author two bullets, one carrying notes ---
    w.append_backlog("- First task\n  some notes here\n- Second task\n");

    // --- sync (SYNC-ID-001, SYNC-NOTES-*) ---
    let out = w.vat(&["sync"]);
    assert_eq!(out.status.code(), Some(0), "sync: {}", stderr(&out));

    let after_sync = w.read("backlog/backlog.md");
    // Both bullets now carry an `abc-<3>` id.
    let id_count = after_sync.matches("- [abc-").count();
    assert_eq!(id_count, 2, "both bullets assigned ids:\n{after_sync}");
    // The note line was lifted out of backlog.md...
    assert!(
        !after_sync.contains("some notes here"),
        "notes extracted from backlog.md:\n{after_sync}"
    );

    let first_id = first_bullet_id(&after_sync);
    assert!(
        first_id.starts_with("abc-") && first_id.len() == 7,
        "id shape: {first_id}"
    );

    // ...and into an item file for the first task (SYNC-NOTES-002).
    let item_rel = format!("backlog/items/{first_id}.md");
    assert!(
        w.path(&item_rel).exists(),
        "item file created for {first_id}"
    );
    assert!(
        w.read(&item_rel).contains("some notes here"),
        "notes moved into item file: {:?}",
        w.read(&item_rel)
    );

    // Both ids are tombstoned after assignment (SYNC-ID writes .used-ids).
    let used_after_sync = w.read("backlog/.used-ids");
    assert!(
        used_after_sync.contains(&first_id),
        ".used-ids records assigned id {first_id}: {used_after_sync:?}"
    );
    assert_eq!(
        used_after_sync.lines().count(),
        2,
        "both assigned ids tombstoned: {used_after_sync:?}"
    );

    // --- start (CMD-START-003) ---
    let out = w.vat(&["start", &first_id]);
    assert_eq!(out.status.code(), Some(0), "start: {}", stderr(&out));
    assert_eq!(stdout(&out).trim(), format!("started {first_id}"));

    let after_start = w.read("backlog/backlog.md");
    assert!(
        after_start.contains(&format!("- [{first_id}] [in-progress] [by:alice]")),
        "claim markers in canonical order:\n{after_start}"
    );

    // --- done (CMD-DONE-001, CMD-DONE-002, CMD-DONE-003) ---
    let out = w.vat(&["done", &first_id]);
    assert_eq!(out.status.code(), Some(0), "done: {}", stderr(&out));
    assert_eq!(stdout(&out).trim(), format!("done {first_id}"));

    let after_done = w.read("backlog/backlog.md");
    // The completed bullet is gone; the second task survives.
    assert!(
        !after_done.contains(&first_id),
        "completed bullet removed:\n{after_done}"
    );
    assert!(
        after_done.matches("- [abc-").count() == 1,
        "the other task remains:\n{after_done}"
    );
    // Its item file was deleted (CMD-DONE-002).
    assert!(
        !w.path(&item_rel).exists(),
        "item file deleted on done: {item_rel}"
    );
    // The tombstone is retained — done never un-records an id (CMD-DONE-003).
    let used_after_done = w.read("backlog/.used-ids");
    assert!(
        used_after_done.contains(&first_id),
        "id stays tombstoned after done: {used_after_done:?}"
    );
}

// ---------------------------------------------------------------------------
// Error paths and exit codes, exercised through the real binary.
// ---------------------------------------------------------------------------

// @spec CMD-INIT-001, CMD-EXIT-002
#[test]
fn init_twice_fails_with_exit_1() {
    let w = World::new();

    let first = w.vat(&["init", "abc"]);
    assert_eq!(first.status.code(), Some(0));

    let second = w.vat(&["init", "abc"]);
    assert_eq!(second.status.code(), Some(1), "second init must fail");
    assert!(
        stderr(&second).contains("already"),
        "stderr explains the conflict: {:?}",
        stderr(&second)
    );
}

// @spec CMD-START-001, CMD-EXIT-002
#[test]
fn start_without_user_name_fails_with_exit_1() {
    let w = World::new();
    w.vat(&["init", "abc"]);
    w.append_backlog("- A task\n");
    assert_eq!(w.vat(&["sync"]).status.code(), Some(0));
    let id = first_bullet_id(&w.read("backlog/backlog.md"));

    // No `config set user.name` ran, and HOME/XDG point at empty temp dirs, so
    // user.name is genuinely unset.
    let out = w.vat(&["start", &id]);
    assert_eq!(out.status.code(), Some(1), "start without user.name");
    assert!(
        stderr(&out).contains("user.name"),
        "stderr points at config: {:?}",
        stderr(&out)
    );
    // The bullet was not claimed.
    assert!(
        !w.read("backlog/backlog.md").contains("[in-progress]"),
        "no claim written on failure"
    );
}

// @spec CMD-START-002, CMD-EXIT-002
#[test]
fn start_already_claimed_fails_with_exit_1() {
    let w = World::new();
    w.vat(&["config", "set", "user.name", "alice"]);
    w.vat(&["init", "abc"]);
    w.append_backlog("- A task\n");
    w.vat(&["sync"]);
    let id = first_bullet_id(&w.read("backlog/backlog.md"));

    assert_eq!(w.vat(&["start", &id]).status.code(), Some(0));

    let again = w.vat(&["start", &id]);
    assert_eq!(again.status.code(), Some(1), "double-claim must fail");
    assert!(
        stderr(&again).contains("already claimed"),
        "stderr names the existing claim: {:?}",
        stderr(&again)
    );
}

// @spec CMD-CC-002, CMD-EXIT-002
#[test]
fn done_unknown_id_fails_with_exit_1_and_writes_nothing() {
    let w = World::new();
    w.vat(&["init", "abc"]);
    w.append_backlog("- A task\n");
    w.vat(&["sync"]);
    let before = w.read("backlog/backlog.md");

    let out = w.vat(&["done", "abc-zzz"]);
    assert_eq!(out.status.code(), Some(1), "unknown id must fail");
    assert!(
        stderr(&out).contains("unknown id"),
        "stderr: {:?}",
        stderr(&out)
    );
    // No mutation on the abort path.
    assert_eq!(w.read("backlog/backlog.md"), before, "backlog unchanged");
}

// @spec CMD-CFG-001, CMD-CFG-002, CMD-CFG-003
#[test]
fn config_roundtrips_user_name_and_reads_project_id() {
    let w = World::new();

    // user.name: get before set is empty (exit 0, no output).
    let unset = w.vat(&["config", "get", "user.name"]);
    assert_eq!(unset.status.code(), Some(0));
    assert!(
        stdout(&unset).trim().is_empty(),
        "unset user.name prints nothing: {:?}",
        stdout(&unset)
    );

    assert_eq!(
        w.vat(&["config", "set", "user.name", "bob"]).status.code(),
        Some(0)
    );
    let got = w.vat(&["config", "get", "user.name"]);
    assert_eq!(got.status.code(), Some(0));
    assert_eq!(stdout(&got).trim(), "bob");

    // project.id reads back the init prefix.
    w.vat(&["init", "xyz"]);
    let pid = w.vat(&["config", "get", "project.id"]);
    assert_eq!(pid.status.code(), Some(0));
    assert_eq!(stdout(&pid).trim(), "xyz");
}

// Independent sandboxes don't see each other's config — guards against a test
// accidentally reading the developer's real ~/.config/vat or another test's.
#[test]
fn sandboxes_are_isolated_from_each_other_and_real_home() {
    let a = World::new();
    let b = World::new();
    a.vat(&["config", "set", "user.name", "from-a"]);

    // `b` never set a user.name, so its lookup is empty despite `a` setting one.
    let got = b.vat(&["config", "get", "user.name"]);
    assert_eq!(got.status.code(), Some(0));
    assert!(
        stdout(&got).trim().is_empty(),
        "sandbox b must not see sandbox a's config: {:?}",
        stdout(&got)
    );
}
