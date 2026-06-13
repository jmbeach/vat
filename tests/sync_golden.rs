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
    String::from_utf8(out).expect("fixture content is ascii")
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
fn run_sync(tmp: &TempDir) {
    let out = Command::new(env!("CARGO_BIN_EXE_vat"))
        .arg("sync")
        .current_dir(tmp.path())
        .output()
        .expect("vat binary runs");
    assert_eq!(
        out.status.code(),
        Some(0),
        "vat sync should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Assert the post-sync `backlog/` tree equals `<fixture>/expected`, optionally
/// masking assigned ids in both sides first.
fn assert_matches_expected(tmp: &TempDir, fixture: &str, mask: bool) {
    let actual = snapshot(&tmp.path().join("backlog"));
    let expected = snapshot(&fixtures_root().join(fixture).join("expected"));
    let normalize = |m: BTreeMap<String, String>| -> BTreeMap<String, String> {
        if mask {
            m.into_iter().map(|(k, v)| (k, mask_ids(&v))).collect()
        } else {
            m
        }
    };
    assert_eq!(
        normalize(actual),
        normalize(expected),
        "fixture `{fixture}`: post-sync tree did not match expected"
    );
}

/// Copy the fixture in, run sync once, and assert the result matches.
fn check(fixture: &str, mask: bool) {
    let tmp = prepare(fixture);
    run_sync(&tmp);
    assert_matches_expected(&tmp, fixture, mask);
}

// ── brand-new bullets get IDs assigned ───────────────────────────────────────

/// IDs are random (SYNC-ID-001), so this fixture masks them; the test still
/// pins that an id lands in canonical front position and is tombstoned in
/// `.used-ids` (SYNC-ID-004).
// @spec SYNC-ID-001, SYNC-ID-004
#[test]
fn golden_new_bullets_get_ids_assigned() {
    check("new-bullets", true);
}

// ── idempotent re-run: the second sync is a no-op ────────────────────────────

// @spec SYNC-WRITE-001, SYNC-WRITE-002
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
    assert_matches_expected(&tmp, "idempotent", false);
}

// ── notes append to an existing item file ────────────────────────────────────

// @spec SYNC-NOTES-001, SYNC-NOTES-003
#[test]
fn golden_notes_append_to_existing_item_file() {
    check("notes-append", false);
}

// ── whitespace-only notes are dropped, no item file created ──────────────────

// @spec SYNC-NOTES-005
#[test]
fn golden_whitespace_only_notes_dropped_and_no_item_file() {
    let tmp = prepare("whitespace-notes");
    run_sync(&tmp);
    assert!(
        !tmp.path().join("backlog/items").exists(),
        "whitespace-only notes must not create an items/ dir or file"
    );
    assert_matches_expected(&tmp, "whitespace-notes", false);
}

// ── dangling blocked-by marker is left alone ─────────────────────────────────

// @spec SYNC-MARK-003
#[test]
fn golden_dangling_blocked_by_left_alone() {
    check("dangling-blocked-by", false);
}

// ── frontmatter is preserved verbatim while notes are extracted ──────────────

/// Multi-key frontmatter round-trips byte-for-byte; the note moves to a new
/// item file carrying `id:` frontmatter (SYNC-NOTES-002).
// @spec SYNC-NOTES-002
#[test]
fn golden_frontmatter_preserved() {
    check("frontmatter-preserved", false);
}
