// @spec SYNC-NOTES-001, SYNC-NOTES-002, SYNC-NOTES-003, SYNC-NOTES-004, SYNC-NOTES-005
// @spec SYNC-PRE-001, SYNC-PRE-002, SYNC-WRITE-002, SYNC-WRITE-004
// @spec SYNC-ID-004

use std::collections::HashMap;
use std::io;
use std::path::Path;

use thiserror::Error;

use crate::backlog_file::{BacklogFile, ParsedRegion, UnsupportedVersion, check_version};
use crate::{base32, file_io, id_assignment, item_file, project_config, tombstone};

#[derive(Debug, Error)]
pub(crate) enum SyncError {
    // @spec SYNC-PRE-001
    #[error("backlog/backlog.md not found; run `vat init`")]
    NoBacklog,
    // @spec SYNC-PRE-002
    #[error(transparent)]
    Version(#[from] UnsupportedVersion),
    #[error(transparent)]
    Config(#[from] project_config::ConfigError),
    #[error(transparent)]
    IdAssignment(#[from] id_assignment::IdAssignmentError),
    #[error(transparent)]
    Tombstone(#[from] tombstone::TombstoneError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    ItemFile(#[from] item_file::ItemFileError),
}

/// Whether [`run`] wrote `backlog.md` or skipped the write because the
/// serialized output was byte-identical to the input (SYNC-WRITE-002).
///
/// Exposing this lets callers (and tests) observe the skip decision directly,
/// instead of inferring it from filesystem mtime — which has coarse granularity
/// on some filesystems and can update spuriously on others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncOutcome {
    /// `backlog.md` was rewritten.
    Wrote,
    /// The write was skipped; `backlog.md` is untouched.
    Skipped,
}

struct PendingWrite {
    item_path: std::path::PathBuf,
    id: String,
    // SYNC-NOTES-004: stripped once by the caller; single source of truth for
    // stripping semantics (passed to write_new_stripped/append_notes_stripped
    // which skip the internal strip call).
    stripped: String,
}

/// Run the ID-assignment and notes-extraction steps of `vat sync`.
///
/// ID assignment (delegated to [`id_assignment::assign_ids`]):
/// - Every non-empty bullet without an `[id]` marker gets a fresh
///   `<prefix>-<3 base32 chars>` ID inserted at the front of the bullet
///   (SYNC-ID-001..003, 005, 006).
/// - Newly-assigned IDs are appended to `backlog/.used-ids` only after
///   `backlog.md` has been written successfully (SYNC-ID-004).
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
/// (SYNC-WRITE-002), reporting that via the returned [`SyncOutcome`]. Creates
/// `backlog/items/` on demand via `item_file::write_new_stripped` (SYNC-WRITE-004).
// @spec SYNC-NOTES-001, SYNC-NOTES-002, SYNC-NOTES-003, SYNC-NOTES-004, SYNC-NOTES-005
// @spec SYNC-PRE-001, SYNC-PRE-002, SYNC-WRITE-002, SYNC-WRITE-004
// @spec SYNC-ID-004
pub(crate) fn run(backlog_dir: &Path) -> Result<SyncOutcome, SyncError> {
    let backlog_path = backlog_dir.join("backlog.md");
    let items_dir = backlog_dir.join("items");
    let used_ids_path = backlog_dir.join(".used-ids");

    // SYNC-PRE-001: backlog.md must exist.
    if !backlog_path.exists() {
        return Err(SyncError::NoBacklog);
    }

    // LLD step 1: load project config up front; fail loudly when vat.toml is
    // missing or invalid. ID generation needs `project.id` as the prefix.
    let config = project_config::load(&backlog_dir.join("vat.toml"))?;

    let input = file_io::read_to_string(&backlog_path)?;
    let bf = BacklogFile::parse(&input);

    // SYNC-PRE-002: version gate.
    check_version(bf.frontmatter())?;

    let mut region = ParsedRegion::parse(bf.parsed());

    // One ID slot per assignable entry. Empty bullets (`- ` with no body) are
    // skipped per the LLD's edge behavior: no ID is assigned and the line is
    // preserved in place. Existing IDs are lowercased here: tombstones are
    // lowercase-normalized on read, and the prefix comparison in
    // `assign_ids` (SYNC-ID-005) is against the lowercase `project.id`.
    // Slots with existing IDs are never written back to the bullet line, so
    // the normalization does not rewrite the user's casing.
    let mut slots: Vec<Option<String>> = Vec::new();
    let mut slot_entry: Vec<usize> = Vec::new();
    for (i, entry) in region.entries.iter().enumerate() {
        let body = entry.bullet_line.strip_prefix("- ").unwrap_or("");
        if body.trim().is_empty() {
            continue;
        }
        slots.push(extract_id(entry.bullet_line).map(str::to_ascii_lowercase));
        slot_entry.push(i);
    }

    // SYNC-ID-002: collision avoidance against tombstones ∪ IDs already
    // present in the parsed region.
    let mut used = tombstone::read(&used_ids_path)?;
    for id in slots.iter().flatten() {
        used.insert(id.clone());
    }

    let needs_id: Vec<usize> = slots
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| slot.is_none().then_some(i))
        .collect();

    let (new_ids, warnings) = id_assignment::assign_ids(
        &mut slots,
        &mut used,
        config.project_id(),
        &mut rand::thread_rng(),
    )?;
    for warning in &warnings {
        eprintln!("{warning}");
    }

    // Entry index → newly-assigned ID, for splicing the marker into the
    // bullet line on serialize. `assign_ids` fills the `None` slots in entry
    // order, so `needs_id` (the pre-call `None` positions) zips with `new_ids`.
    let inserted: HashMap<usize, &str> = needs_id
        .iter()
        .zip(&new_ids)
        .map(|(&slot_idx, id)| (slot_entry[slot_idx], id.as_str()))
        .collect();

    // Collect all item-file writes before touching any file.  A scan-phase
    // error (e.g. disk full) therefore leaves disk state unchanged: no item
    // file is written and `backlog.md` is not modified.
    //
    // Note: the write phase (step A: item files, step B: backlog.md) is not
    // truly atomic.  A crash between A and B leaves orphaned item-file writes
    // that will be re-processed and double-appended on the next `vat sync` run.
    // Truly atomic cross-file writes require OS support that is out of scope.
    let mut pending: Vec<PendingWrite> = Vec::new();

    for (i, entry) in region.entries.iter_mut().enumerate() {
        let notes: String = entry.notes.to_owned();
        // A bullet that just received an ID extracts its notes to that ID's
        // item file (LLD step 5: assignment happens before extraction).
        let id: Option<String> = inserted
            .get(&i)
            .map(|id| (*id).to_owned())
            .or_else(|| extract_id(entry.bullet_line).map(str::to_owned));

        if !notes.is_empty() {
            let stripped = item_file::strip_notes(&notes);
            if !stripped.is_empty()
                && let Some(id) = id
            {
                pending.push(PendingWrite {
                    item_path: items_dir.join(format!("{id}.md")),
                    id,
                    stripped,
                });
            }
        }

        // SYNC-NOTES-001, SYNC-NOTES-005: always clear notes from this entry.
        entry.notes = "";
    }

    // Serialize, splicing each newly-assigned ID marker in at the front of
    // its bullet (SYNC-ID-001: `[id]` comes before any other markers).
    let mut new_parsed = String::with_capacity(bf.parsed().len() + 8 * inserted.len());
    new_parsed.push_str(region.preamble);
    for (i, entry) in region.entries.iter().enumerate() {
        if let Some(id) = inserted.get(&i) {
            new_parsed.push_str("- [");
            new_parsed.push_str(id);
            new_parsed.push_str("] ");
            new_parsed.push_str(&entry.bullet_line[2..]);
        } else {
            new_parsed.push_str(entry.bullet_line);
        }
        new_parsed.push_str(entry.notes);
    }
    let output = bf.serialize(&new_parsed);

    // SYNC-WRITE-002: skip write when byte-identical.
    if output == input {
        return Ok(SyncOutcome::Skipped);
    }

    // Step A: write item files.
    for pw in pending {
        // Routing by `exists()` is a hint, not a guarantee: a concurrent
        // `vat sync` could create the file between this check and the write.
        // `write_new_stripped` uses `create_new` (atomic), so if we lose that
        // race we fall back to appending — both paths converge on the
        // SYNC-NOTES-003 (append) behaviour.
        if pw.item_path.exists() {
            // SYNC-NOTES-003: append to existing item file.
            item_file::append_notes_stripped(&pw.item_path, &pw.stripped)?;
        } else {
            // SYNC-NOTES-002: create new item file.
            // `write_new_stripped` calls `create_dir_all` → satisfies SYNC-WRITE-004.
            match item_file::write_new_stripped(&pw.item_path, &pw.id, &pw.stripped) {
                Ok(()) => {}
                Err(item_file::ItemFileError::AlreadyExists(_)) => {
                    item_file::append_notes_stripped(&pw.item_path, &pw.stripped)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    // Step B: write backlog.md.
    file_io::write(&backlog_path, &output)?;

    // SYNC-ID-004: append newly-assigned IDs to .used-ids only after the
    // backlog.md write succeeded. An empty batch is a no-op in `append`.
    let new_id_refs: Vec<&str> = new_ids.iter().map(String::as_str).collect();
    tombstone::append(&used_ids_path, &new_id_refs)?;

    Ok(SyncOutcome::Wrote)
}

/// Scan all `[…]` bracket tokens in a bullet line and return the first whose
/// content matches the Crockford base32 `<3-char>-<3-char>` ID format.
///
/// Bullets carry multiple marker tokens (`[in-progress]`, `[blocked-by:…]`,
/// etc.); only a token whose two dash-separated segments are both valid 3-char
/// Crockford base32 strings is treated as an ID marker.  The scan can find a
/// valid ID at any position in the line; if multiple ID-shaped tokens appear
/// (e.g. `[vat-t1h] [vat-g5y] Title`), the first one wins.
fn extract_id(bullet_line: &str) -> Option<&str> {
    let mut s = bullet_line.strip_prefix("- ")?;
    loop {
        let open = s.find('[')?;
        s = &s[open + 1..];
        let close = s.find(']')?;
        let candidate = &s[..close];
        s = &s[close + 1..];
        if let Some((prefix, suffix)) = candidate.split_once('-')
            && base32::validate(prefix, 3).is_ok()
            && base32::validate(suffix, 3).is_ok()
        {
            return Some(candidate);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    use super::{SyncError, SyncOutcome, extract_id, run};

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Tempdir acting as `backlog/`, with a `vat.toml` declaring project id
    /// `vat` (run loads it up front; ID generation needs the prefix).
    fn setup() -> TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("vat.toml"), "[project]\nid = \"vat\"\n")
            .expect("write vat.toml");
        dir
    }

    /// Write `backlog.md` into `dir/`.
    fn write_backlog(dir: &TempDir, content: &str) {
        fs::write(dir.path().join("backlog.md"), content).expect("write backlog.md");
    }

    fn read_backlog(dir: &TempDir) -> String {
        fs::read_to_string(dir.path().join("backlog.md")).expect("read backlog.md")
    }

    // ── extract_id ────────────────────────────────────────────────────────────

    #[test]
    fn extract_id_finds_first_valid_id_marker() {
        assert_eq!(
            extract_id("- [vat-t1h] [agent-ready] Title"),
            Some("vat-t1h")
        );
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
        assert_eq!(extract_id("- [blocked-by:vat-f1w] Title"), None);
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
        write_backlog(&dir, "- [vat-t1h] Title\n  note line\n");
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
        assert!(
            contents.contains("My note."),
            "item file should contain the note"
        );
        assert!(
            contents.starts_with("---\nid: vat-t1h\n---\n"),
            "item file needs frontmatter"
        );
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
        assert!(
            contents.contains("Existing note."),
            "original note preserved"
        );
        assert!(contents.contains("New note."), "new note appended");
        // New content comes after existing content.
        let existing_pos = contents.find("Existing note.").unwrap();
        let new_pos = contents.find("New note.").unwrap();
        assert!(
            new_pos > existing_pos,
            "new note should come after existing note"
        );
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
        assert!(
            !contents.contains("  indented note"),
            "leading spaces stripped"
        );
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

        // `run` reports the skip directly. Asserting on the returned outcome is
        // deterministic, unlike an mtime comparison whose granularity varies by
        // filesystem (e.g. 2-second resolution on FAT32) and can update
        // spuriously on some network mounts.
        let outcome = run(dir.path()).unwrap();
        assert_eq!(
            outcome,
            SyncOutcome::Skipped,
            "byte-identical write skipped"
        );
        assert_eq!(read_backlog(&dir), content);
    }

    // @spec SYNC-WRITE-002
    #[test]
    fn run_reports_wrote_when_backlog_changes() {
        let dir = setup();
        // A bullet with notes — sync clears them, so the output differs and the
        // file is rewritten.
        write_backlog(&dir, "- [vat-t1h] Title\n  A note.\n");
        let outcome = run(dir.path()).unwrap();
        assert_eq!(outcome, SyncOutcome::Wrote, "changed backlog was rewritten");
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
        assert!(
            dir.path().join("items").exists(),
            "items/ dir should be created"
        );
    }

    // ── ID assignment (SYNC-ID-001..006) ─────────────────────────────────────

    /// Extract the assigned `vat-xxx` id from a bullet line like
    /// `- [vat-xxx] Title`.
    fn assigned_id(line: &str) -> &str {
        let start = line.find("[vat-").expect("line has a [vat- id") + 1;
        let end = line[start..].find(']').expect("id marker closed") + start;
        &line[start..end]
    }

    // @spec SYNC-ID-001
    #[test]
    fn run_assigns_id_to_bullet_without_one() {
        let dir = setup();
        write_backlog(&dir, "- A task without an id\n");
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        let line = out.lines().next().unwrap();
        assert!(
            line.starts_with("- [vat-"),
            "id with project prefix inserted at front: {line:?}"
        );
        assert!(
            line.ends_with("] A task without an id"),
            "title preserved after the marker: {line:?}"
        );
        let id = assigned_id(line);
        assert_eq!(id.len(), "vat-".len() + 3, "3-char base32 suffix: {id:?}");
    }

    // @spec SYNC-ID-001
    #[test]
    fn run_assigns_id_before_existing_markers() {
        let dir = setup();
        write_backlog(&dir, "- [in-progress] Claimed but unid'd task\n");
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        let line = out.lines().next().unwrap();
        assert!(
            line.starts_with("- [vat-"),
            "new id goes before other markers: {line:?}"
        );
        assert!(line.contains("[in-progress] Claimed but unid'd task"));
    }

    // @spec SYNC-ID-004
    #[test]
    fn run_appends_newly_assigned_ids_to_used_ids() {
        let dir = setup();
        write_backlog(&dir, "- First new task\n- Second new task\n");
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        let tombstones = fs::read_to_string(dir.path().join(".used-ids")).unwrap();
        for line in out.lines() {
            let id = assigned_id(line);
            assert!(
                tombstones.lines().any(|t| t == id),
                "assigned id {id:?} must be in .used-ids: {tombstones:?}"
            );
        }
        assert_eq!(tombstones.lines().count(), 2, "one line per new id");
    }

    // @spec SYNC-ID-004
    #[test]
    fn run_does_not_touch_used_ids_when_no_ids_were_assigned() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Already id'd\n  a note\n");
        run(dir.path()).unwrap();
        assert!(
            !dir.path().join(".used-ids").exists(),
            "no new ids → .used-ids untouched"
        );
    }

    // @spec SYNC-ID-002
    #[test]
    fn run_does_not_reuse_tombstoned_or_existing_ids() {
        let dir = setup();
        fs::write(dir.path().join(".used-ids"), "vat-aaa\n").unwrap();
        write_backlog(&dir, "- [vat-bbb] Existing\n- Needs an id\n");
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        let new_line = out.lines().nth(1).unwrap();
        let id = assigned_id(new_line);
        assert_ne!(id, "vat-aaa", "tombstoned id must not be reused");
        assert_ne!(id, "vat-bbb", "existing bullet id must not be reused");
    }

    // @spec SYNC-ID-005
    #[test]
    fn run_leaves_foreign_prefix_id_unchanged() {
        let dir = setup();
        let content = "- [bar-7k2] Imported from another project\n";
        write_backlog(&dir, content);
        run(dir.path()).unwrap();
        assert_eq!(read_backlog(&dir), content);
        assert!(!dir.path().join(".used-ids").exists());
    }

    // @spec SYNC-ID-006
    #[test]
    fn run_aborts_on_duplicate_ids_without_writing_anything() {
        let dir = setup();
        let content = "- [vat-abc] First\n  a note\n- [vat-abc] Duplicate\n- New task\n";
        write_backlog(&dir, content);
        let err = run(dir.path()).unwrap_err();
        assert!(
            matches!(err, SyncError::IdAssignment(_)),
            "expected duplicate-id error, got {err}"
        );
        assert_eq!(read_backlog(&dir), content, "backlog.md untouched");
        assert!(!dir.path().join("items").exists(), "no item files written");
        assert!(
            !dir.path().join(".used-ids").exists(),
            "no tombstones appended on error"
        );
    }

    // ── vat.toml precondition (LLD step 1) ────────────────────────────────────

    #[test]
    fn run_errors_when_vat_toml_missing() {
        let dir = tempfile::tempdir().expect("tempdir"); // no vat.toml
        fs::write(dir.path().join("backlog.md"), "- [vat-t1h] Title\n").unwrap();
        let err = run(dir.path()).unwrap_err();
        assert!(
            matches!(err, SyncError::Config(_)),
            "expected config error, got {err}"
        );
    }

    // ── Empty bullet is skipped for ID assignment ─────────────────────────────

    #[test]
    fn run_does_not_assign_id_to_empty_bullet() {
        let dir = setup();
        let content = "- \n";
        write_backlog(&dir, content);
        run(dir.path()).unwrap();
        assert_eq!(read_backlog(&dir), content, "empty bullet preserved as-is");
        assert!(!dir.path().join(".used-ids").exists());
    }

    // ── Bullet without ID, with notes ─────────────────────────────────────────

    // @spec SYNC-ID-001, SYNC-NOTES-002
    #[test]
    fn run_extracts_notes_of_unid_bullet_to_its_new_item_file() {
        let dir = setup();
        write_backlog(&dir, "- No id on this bullet\n  A note.\n");
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        let line = out.lines().next().unwrap();
        let id = assigned_id(line);
        // Notes were cleared and moved to the freshly-assigned id's item file.
        assert_eq!(out, format!("{line}\n"));
        let item_path = dir.path().join("items").join(format!("{id}.md"));
        let contents = fs::read_to_string(&item_path).expect("item file for new id");
        assert!(contents.contains("A note."));
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

    // ── Frontmatter-less backlog is accepted ─────────────────────────────────

    #[test]
    fn run_accepts_backlog_without_frontmatter() {
        // A backlog.md with no YAML frontmatter block is valid: the parsed
        // region is the whole file.  Notes are cleared and item files created
        // just as they would be with frontmatter present.
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Title\n  A note.\n");
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        assert_eq!(out, "- [vat-t1h] Title\n", "notes cleared");
        let item_path = dir.path().join("items").join("vat-t1h.md");
        assert!(item_path.exists(), "item file created");
        assert!(fs::read_to_string(item_path).unwrap().contains("A note."));
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
