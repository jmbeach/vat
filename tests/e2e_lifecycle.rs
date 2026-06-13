//! Black-box end-to-end tests that spawn the compiled `vat` binary in a throwaway
//! temp directory and exercise the documented lifecycle — `init` → `sync` →
//! `start` → `done` — asserting on stdout, exit codes, and the on-disk state of
//! `backlog.md`, `backlog/items/`, and `backlog/.used-ids`.
//!
//! These exercise the real binary (via `CARGO_BIN_EXE_vat`, the same pattern as
//! `tests/completions.rs`) rather than calling library functions, so they pin the
//! CLI contract from the outside and are insulated from internal refactors. They
//! cannot reuse `src/test_support.rs` — that module is `pub(crate)`, reachable
//! only from in-crate `#[cfg(test)]` unit tests, and its helpers hand-write
//! backlog files rather than letting the binary create them via `vat init`.
//!
//! Each independent behavior is its own `#[test]` so an upstream failure (say, a
//! `sync` regression) can't mask whether `start`/`done` still work.
//!
//! @spec CMD-INIT-005, CMD-INIT-001, CMD-CFG-001, CMD-CFG-002, CMD-CFG-003
//! @spec SYNC-ID-001, SYNC-NOTES-001, SYNC-NOTES-002, CMD-START-001, CMD-START-002, CMD-START-003
//! @spec CMD-DONE-001, CMD-DONE-002, CMD-DONE-003, CMD-CC-002, CMD-EXIT-001, CMD-EXIT-002

use std::path::PathBuf;
use std::process::Output;

use tempfile::TempDir;

/// Length of a synced id: `<3-char prefix>-<3-char suffix>`. The suffix length
/// mirrors `ID_SEGMENT_LEN` in `src/tombstone.rs` (which is `pub(crate)` and so
/// cannot be imported into this integration-test crate). Derived here rather
/// than hardcoded so a future suffix-length change has one obvious place to
/// update the test.
const ID_SUFFIX_LEN: usize = 3;
const SYNCED_ID_LEN: usize = "abc-".len() + ID_SUFFIX_LEN;

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

    /// Run `vat <args>` in this sandbox.
    ///
    /// The child starts from a **cleared** environment (`env_clear`) plus only
    /// `XDG_CONFIG_HOME`, `HOME`, and `PATH`. Clearing first is what actually
    /// guarantees isolation: a bare `.env()` only adds/overrides individual
    /// vars, so any inherited var (a real `HOME`, a stray `VAT_*` override, etc.)
    /// would still reach the binary. `PATH` is re-added so the loader behaves
    /// normally; the binary itself is launched by absolute path.
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

    /// Run `vat <args>` as a setup step and assert it succeeded (exit 0). Returns
    /// the output so callers can also inspect stdout. Guarding setup this way
    /// means a setup regression fails *here*, with the offending command named,
    /// rather than surfacing as a confusing panic in a later assertion.
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
// Lifecycle, one behavior per test so a failure in one phase can't hide the
// others. Each test re-runs only the setup it needs, with every setup step
// guarded via `vat_ok`.
// ---------------------------------------------------------------------------

// @spec CMD-INIT-005, CMD-EXIT-001
#[test]
fn init_creates_expected_backlog_structure() {
    let w = World::new();

    let out = w.vat_ok(&["init", "abc"]);
    assert!(
        stdout(&out).contains("initialized backlog/ with prefix abc"),
        "init stdout: {:?}",
        stdout(&out)
    );

    // Structural invariants rather than byte-exact content: a fresh backlog.md
    // declares version 1 and carries no task bullets yet.
    let backlog = w.read("backlog/backlog.md");
    assert!(
        backlog.contains("version: 1"),
        "frontmatter declares version 1: {backlog:?}"
    );
    assert!(
        !backlog.contains("- ["),
        "no id-bearing bullets after init: {backlog:?}"
    );

    // The tombstone starts empty and vat.toml records the prefix.
    assert!(
        w.read("backlog/.used-ids").trim().is_empty(),
        ".used-ids starts empty"
    );
    assert!(
        w.read("backlog/vat.toml").contains("id = \"abc\""),
        "vat.toml: {:?}",
        w.read("backlog/vat.toml")
    );
    assert!(w.path("backlog/README.md").exists(), "README.md created");
}

// @spec SYNC-ID-001, SYNC-NOTES-001, SYNC-NOTES-002, CMD-EXIT-001
#[test]
fn sync_assigns_ids_and_extracts_notes() {
    let w = World::new();
    w.vat_ok(&["init", "abc"]);
    w.append_backlog("- First task\n  some notes here\n- Second task\n");

    w.vat_ok(&["sync"]);

    let after_sync = w.read("backlog/backlog.md");
    // Both bullets now carry an `abc-<3>` id (SYNC-ID-001).
    assert_eq!(
        after_sync.matches("- [abc-").count(),
        2,
        "both bullets assigned ids:\n{after_sync}"
    );
    // The note line was lifted out of backlog.md (SYNC-NOTES-001)...
    assert!(
        !after_sync.contains("some notes here"),
        "notes extracted from backlog.md:\n{after_sync}"
    );

    let first_id = first_bullet_id(&after_sync);
    assert!(
        first_id.starts_with("abc-") && first_id.len() == SYNCED_ID_LEN,
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

    // Both ids are tombstoned after assignment.
    let used = w.read("backlog/.used-ids");
    assert!(
        used.contains(&first_id),
        ".used-ids records assigned id {first_id}: {used:?}"
    );
    assert_eq!(
        used.lines().count(),
        2,
        "both assigned ids tombstoned: {used:?}"
    );
}

// @spec CMD-CFG-003, CMD-START-003, CMD-START-004, CMD-EXIT-001
#[test]
fn start_claims_task_with_canonical_markers() {
    let w = World::new();
    // `start` needs a configured user.name; set it first (CMD-CFG-003). The
    // config lands under the sandbox's XDG dir, not the developer's home.
    w.vat_ok(&["config", "set", "user.name", "alice"]);
    w.vat_ok(&["init", "abc"]);
    w.append_backlog("- A task\n");
    w.vat_ok(&["sync"]);
    let id = first_bullet_id(&w.read("backlog/backlog.md"));

    let out = w.vat_ok(&["start", &id]);
    assert_eq!(stdout(&out).trim(), format!("started {id}"));

    let after_start = w.read("backlog/backlog.md");
    assert!(
        after_start.contains(&format!("- [{id}] [in-progress] [by:alice]")),
        "claim markers in canonical order:\n{after_start}"
    );
}

// @spec CMD-DONE-001, CMD-DONE-002, CMD-DONE-003, CMD-EXIT-001
#[test]
fn done_removes_bullet_deletes_item_and_keeps_tombstone() {
    let w = World::new();
    w.vat_ok(&["init", "abc"]);
    w.append_backlog("- First task\n  some notes here\n- Second task\n");
    w.vat_ok(&["sync"]);
    let first_id = first_bullet_id(&w.read("backlog/backlog.md"));
    let item_rel = format!("backlog/items/{first_id}.md");
    assert!(w.path(&item_rel).exists(), "precondition: item file exists");

    let out = w.vat_ok(&["done", &first_id]);
    assert_eq!(stdout(&out).trim(), format!("done {first_id}"));

    let after_done = w.read("backlog/backlog.md");
    // The completed bullet line is gone (CMD-DONE-001). Match the bullet line
    // specifically rather than the bare id, which could legitimately appear in
    // another bullet's title text.
    assert!(
        !after_done.contains(&format!("- [{first_id}]")),
        "completed bullet removed:\n{after_done}"
    );
    assert_eq!(
        after_done.matches("- [abc-").count(),
        1,
        "the other task remains:\n{after_done}"
    );
    // Its item file was deleted (CMD-DONE-002).
    assert!(
        !w.path(&item_rel).exists(),
        "item file deleted on done: {item_rel}"
    );
    // The tombstone is retained — done never un-records an id (CMD-DONE-003).
    let used = w.read("backlog/.used-ids");
    assert!(
        used.contains(&first_id),
        "id stays tombstoned after done: {used:?}"
    );
}

// ---------------------------------------------------------------------------
// Error paths and exit codes, exercised through the real binary.
// ---------------------------------------------------------------------------

// @spec CMD-INIT-001, CMD-EXIT-002
#[test]
fn init_twice_fails_with_exit_1() {
    let w = World::new();

    w.vat_ok(&["init", "abc"]);

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
    w.vat_ok(&["init", "abc"]);
    w.append_backlog("- A task\n");
    w.vat_ok(&["sync"]);
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
    w.vat_ok(&["config", "set", "user.name", "alice"]);
    w.vat_ok(&["init", "abc"]);
    w.append_backlog("- A task\n");
    w.vat_ok(&["sync"]);
    let id = first_bullet_id(&w.read("backlog/backlog.md"));

    w.vat_ok(&["start", &id]);

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
    w.vat_ok(&["init", "abc"]);
    w.append_backlog("- A task\n");
    w.vat_ok(&["sync"]);
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
    let unset = w.vat_ok(&["config", "get", "user.name"]);
    assert!(
        stdout(&unset).trim().is_empty(),
        "unset user.name prints nothing: {:?}",
        stdout(&unset)
    );

    w.vat_ok(&["config", "set", "user.name", "bob"]);
    let got = w.vat_ok(&["config", "get", "user.name"]);
    assert_eq!(stdout(&got).trim(), "bob");

    // project.id reads back the init prefix.
    w.vat_ok(&["init", "xyz"]);
    let pid = w.vat_ok(&["config", "get", "project.id"]);
    assert_eq!(stdout(&pid).trim(), "xyz");
}

// Independent sandboxes don't see each other's config — guards against a test
// accidentally reading the developer's real ~/.config/vat or another test's.
#[test]
fn sandboxes_are_isolated_from_each_other_and_real_home() {
    let a = World::new();
    let b = World::new();
    a.vat_ok(&["config", "set", "user.name", "from-a"]);

    // `b` never set a user.name, so its lookup is empty despite `a` setting one.
    let got = b.vat_ok(&["config", "get", "user.name"]);
    assert!(
        stdout(&got).trim().is_empty(),
        "sandbox b must not see sandbox a's config: {:?}",
        stdout(&got)
    );
}
