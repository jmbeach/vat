//! Golden-fixture snapshot tests for the four bullet-mutating commands
//! (`start`, `block`, `unblock`, `done`), exercising the real `vat` binary
//! (vat-f7v).
//!
//! Each command has one input fixture under `tests/fixtures/commands/<cmd>/`
//! (the contents of a `backlog/` directory). A scenario copies that fixture
//! into a fresh tempdir's `backlog/`, runs the real binary, and makes
//! *structural* assertions on the exit code, the message, and the resulting
//! `backlog.md` — rather than byte-exact whole-tree comparisons, which would
//! break on cosmetic output changes. No-op cases additionally assert the file
//! is byte-for-byte unchanged, which is the behavior under test, not a cosmetic
//! detail.
//!
//! Each scenario is its own `#[test]` so an upstream regression cannot mask a
//! downstream assertion, and every setup step is checked for success.
//!
//! These pin already-implemented CMD-* behavior; they introduce no new
//! requirements, so no EARS markers change.
//!
//! The `make_backlog_dir`/`write_backlog`/`read_backlog`/`HEADER` helpers in
//! `src/test_support.rs` are a `#[cfg(test)]` *in-crate* module; an integration
//! test compiles as a separate crate and can reach only the public API, so they
//! cannot be imported here. The local helpers below mirror that role for the
//! binary-invocation tests, following the precedent set by `tests/sync_golden.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Output;

use tempfile::TempDir;

/// Absolute path to `tests/fixtures/commands`.
fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/commands")
}

/// Recursively copy the tree at `src` into `dst` (creating `dst`).
fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create dst dir");
    for entry in fs::read_dir(src).expect("read_dir src") {
        let entry = entry.expect("dir entry");
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type().expect("file_type").is_dir() {
            copy_tree(&from, &to);
        } else {
            fs::copy(&from, &to).expect("copy file");
        }
    }
}

/// Copy `tests/fixtures/commands/<cmd>` into a fresh tempdir's `backlog/` and
/// return the tempdir. The command runs with this dir as its cwd, so `backlog/`
/// resolves the way it does in real use.
fn prepare(cmd: &str) -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_tree(&fixtures_root().join(cmd), &tmp.path().join("backlog"));
    tmp
}

/// Write an isolated user config carrying `user.name = <name>` under the
/// tempdir's `XDG_CONFIG_HOME`, so `vat start` resolves a claimer without
/// touching the developer's real `~/.config`.
fn set_user_name(tmp: &TempDir, name: &str) {
    let cfg = tmp.path().join("xdg").join("vat").join("config.toml");
    fs::create_dir_all(cfg.parent().expect("config has parent")).expect("create xdg/vat");
    fs::write(&cfg, format!("[user]\nname = \"{name}\"\n")).expect("write user config");
}

/// Run the real `vat` binary in `tmp` with a fully isolated environment.
///
/// `env_clear()` drops every inherited variable first — `Command::env` only
/// adds/overrides, so without the clear a stray `XDG_CONFIG_HOME` (or a future
/// `VAT_*` override) from the test runner's shell could leak into the
/// subprocess. Only `HOME` and `XDG_CONFIG_HOME`, both pointed inside `tmp`, are
/// then provided — the only environment the binary reads.
fn vat(tmp: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vat"))
        .args(args)
        .current_dir(tmp.path())
        .env_clear()
        .env("HOME", tmp.path().join("home"))
        .env("XDG_CONFIG_HOME", tmp.path().join("xdg"))
        .output()
        .expect("vat binary runs")
}

/// Read back `<tmp>/backlog/backlog.md`.
fn read_backlog(tmp: &TempDir) -> String {
    fs::read_to_string(tmp.path().join("backlog").join("backlog.md")).expect("read backlog.md")
}

/// Expected process outcome — a named alternative to an opaque success bool.
#[derive(Clone, Copy, Debug)]
enum Outcome {
    /// User-facing success: exit 0, confirmation on stdout.
    Success,
    /// User-facing refusal: exit 1, diagnostic on stderr (CMD-EXIT-002).
    Refused,
}

/// Assert the process `out` matches `expected` and return `(stdout, stderr)` for
/// further message assertions.
fn assert_outcome(out: &Output, expected: Outcome) -> (String, String) {
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let want = match expected {
        Outcome::Success => 0,
        Outcome::Refused => 1,
    };
    assert_eq!(
        out.status.code(),
        Some(want),
        "expected {expected:?} (exit {want})\nstdout: {stdout}\nstderr: {stderr}"
    );
    (stdout, stderr)
}

// ════════════════════════════════════════════════════════════════════════════
// vat start
// ════════════════════════════════════════════════════════════════════════════

// @spec CMD-START-003, CMD-START-004
#[test]
fn start_claim_succeeds_and_adds_markers() {
    let tmp = prepare("start");
    set_user_name(&tmp, "tester");

    let out = vat(&tmp, &["start", "vat-g5y"]);
    let (stdout, _) = assert_outcome(&out, Outcome::Success);

    assert!(
        stdout.contains("vat-g5y"),
        "confirmation names the id: {stdout}"
    );
    // CMD-START-003: both markers added in canonical position.
    assert!(
        read_backlog(&tmp).contains("- [vat-g5y] [in-progress] [by:tester] A task\n"),
        "claimed bullet, got:\n{}",
        read_backlog(&tmp)
    );
}

// @spec CMD-START-002
#[test]
fn start_already_claimed_is_refused_and_leaves_file_unchanged() {
    let tmp = prepare("start");
    set_user_name(&tmp, "tester");
    let before = read_backlog(&tmp);

    let out = vat(&tmp, &["start", "vat-h8x"]);
    let (_, stderr) = assert_outcome(&out, Outcome::Refused);

    assert!(
        stderr.contains("already claimed") || stderr.contains("already in progress"),
        "refusal names the existing claim: {stderr}"
    );
    assert_eq!(read_backlog(&tmp), before, "a refused claim must not write");
}

// ════════════════════════════════════════════════════════════════════════════
// vat block
// ════════════════════════════════════════════════════════════════════════════

// @spec CMD-BLOCK-005
#[test]
fn block_adds_blocked_by_marker() {
    let tmp = prepare("block");

    let out = vat(&tmp, &["block", "vat-g5y", "vat-f1w"]);
    assert_outcome(&out, Outcome::Success);

    assert!(
        read_backlog(&tmp).contains("- [vat-g5y] [blocked-by:vat-f1w] Target\n"),
        "blocker added in canonical position, got:\n{}",
        read_backlog(&tmp)
    );
}

// @spec CMD-BLOCK-001
#[test]
fn block_self_block_is_refused_and_leaves_file_unchanged() {
    let tmp = prepare("block");
    let before = read_backlog(&tmp);

    let out = vat(&tmp, &["block", "vat-g5y", "vat-g5y"]);
    let (_, stderr) = assert_outcome(&out, Outcome::Refused);

    assert!(stderr.contains("itself"), "self-block diagnostic: {stderr}");
    assert_eq!(read_backlog(&tmp), before, "a refused block must not write");
}

// @spec CMD-BLOCK-002
#[test]
fn block_unknown_blocker_is_refused_and_leaves_file_unchanged() {
    let tmp = prepare("block");
    let before = read_backlog(&tmp);

    let out = vat(&tmp, &["block", "vat-g5y", "vat-zzz"]);
    let (_, stderr) = assert_outcome(&out, Outcome::Refused);

    assert!(
        stderr.contains("unknown blocker: vat-zzz"),
        "unknown-blocker diagnostic: {stderr}"
    );
    assert_eq!(read_backlog(&tmp), before, "a refused block must not write");
}

// @spec CMD-BLOCK-004
#[test]
fn block_replaces_an_existing_different_blocker() {
    let tmp = prepare("block");

    // vat-h8x starts [blocked-by:vat-k2m]; re-blocking by vat-f1w replaces it.
    let out = vat(&tmp, &["block", "vat-h8x", "vat-f1w"]);
    assert_outcome(&out, Outcome::Success);

    let after = read_backlog(&tmp);
    assert!(
        after.contains("- [vat-h8x] [blocked-by:vat-f1w] Dependent\n"),
        "blocker replaced, got:\n{after}"
    );
    assert!(
        !after.contains("[blocked-by:vat-k2m]"),
        "the old blocker must be gone (single blocker per task), got:\n{after}"
    );
}

// @spec CMD-BLOCK-003
#[test]
fn block_same_blocker_is_a_noop_and_leaves_file_unchanged() {
    let tmp = prepare("block");
    let before = read_backlog(&tmp);

    // vat-h8x is already [blocked-by:vat-k2m]; blocking by the same id is a no-op.
    let out = vat(&tmp, &["block", "vat-h8x", "vat-k2m"]);
    assert_outcome(&out, Outcome::Success);

    assert_eq!(
        read_backlog(&tmp),
        before,
        "re-blocking by the same id must not rewrite the file"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// vat unblock
// ════════════════════════════════════════════════════════════════════════════

// @spec CMD-UNBLOCK-002
#[test]
fn unblock_strips_the_blocked_by_marker() {
    let tmp = prepare("unblock");

    let out = vat(&tmp, &["unblock", "vat-g5y"]);
    assert_outcome(&out, Outcome::Success);

    let after = read_backlog(&tmp);
    assert!(
        after.contains("- [vat-g5y] Blocked task\n"),
        "marker stripped, title preserved, got:\n{after}"
    );
    assert!(
        !after.contains("blocked-by"),
        "no blocker marker should remain, got:\n{after}"
    );
}

// @spec CMD-UNBLOCK-001
#[test]
fn unblock_not_blocked_is_a_noop_and_leaves_file_unchanged() {
    let tmp = prepare("unblock");
    let before = read_backlog(&tmp);

    let out = vat(&tmp, &["unblock", "vat-h8x"]);
    assert_outcome(&out, Outcome::Success);

    assert_eq!(
        read_backlog(&tmp),
        before,
        "unblocking an unblocked task must not rewrite the file"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// vat done
// ════════════════════════════════════════════════════════════════════════════

// @spec CMD-DONE-001, CMD-DONE-002, CMD-DONE-003, CMD-DONE-004
#[test]
fn done_removes_entry_deletes_item_tombstones_and_auto_unblocks() {
    let tmp = prepare("done");
    let item = tmp.path().join("backlog").join("items").join("vat-g5y.md");
    assert!(item.exists(), "fixture should ship an item file to delete");

    let out = vat(&tmp, &["done", "vat-g5y"]);
    assert_outcome(&out, Outcome::Success);

    let after = read_backlog(&tmp);
    // CMD-DONE-001: the completed bullet is gone.
    assert!(
        !after.contains("- [vat-g5y]"),
        "completed bullet removed, got:\n{after}"
    );
    // CMD-DONE-004: the dependent is auto-unblocked.
    assert!(
        after.contains("- [vat-h8x] Dependent\n"),
        "dependent auto-unblocked, got:\n{after}"
    );
    assert!(
        !after.contains("blocked-by"),
        "no dangling blocker, got:\n{after}"
    );
    // CMD-DONE-002: the item file is deleted.
    assert!(!item.exists(), "item file should be deleted");
    // CMD-DONE-003: the id is tombstoned (appended, existing entry preserved).
    let used =
        fs::read_to_string(tmp.path().join("backlog").join(".used-ids")).expect("read .used-ids");
    assert!(used.contains("vat-g5y"), "id tombstoned, got:\n{used}");
    assert!(
        used.contains("vat-aaa"),
        "pre-existing tombstone preserved, got:\n{used}"
    );
}

// @spec CMD-CC-002
#[test]
fn done_unknown_id_is_refused_and_touches_nothing() {
    let tmp = prepare("done");
    let before = read_backlog(&tmp);
    let item = tmp.path().join("backlog").join("items").join("vat-g5y.md");

    let out = vat(&tmp, &["done", "vat-zzz"]);
    let (_, stderr) = assert_outcome(&out, Outcome::Refused);

    assert!(
        stderr.contains("unknown id: vat-zzz"),
        "unknown-id diagnostic: {stderr}"
    );
    assert_eq!(
        read_backlog(&tmp),
        before,
        "a refused done must not write the backlog"
    );
    assert!(item.exists(), "a refused done must not delete item files");
}
