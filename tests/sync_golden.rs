//! Golden-file tests for `vat sync` (vat-d3t).
//!
//! Each fixture under `tests/fixtures/sync/<case>/` has an `input/` tree — the
//! contents of a `backlog/` directory — and an `expected/` tree describing that
//! directory after `vat sync`. A test copies `input/` into a tempdir's
//! `backlog/`, runs the real `vat` binary, and asserts the resulting tree
//! matches `expected/` byte-for-byte.
//!
//! These pin already-implemented SYNC-* behavior; they introduce no new
//! requirements, so no EARS markers change. Helpers are kept local to this file
//! (rather than shared with the per-command inline test modules) so the
//! in-flight shared-test-helper consolidation in vat-m2k has nothing to collide
//! with here.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// Absolute path to `tests/fixtures/sync`.
fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sync")
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

/// Snapshot a directory tree as a map of forward-slash relative path → file
/// contents. Every fixture file is UTF-8 text.
fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(dir).expect("read_dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if entry.file_type().expect("file_type").is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .expect("path under root")
                    .to_string_lossy()
                    .replace('\\', "/");
                let content = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                out.insert(rel, content);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Replace every `vat-<3 base32-ish chars>` id with a stable placeholder so the
/// new-bullets fixture — whose ids are randomly generated (SYNC-ID-001) — can be
/// compared deterministically. Applied only to fixtures whose ids are freshly
/// minted; fixtures with pre-existing ids compare exactly and leave this off.
fn mask_ids(s: &str) -> String {
    let b = s.as_bytes();
    let alnum = |c: u8| c.is_ascii_alphanumeric();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        // Match `vat-` + exactly three alphanumeric bytes, not a prefix of a
        // longer token (the trailing-boundary check rules out `vat-abcd`).
        if b[i..].starts_with(b"vat-")
            && i + 7 <= b.len()
            && alnum(b[i + 4])
            && alnum(b[i + 5])
            && alnum(b[i + 6])
            && (i + 7 == b.len() || !alnum(b[i + 7]))
        {
            out.extend_from_slice(b"vat-@@@");
            i += 7;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    // The loop only ever swaps one ASCII run (`vat-xxx`) for another, so valid
    // UTF-8 in → valid UTF-8 out; the constraint is UTF-8, not ASCII (task
    // titles may carry accented characters).
    String::from_utf8(out).expect("utf-8 fixture content stays utf-8 after id masking")
}

/// Copy `<fixture>/input` into a fresh tempdir's `backlog/` and return the dir.
fn prepare(fixture: &str) -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_tree(
        &fixtures_root().join(fixture).join("input"),
        &tmp.path().join("backlog"),
    );
    tmp
}

/// Run the real `vat sync` with `tmp` as the working directory; assert it
/// exits 0 (`cmd_sync` resolves `backlog/` relative to the cwd).
///
/// `HOME` and `XDG_CONFIG_HOME` are pointed inside the tempdir (the same
/// isolation PR #56 established for e2e invocations) so user-config resolution
/// is deterministic and never reads or writes the developer's real
/// `~/.config/vat/`. `vat sync` does not read user config today, but the
/// isolation keeps these tests environment-independent if it ever starts to.
fn run_sync(tmp: &TempDir) {
    let out = Command::new(env!("CARGO_BIN_EXE_vat"))
        .arg("sync")
        .current_dir(tmp.path())
        .env("HOME", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .expect("vat binary runs");
    assert_eq!(
        out.status.code(),
        Some(0),
        "vat sync should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Whether a fixture's ids are freshly minted by sync — random, so they must be
/// masked before the byte-comparison (SYNC-ID-001) — or already present in the
/// input, in which case the tree is compared exactly.
#[derive(Clone, Copy)]
enum Ids {
    Fresh,
    PreExisting,
}

/// Assert the post-sync `backlog/` tree equals `<fixture>/expected`. For
/// [`Ids::Fresh`] fixtures, assigned ids are masked on both sides first.
fn assert_matches_expected(tmp: &TempDir, fixture: &str, ids: Ids) {
    let actual = snapshot(&tmp.path().join("backlog"));
    let expected = snapshot(&fixtures_root().join(fixture).join("expected"));
    let normalize = |m: BTreeMap<String, String>| -> BTreeMap<String, String> {
        match ids {
            Ids::Fresh => m.into_iter().map(|(k, v)| (k, mask_ids(&v))).collect(),
            Ids::PreExisting => m,
        }
    };
    assert_eq!(
        normalize(actual),
        normalize(expected),
        "fixture `{fixture}`: post-sync tree did not match expected"
    );
}

/// Copy the fixture in, run sync once, and assert the result matches.
fn check(fixture: &str, ids: Ids) {
    let tmp = prepare(fixture);
    run_sync(&tmp);
    assert_matches_expected(&tmp, fixture, ids);
}

// ── brand-new bullets get IDs assigned ───────────────────────────────────────

/// IDs are random (SYNC-ID-001), so this fixture masks them; the test still
/// pins that an id lands in canonical front position and is tombstoned in
/// `.used-ids` (SYNC-ID-004).
// @spec SYNC-ID-001, SYNC-ID-004
#[test]
fn golden_new_bullets_get_ids_assigned() {
    check("new-bullets", Ids::Fresh);
}

// ── idempotent re-run: the second sync is a no-op ────────────────────────────

// @spec SYNC-WRITE-001, SYNC-WRITE-002, SYNC-PTR-003
#[test]
fn golden_second_sync_is_a_noop() {
    let tmp = prepare("idempotent");
    run_sync(&tmp);
    let after_first = snapshot(&tmp.path().join("backlog"));
    run_sync(&tmp);
    let after_second = snapshot(&tmp.path().join("backlog"));
    assert_eq!(
        after_first, after_second,
        "the second sync must change nothing"
    );
    assert_matches_expected(&tmp, "idempotent", Ids::PreExisting);
}

// ── notes append to an existing item file ────────────────────────────────────

// @spec SYNC-NOTES-001, SYNC-NOTES-003, SYNC-PTR-001
#[test]
fn golden_notes_append_to_existing_item_file() {
    check("notes-append", Ids::PreExisting);
}

// ── whitespace-only notes are dropped, no item file created ──────────────────

// @spec SYNC-NOTES-005
#[test]
fn golden_whitespace_only_notes_dropped_and_no_item_file() {
    let tmp = prepare("whitespace-notes");
    // The whitespace-only note line is appended HERE rather than stored in the
    // fixture: a line of literal trailing spaces is exactly what editors and
    // `core.whitespace` settings silently strip, which would turn it into a
    // blank line (a bullet-block terminator) and quietly stop testing
    // SYNC-NOTES-005. Synthesizing it at runtime makes the test robust.
    let backlog_md = tmp.path().join("backlog/backlog.md");
    let mut content = fs::read_to_string(&backlog_md).expect("read fixture backlog.md");
    assert!(
        content.ends_with("- [vat-t1h] Title\n"),
        "fixture should end with the bare bullet; got: {content:?}"
    );
    content.push_str("   \n"); // three-space, whitespace-only note line
    fs::write(&backlog_md, &content).expect("write whitespace note");

    run_sync(&tmp);
    assert!(
        !tmp.path().join("backlog/items").exists(),
        "whitespace-only notes must not create an items/ dir or file"
    );
    assert_matches_expected(&tmp, "whitespace-notes", Ids::PreExisting);
}

// ── dangling blocked-by marker is left alone ─────────────────────────────────

// @spec SYNC-MARK-003
#[test]
fn golden_dangling_blocked_by_left_alone() {
    check("dangling-blocked-by", Ids::PreExisting);
}

// ── pointer suffix is appended when an item file exists ──────────────────────

/// A bullet whose id already has an `items/<id>.md` file (and no notes this
/// run) gains the canonical ` (see ./items/<id>.md)` pointer suffix; the item
/// file is left untouched (SYNC-PTR-001).
// @spec SYNC-PTR-001
#[test]
fn golden_pointer_suffix_appended_when_item_file_exists() {
    check("pointer-suffix", Ids::PreExisting);
}

/// Re-running sync on a bullet that already carries the canonical suffix is a
/// no-op: the suffix is not doubled (SYNC-PTR-003).
// @spec SYNC-PTR-003
#[test]
fn golden_pointer_suffix_is_idempotent() {
    let tmp = prepare("pointer-suffix");
    run_sync(&tmp);
    let after_first = snapshot(&tmp.path().join("backlog"));
    run_sync(&tmp);
    let after_second = snapshot(&tmp.path().join("backlog"));
    assert_eq!(
        after_first, after_second,
        "second sync must not re-append the suffix"
    );
    assert_matches_expected(&tmp, "pointer-suffix", Ids::PreExisting);
}

// ── frontmatter is preserved verbatim while notes are extracted ──────────────

/// Multi-key frontmatter round-trips byte-for-byte; the note moves to a new
/// item file carrying `id:` frontmatter (SYNC-NOTES-002).
// @spec SYNC-NOTES-002, SYNC-PTR-001
#[test]
fn golden_frontmatter_preserved() {
    check("frontmatter-preserved", Ids::PreExisting);
}
