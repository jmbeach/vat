// @spec SYNC-NOTES-001, SYNC-NOTES-002, SYNC-NOTES-003, SYNC-NOTES-004, SYNC-NOTES-005
// @spec SYNC-PRE-001, SYNC-PRE-002, SYNC-WRITE-002, SYNC-WRITE-004

use std::io;
use std::path::Path;

use thiserror::Error;

use crate::backlog_file::{BacklogFile, ParsedRegion, UnsupportedVersion, check_version};
use crate::{base32, file_io, item_file};

#[derive(Debug, Error)]
pub(crate) enum SyncError {
    // @spec SYNC-PRE-001
    #[error("backlog/backlog.md not found; run `vat init`")]
    NoBacklog,
    // @spec SYNC-PRE-002
    #[error(transparent)]
    Version(#[from] UnsupportedVersion),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    ItemFile(#[from] item_file::ItemFileError),
}

/// Run the notes-extraction step of `vat sync`.
///
/// For each task entry that has note lines:
/// - Strips indentation (SYNC-NOTES-004) and trims blank edges.
/// - If the stripped result is non-empty and an item file for the entry's ID
///   does not exist, creates it (SYNC-NOTES-002).
/// - If the stripped result is non-empty and an item file already exists,
///   appends to it (SYNC-NOTES-003).
/// - In all cases clears the notes from the entry in `backlog.md`
///   (SYNC-NOTES-001, SYNC-NOTES-005).
///
/// Skips the `backlog.md` write when the output is byte-identical to the input
/// (SYNC-WRITE-002). Creates `backlog/items/` on demand via `item_file::write_new`
/// (SYNC-WRITE-004).
// @spec SYNC-NOTES-001, SYNC-NOTES-002, SYNC-NOTES-003, SYNC-NOTES-004, SYNC-NOTES-005
// @spec SYNC-PRE-001, SYNC-PRE-002, SYNC-WRITE-002, SYNC-WRITE-004
pub(crate) fn run(backlog_dir: &Path) -> Result<(), SyncError> {
    let backlog_path = backlog_dir.join("backlog.md");
    let items_dir = backlog_dir.join("items");

    // SYNC-PRE-001: backlog.md must exist.
    if !backlog_path.exists() {
        return Err(SyncError::NoBacklog);
    }

    let input = file_io::read_to_string(&backlog_path)?;
    let bf = BacklogFile::parse(&input);

    // SYNC-PRE-002: version gate.
    check_version(bf.frontmatter())?;

    let mut region = ParsedRegion::parse(bf.parsed());

    for entry in &mut region.entries {
        // Clone to owned values before any mutation of `entry`.
        let notes: String = entry.notes.to_owned();
        let id: Option<String> = extract_id(entry.bullet_line).map(str::to_owned);

        if !notes.is_empty() {
            // SYNC-NOTES-004: strip common leading whitespace and trim blank edges.
            let stripped = item_file::strip_notes(&notes);

            if !stripped.is_empty() {
                if let Some(ref id) = id {
                    let item_path = items_dir.join(format!("{id}.md"));
                    if item_path.exists() {
                        // SYNC-NOTES-003: append to existing item file.
                        item_file::append_notes(&item_path, &notes)?;
                    } else {
                        // SYNC-NOTES-002: create new item file.
                        // `write_new` calls `create_dir_all` → satisfies SYNC-WRITE-004.
                        item_file::write_new(&item_path, id, &notes)?;
                    }
                }
                // No ID on this bullet: notes are still cleared below (see SYNC-NOTES-001).
            }
        }

        // SYNC-NOTES-001, SYNC-NOTES-005: always clear notes from this entry.
        entry.notes = "";
    }

    let new_parsed = region.serialize();
    let output = bf.serialize(&new_parsed);

    // SYNC-WRITE-002: skip write when byte-identical.
    if output == input {
        return Ok(());
    }

    file_io::write(&backlog_path, &output)?;
    Ok(())
}

/// Scan all `[…]` bracket tokens in a bullet line and return the first whose
/// content matches the Crockford base32 `<3-char>-<3-char>` ID format.
///
/// Bullets carry multiple marker tokens (`[in-progress]`, `[blocked-by:…]`,
/// etc.); only a token whose two dash-separated segments are both valid 3-char
/// Crockford base32 strings is treated as an ID marker.  Returning the *first*
/// match relies on canonical marker ordering (ID appears first), but the scan
/// is order-independent and will find a valid ID wherever it appears in the
/// line, so out-of-order bullets still work.
fn extract_id(bullet_line: &str) -> Option<&str> {
    let mut s = bullet_line.strip_prefix("- ")?;
    loop {
        let open = s.find('[')?;
        s = &s[open + 1..];
        let close = s.find(']')?;
        let candidate = &s[..close];
        s = &s[close + 1..];
        if let Some((prefix, suffix)) = candidate.split_once('-') {
            if base32::validate(prefix, 3).is_ok() && base32::validate(suffix, 3).is_ok() {
                return Some(candidate);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    use super::{SyncError, extract_id, run};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn setup() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    /// Write `backlog.md` into `dir/` and return the path to `dir/`.
    fn write_backlog(dir: &TempDir, content: &str) -> std::path::PathBuf {
        let d = dir.path().to_path_buf();
        fs::write(d.join("backlog.md"), content).expect("write backlog.md");
        d
    }

    fn read_backlog(dir: &TempDir) -> String {
        fs::read_to_string(dir.path().join("backlog.md")).expect("read backlog.md")
    }

    // ── extract_id ────────────────────────────────────────────────────────────

    #[test]
    fn extract_id_finds_first_valid_id_marker() {
        assert_eq!(extract_id("- [vat-t1h] [agent-ready] Title"), Some("vat-t1h"));
    }

    #[test]
    fn extract_id_skips_non_id_markers_and_finds_id() {
        // [in-progress] is not an ID; [vat-t1h] is.
        assert_eq!(
            extract_id("- [in-progress] [vat-t1h] Title"),
            Some("vat-t1h")
        );
    }

    #[test]
    fn extract_id_returns_none_when_no_id_marker() {
        assert_eq!(extract_id("- [agent-ready] Title without id"), None);
    }

    #[test]
    fn extract_id_returns_none_for_bare_title() {
        assert_eq!(extract_id("- Just a plain title"), None);
    }

    #[test]
    fn extract_id_does_not_match_blocked_by_marker() {
        // [blocked-by:vat-f1w] contains a dash but does not split into two 3-char base32 parts.
        assert_eq!(
            extract_id("- [blocked-by:vat-f1w] Title"),
            None
        );
    }

    #[test]
    fn extract_id_does_not_match_in_progress() {
        assert_eq!(extract_id("- [in-progress] Title"), None);
    }

    #[test]
    fn extract_id_returns_none_for_empty_bullet() {
        assert_eq!(extract_id("- "), None);
    }

    // ── SYNC-PRE-001 ─────────────────────────────────────────────────────────

    // @spec SYNC-PRE-001
    #[test]
    fn run_errors_when_backlog_md_missing() {
        let dir = setup();
        let err = run(dir.path()).unwrap_err();
        assert!(
            matches!(err, SyncError::NoBacklog),
            "expected NoBacklog, got {err}"
        );
    }

    // ── SYNC-PRE-002 ─────────────────────────────────────────────────────────

    // @spec SYNC-PRE-002
    #[test]
    fn run_errors_when_backlog_version_too_high() {
        let dir = setup();
        write_backlog(&dir, "---\nversion: 99\n---\n- [vat-t1h] Title\n");
        let err = run(dir.path()).unwrap_err();
        assert!(
            matches!(err, SyncError::Version(_)),
            "expected Version error, got {err}"
        );
    }

    // ── SYNC-NOTES-001 ───────────────────────────────────────────────────────

    // @spec SYNC-NOTES-001
    #[test]
    fn run_removes_note_lines_from_backlog() {
        let dir = setup();
        write_backlog(
            &dir,
            "- [vat-t1h] Title\n  note line\n",
        );
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        assert_eq!(out, "- [vat-t1h] Title\n");
        assert!(!out.contains("note line"));
    }

    // @spec SYNC-NOTES-001
    #[test]
    fn run_removes_notes_from_multiple_entries() {
        let dir = setup();
        write_backlog(
            &dir,
            "- [vat-t1h] First\n  note A\n- [vat-g5y] Second\n  note B\n",
        );
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        assert_eq!(out, "- [vat-t1h] First\n- [vat-g5y] Second\n");
    }

    // ── SYNC-NOTES-002 ───────────────────────────────────────────────────────

    // @spec SYNC-NOTES-002
    #[test]
    fn run_creates_item_file_when_none_exists() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Title\n  My note.\n");
        run(dir.path()).unwrap();
        let item_path = dir.path().join("items").join("vat-t1h.md");
        assert!(item_path.exists(), "item file should have been created");
        let contents = fs::read_to_string(&item_path).unwrap();
        assert!(contents.contains("My note."), "item file should contain the note");
        assert!(contents.starts_with("---\nid: vat-t1h\n---\n"), "item file needs frontmatter");
    }

    // @spec SYNC-NOTES-002
    #[test]
    fn run_creates_separate_item_files_for_separate_entries() {
        let dir = setup();
        write_backlog(
            &dir,
            "- [vat-t1h] First\n  Note A\n- [vat-g5y] Second\n  Note B\n",
        );
        run(dir.path()).unwrap();
        let a = dir.path().join("items").join("vat-t1h.md");
        let b = dir.path().join("items").join("vat-g5y.md");
        assert!(a.exists());
        assert!(b.exists());
        assert!(fs::read_to_string(a).unwrap().contains("Note A"));
        assert!(fs::read_to_string(b).unwrap().contains("Note B"));
    }

    // ── SYNC-NOTES-003 ───────────────────────────────────────────────────────

    // @spec SYNC-NOTES-003
    #[test]
    fn run_appends_to_existing_item_file() {
        let dir = setup();
        let items_dir = dir.path().join("items");
        fs::create_dir_all(&items_dir).unwrap();
        let item_path = items_dir.join("vat-t1h.md");
        fs::write(&item_path, "---\nid: vat-t1h\n---\n\nExisting note.\n").unwrap();

        write_backlog(&dir, "- [vat-t1h] Title\n  New note.\n");
        run(dir.path()).unwrap();

        let contents = fs::read_to_string(&item_path).unwrap();
        assert!(contents.contains("Existing note."), "original note preserved");
        assert!(contents.contains("New note."), "new note appended");
        // New content comes after existing content.
        let existing_pos = contents.find("Existing note.").unwrap();
        let new_pos = contents.find("New note.").unwrap();
        assert!(new_pos > existing_pos, "new note should come after existing note");
    }

    // ── SYNC-NOTES-004 ───────────────────────────────────────────────────────

    // @spec SYNC-NOTES-004
    #[test]
    fn run_strips_common_indentation_before_writing_item_file() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Title\n  indented note\n  another line\n");
        run(dir.path()).unwrap();
        let contents = fs::read_to_string(dir.path().join("items").join("vat-t1h.md")).unwrap();
        // The two leading spaces are the common indent and should be stripped.
        assert!(contents.contains("indented note"), "stripped note in file");
        assert!(!contents.contains("  indented note"), "leading spaces stripped");
    }

    // ── SYNC-NOTES-005 ───────────────────────────────────────────────────────

    // @spec SYNC-NOTES-005
    #[test]
    fn run_clears_whitespace_only_notes_without_creating_item_file() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Title\n   \n\n");
        run(dir.path()).unwrap();
        // No item file created.
        let item_path = dir.path().join("items").join("vat-t1h.md");
        assert!(!item_path.exists(), "no item file for blank notes");
        // Notes cleared from backlog.
        let out = read_backlog(&dir);
        assert!(!out.contains("   "), "whitespace-only lines cleared");
    }

    // @spec SYNC-NOTES-005
    #[test]
    fn run_clears_blank_note_lines_without_creating_item_file() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Title\n\n\n");
        run(dir.path()).unwrap();
        let item_path = dir.path().join("items").join("vat-t1h.md");
        assert!(!item_path.exists());
        let out = read_backlog(&dir);
        assert_eq!(out, "- [vat-t1h] Title\n");
    }

    // ── SYNC-WRITE-002 (skip write when no change) ───────────────────────────

    // @spec SYNC-WRITE-002
    #[test]
    fn run_skips_write_when_output_is_byte_identical() {
        let dir = setup();
        // A bullet with no notes already — the output equals the input.
        let content = "- [vat-t1h] Title\n";
        write_backlog(&dir, content);

        // Record the file's modification time before and after.
        let before = fs::metadata(dir.path().join("backlog.md"))
            .unwrap()
            .modified()
            .unwrap();
        run(dir.path()).unwrap();
        let after = fs::metadata(dir.path().join("backlog.md"))
            .unwrap()
            .modified()
            .unwrap();

        // mtime must not change if sync skipped the write.
        assert_eq!(before, after, "file should not have been touched");
        assert_eq!(read_backlog(&dir), content);
    }

    // ── SYNC-WRITE-004 (create items/ dir) ───────────────────────────────────

    // @spec SYNC-WRITE-004
    #[test]
    fn run_creates_items_dir_when_missing() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Title\n  A note.\n");
        // Ensure items/ does NOT exist before the run.
        assert!(!dir.path().join("items").exists());
        run(dir.path()).unwrap();
        assert!(dir.path().join("items").exists(), "items/ dir should be created");
    }

    // ── Bullet without ID ─────────────────────────────────────────────────────

    #[test]
    fn run_clears_notes_for_bullet_without_id_and_does_not_create_item_file() {
        let dir = setup();
        write_backlog(&dir, "- No id on this bullet\n  A note.\n");
        run(dir.path()).unwrap();
        // No item file created (no ID to name it with).
        assert!(!dir.path().join("items").exists());
        // Notes still cleared.
        let out = read_backlog(&dir);
        assert_eq!(out, "- No id on this bullet\n");
    }

    // ── Idempotence ───────────────────────────────────────────────────────────

    // @spec SYNC-WRITE-001, SYNC-WRITE-002
    #[test]
    fn run_is_idempotent() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Title\n  A note.\n");
        run(dir.path()).unwrap();
        let after_first = read_backlog(&dir);
        run(dir.path()).unwrap();
        let after_second = read_backlog(&dir);
        assert_eq!(after_first, after_second, "second run must be a no-op");
    }

    // ── Preamble and freeform are preserved ───────────────────────────────────

    #[test]
    fn run_preserves_preamble_and_freeform_region() {
        let dir = setup();
        let content = concat!(
            "---\nversion: 1\n---\n",
            "# Heading\n\n",
            "- [vat-t1h] Title\n  note\n",
            "---\n",
            "Freeform text here.\n",
        );
        write_backlog(&dir, content);
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        assert!(out.contains("# Heading\n"), "preamble preserved");
        assert!(out.contains("Freeform text here."), "freeform preserved");
        assert!(!out.contains("  note"), "notes cleared");
    }

    // ── Entry with no notes is a pass-through ─────────────────────────────────

    #[test]
    fn run_leaves_notes_free_entry_unchanged() {
        let dir = setup();
        let content = "- [vat-t1h] Title with no notes\n";
        write_backlog(&dir, content);
        run(dir.path()).unwrap();
        assert_eq!(read_backlog(&dir), content);
    }
}
