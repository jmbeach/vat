// @spec SYNC-NOTES-001, SYNC-NOTES-002, SYNC-NOTES-003, SYNC-NOTES-004, SYNC-NOTES-005
// @spec SYNC-PTR-001, SYNC-PTR-002, SYNC-PTR-003
// @spec SYNC-PRE-001, SYNC-PRE-002
// @spec SYNC-WRITE-001, SYNC-WRITE-002, SYNC-WRITE-003, SYNC-WRITE-004
// @spec SYNC-ID-004
// @spec SYNC-MARK-001, SYNC-MARK-002, SYNC-MARK-003, SYNC-MARK-004
// @spec FMT-PARSE-006

use std::io;
use std::path::Path;

use thiserror::Error;

use crate::backlog_file::{BacklogFile, ParsedRegion, UnsupportedVersion, check_version};
use crate::bullet::{Bullet, BulletError};
use crate::{file_io, id_assignment, item_file, project_config, tombstone};

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

/// Run the marker-normalization, ID-assignment, and notes-extraction steps of
/// `vat sync`.
///
/// Every bullet is parsed with [`Bullet::parse`] (the shared front-loaded
/// marker tokenizer) and re-emitted via [`Bullet::serialize`], which puts
/// markers in canonical order with single-space separators (SYNC-MARK-001),
/// reorders/respaces without changing marker values — ID lowercasing is
/// canonicalization (SYNC-MARK-002) — and preserves dangling
/// `[blocked-by:...]` markers (SYNC-MARK-003). When a bullet carries more than
/// one `[blocked-by:...]`, only the first survives re-serialization
/// (FMT-MARK-007); sync prints a warning naming each dropped target ID so the
/// loss is not silent (SYNC-MARK-004). A title-less bullet is
/// malformed: a warning is printed and the line plus its note lines pass
/// through verbatim, skipped for ID assignment and notes extraction
/// (FMT-PARSE-006).
///
/// ID assignment (delegated to [`id_assignment::assign_ids`]):
/// - Every well-formed bullet without an `[id]` marker gets a fresh
///   `<prefix>-<3 base32 chars>` ID emitted at the front of the bullet
///   (SYNC-ID-001..003, 005, 006).
/// - Newly-assigned IDs are appended to `backlog/.used-ids` only after
///   `backlog.md` has been written successfully (SYNC-ID-004).
///
/// For each well-formed task entry that has note lines:
/// - Strips indentation (SYNC-NOTES-004) and trims blank edges.
/// - If the stripped result is non-empty and an item file for the entry's ID
///   does not exist, creates it (SYNC-NOTES-002).
/// - If the stripped result is non-empty and an item file already exists,
///   appends to it (SYNC-NOTES-003).
/// - In all cases clears the notes from the entry in `backlog.md`
///   (SYNC-NOTES-001, SYNC-NOTES-005).
///
/// After notes extraction, when the entry's id has a corresponding item file
/// (pre-existing on disk or just created from its notes), ensures the bullet's
/// title ends with the canonical pointer suffix ` (see ./items/<id>.md)`,
/// appending it only when absent (SYNC-PTR-001, idempotent per SYNC-PTR-003).
/// When no item file exists the suffix is neither added nor stripped
/// (SYNC-PTR-002).
///
/// Writes are all-or-nothing: parsing and ID generation finish before any
/// file is touched (SYNC-WRITE-003), and a second run on canonical output is
/// byte-identical (SYNC-WRITE-001). Skips the `backlog.md` write when the
/// output is byte-identical to the input (SYNC-WRITE-002), reporting that via
/// the returned [`SyncOutcome`]. Creates `backlog/items/` on demand via
/// `item_file::write_new_stripped` (SYNC-WRITE-004).
// @spec SYNC-NOTES-001, SYNC-NOTES-002, SYNC-NOTES-003, SYNC-NOTES-004, SYNC-NOTES-005
// @spec SYNC-PTR-001, SYNC-PTR-002, SYNC-PTR-003
// @spec SYNC-PRE-001, SYNC-PRE-002
// @spec SYNC-WRITE-001, SYNC-WRITE-002, SYNC-WRITE-003, SYNC-WRITE-004
// @spec SYNC-ID-004
// @spec SYNC-MARK-001, SYNC-MARK-002, SYNC-MARK-003, SYNC-MARK-004
// @spec FMT-PARSE-006
pub(crate) fn run(backlog_dir: &Path) -> Result<SyncOutcome, SyncError> {
    let mut warnings = Vec::new();
    let result = run_impl(backlog_dir, &mut warnings);
    for warning in &warnings {
        eprintln!("{warning}");
    }
    result
}

/// [`run`] with warnings collected into `warnings` instead of printed, so
/// tests can assert on warning content without capturing stderr.
fn run_impl(backlog_dir: &Path, warnings: &mut Vec<String>) -> Result<SyncOutcome, SyncError> {
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

    // FMT-PARSE-006: parse each bullet with the shared marker tokenizer. A
    // title-less bullet is malformed — warn, preserve the line (and its note
    // lines) verbatim, and exclude it from every later step. Malformed bullets
    // are fully inert: an ID-shaped token on one does not seed the collision
    // set and does not count for duplicate detection (LLD "Decisions").
    let mut parsed: Vec<Option<Bullet>> = Vec::with_capacity(region.entries.len());
    for (i, entry) in region.entries.iter().enumerate() {
        match Bullet::parse_reporting_dropped(entry.bullet_line) {
            Ok((bullet, dropped_blocked_by)) => {
                // SYNC-MARK-004 / FMT-MARK-007: re-serialization keeps only the
                // first [blocked-by:]. Warn (naming each dropped target ID and
                // the bullet's position) so a user who listed several blockers
                // isn't silently robbed of them on their first sync.
                for dropped in &dropped_blocked_by {
                    warnings.push(format!(
                        "warning: bullet #{n} drops extra [blocked-by:{dropped}] (only the first [blocked-by:] is kept): {line:?}",
                        n = i + 1,
                        line = entry.bullet_line.trim_end()
                    ));
                }
                parsed.push(Some(bullet));
            }
            Err(BulletError::EmptyTitle) => {
                // Name the bullet by its 1-based position among task bullets
                // (not a file line number — the parsed region doesn't track
                // those) so the human fixing it can locate the line, alongside
                // the debug-escaped content.
                warnings.push(format!(
                    "warning: bullet #{} has no title, skipping: {:?}",
                    i + 1,
                    entry.bullet_line.trim_end()
                ));
                parsed.push(None);
            }
        }
    }

    // One ID slot per well-formed bullet, in `parsed` order. `Bullet::parse`
    // already lowercases IDs (FMT-MARK-001): tombstones are lowercase-normalized
    // on read, and the prefix comparison in `assign_ids` (SYNC-ID-005) is
    // against the lowercase `project.id`.
    //
    // `slots` is positionally aligned with `parsed.iter().flatten()` (the
    // well-formed bullets, in order). Both the seed below and the copy-back
    // re-derive that correspondence from the same `flatten()` iterator, so
    // there is no separate index vector that could drift out of lockstep with
    // `slots` if a future change adds another reason to skip an entry.
    let mut slots: Vec<Option<String>> = parsed.iter().flatten().map(|b| b.id.clone()).collect();

    // SYNC-ID-002: collision avoidance against tombstones ∪ IDs already
    // present in the parsed region.
    let mut used = tombstone::read(&used_ids_path)?;
    for id in slots.iter().flatten() {
        used.insert(id.clone());
    }

    let (new_ids, id_warnings) = id_assignment::assign_ids(
        &mut slots,
        &mut used,
        config.project_id(),
        &mut rand::thread_rng(),
    )?;
    warnings.extend(id_warnings);

    // `assign_ids` filled every `None` slot; copy the slots back so each
    // well-formed bullet now carries its ID (SYNC-ID-001: serialize emits the
    // `[id]` marker first, before any other markers). `parsed.iter_mut()
    // .flatten()` yields exactly the same well-formed bullets, in the same
    // order, that built `slots`, so the zip pairs each ID with its own bullet.
    for (bullet, slot) in parsed.iter_mut().flatten().zip(slots.iter()) {
        bullet.id.clone_from(slot);
    }

    // Collect all item-file writes before touching any file.  A scan-phase
    // error (e.g. disk full) therefore leaves disk state unchanged: no item
    // file is written and `backlog.md` is not modified.
    //
    // Note: the write phase (step A: item files, step B: backlog.md) is not
    // truly atomic.  A crash between A and B leaves orphaned item-file writes
    // that will be re-processed and double-appended on the next `vat sync` run.
    // Truly atomic cross-file writes require OS support that is out of scope.
    let pending = extract_notes_and_link(&mut region, &mut parsed, &items_dir);

    // Serialize. Well-formed bullets are re-emitted in canonical form —
    // markers in canonical order with single-space separators (SYNC-MARK-001),
    // values untouched apart from ID lowercasing (SYNC-MARK-002), dangling
    // [blocked-by:...] preserved (SYNC-MARK-003). Malformed bullets and their
    // notes pass through verbatim (FMT-PARSE-006).
    let mut new_parsed = String::with_capacity(bf.parsed().len() + 8 * new_ids.len());
    new_parsed.push_str(region.preamble);
    for (i, entry) in region.entries.iter().enumerate() {
        match &parsed[i] {
            Some(bullet) => new_parsed.push_str(&bullet.serialize()),
            None => new_parsed.push_str(entry.bullet_line),
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

/// Per well-formed entry: queue its notes for extraction into an item file,
/// clear the notes from the bullet, and link the bullet to its item file with
/// the pointer suffix. Returns the queued item-file writes — nothing is written
/// here; all writes happen at the end of [`run_impl`], after serialization.
///
/// `parsed` is positionally aligned with `region.entries`; a `None` slot is a
/// malformed (title-less) bullet, which keeps its note lines in place — clearing
/// them without an item file to receive them would silently destroy the notes
/// (FMT-PARSE-006).
// @spec SYNC-NOTES-001, SYNC-NOTES-005, FMT-PARSE-006
fn extract_notes_and_link(
    region: &mut ParsedRegion,
    parsed: &mut [Option<Bullet>],
    items_dir: &Path,
) -> Vec<PendingWrite> {
    let mut pending: Vec<PendingWrite> = Vec::new();
    for (i, entry) in region.entries.iter_mut().enumerate() {
        let Some(bullet) = parsed[i].as_mut() else {
            continue;
        };

        // `extracting_notes` records whether a (new or appended) item file is
        // queued for this id this run; combined with the on-disk check in
        // `apply_pointer_suffix` it answers "will an item file exist after sync?"
        let mut extracting_notes = false;
        if !entry.notes.is_empty() {
            let stripped = item_file::strip_notes(entry.notes);
            // Every well-formed bullet has an ID after `assign_ids`; a bullet
            // that just received one extracts its notes to that ID's item file
            // (LLD step 5: assignment happens before extraction).
            if !stripped.is_empty()
                && let Some(id) = &bullet.id
            {
                pending.push(PendingWrite {
                    item_path: items_dir.join(format!("{id}.md")),
                    id: id.clone(),
                    stripped,
                });
                extracting_notes = true;
            }
        }

        // SYNC-NOTES-001, SYNC-NOTES-005: always clear notes from this entry.
        entry.notes = "";

        // SYNC-PTR-001..003: link the bullet to its item file (if one exists)
        // before serialization, so the suffix is part of the SYNC-WRITE-002
        // byte-identical comparison.
        apply_pointer_suffix(bullet, items_dir, extracting_notes);
    }
    pending
}

/// Ensure `bullet`'s title ends with the canonical ` (see ./items/<id>.md)`
/// pointer suffix when an item file exists for its id.
///
/// An item file is considered to exist when `extracting_notes` is true (a new
/// or appended file is queued for this id this run) or when `items/<id>.md` is
/// already on disk. The append is idempotent — `ends_with` guards against
/// re-adding the suffix (SYNC-PTR-003) — and conservative: when no item file
/// exists the title is left untouched, so an existing suffix is never stripped
/// (SYNC-PTR-002). A bullet with no id is left untouched.
// @spec SYNC-PTR-001, SYNC-PTR-002, SYNC-PTR-003
fn apply_pointer_suffix(bullet: &mut Bullet, items_dir: &Path, extracting_notes: bool) {
    let Some(id) = bullet.id.clone() else { return };
    let item_exists = extracting_notes || items_dir.join(format!("{id}.md")).exists();
    if !item_exists {
        return;
    }
    let suffix = format!(" (see ./items/{id}.md)");
    if !bullet.title.ends_with(&suffix) {
        bullet.title.push_str(&suffix);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    use super::{SyncError, SyncOutcome, run};

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
        // SYNC-PTR-001: extracting the note creates the item file, so the
        // bullet gains the pointer suffix.
        assert_eq!(out, "- [vat-t1h] Title (see ./items/vat-t1h.md)\n");
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
        // SYNC-PTR-001: each note creates an item file, so each bullet gains
        // its own pointer suffix.
        assert_eq!(
            out,
            "- [vat-t1h] First (see ./items/vat-t1h.md)\n- [vat-g5y] Second (see ./items/vat-g5y.md)\n"
        );
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

    // ── SYNC-PTR-001..003 (item-file pointer suffix) ─────────────────────────

    // @spec SYNC-PTR-001
    #[test]
    fn run_appends_pointer_suffix_when_item_file_created() {
        let dir = setup();
        // A fresh note creates items/vat-t1h.md, so the bullet must gain the
        // canonical pointer suffix.
        write_backlog(&dir, "- [vat-t1h] Title\n  A note.\n");
        run(dir.path()).unwrap();
        assert_eq!(
            read_backlog(&dir),
            "- [vat-t1h] Title (see ./items/vat-t1h.md)\n"
        );
    }

    // @spec SYNC-PTR-001
    #[test]
    fn run_appends_pointer_suffix_when_item_file_preexists_without_notes() {
        let dir = setup();
        // The item file already exists and the bullet has no notes this run;
        // the suffix is still added because the file exists.
        let items_dir = dir.path().join("items");
        fs::create_dir_all(&items_dir).unwrap();
        fs::write(
            items_dir.join("vat-t1h.md"),
            "---\nid: vat-t1h\n---\n\nExisting.\n",
        )
        .unwrap();
        write_backlog(&dir, "- [vat-t1h] Title\n");
        run(dir.path()).unwrap();
        assert_eq!(
            read_backlog(&dir),
            "- [vat-t1h] Title (see ./items/vat-t1h.md)\n"
        );
    }

    // @spec SYNC-PTR-002
    #[test]
    fn run_does_not_add_pointer_suffix_when_no_item_file() {
        let dir = setup();
        // No notes, no pre-existing item file → no suffix.
        let content = "- [vat-t1h] Title\n";
        write_backlog(&dir, content);
        run(dir.path()).unwrap();
        assert_eq!(read_backlog(&dir), content);
    }

    // @spec SYNC-PTR-002
    #[test]
    fn run_does_not_add_suffix_for_whitespace_only_notes() {
        let dir = setup();
        // Whitespace-only notes never create an item file (SYNC-NOTES-005), so
        // no suffix is added either.
        write_backlog(&dir, "- [vat-t1h] Title\n   \n");
        run(dir.path()).unwrap();
        assert_eq!(read_backlog(&dir), "- [vat-t1h] Title\n");
        assert!(!dir.path().join("items").join("vat-t1h.md").exists());
    }

    // @spec SYNC-PTR-002
    #[test]
    fn run_keeps_existing_suffix_when_item_file_missing() {
        let dir = setup();
        // The user hand-deleted the item file but left the suffix. Sync is
        // conservative: it never strips the suffix, even with no file present.
        let content = "- [vat-t1h] Title (see ./items/vat-t1h.md)\n";
        write_backlog(&dir, content);
        let outcome = run(dir.path()).unwrap();
        assert_eq!(outcome, SyncOutcome::Skipped, "nothing to change");
        assert_eq!(read_backlog(&dir), content);
    }

    // @spec SYNC-PTR-003
    #[test]
    fn run_does_not_double_append_pointer_suffix() {
        let dir = setup();
        // Title already carries the canonical suffix and the item file exists:
        // re-sync leaves it untouched (idempotent, no doubling).
        let items_dir = dir.path().join("items");
        fs::create_dir_all(&items_dir).unwrap();
        fs::write(
            items_dir.join("vat-t1h.md"),
            "---\nid: vat-t1h\n---\n\nExisting.\n",
        )
        .unwrap();
        let content = "- [vat-t1h] Title (see ./items/vat-t1h.md)\n";
        write_backlog(&dir, content);
        let outcome = run(dir.path()).unwrap();
        assert_eq!(outcome, SyncOutcome::Skipped, "already canonical");
        assert_eq!(read_backlog(&dir), content);
    }

    // @spec SYNC-PTR-001, SYNC-PTR-003
    #[test]
    fn run_pointer_suffix_is_idempotent_across_two_runs() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Title\n  A note.\n");
        run(dir.path()).unwrap();
        let after_first = read_backlog(&dir);
        assert_eq!(after_first, "- [vat-t1h] Title (see ./items/vat-t1h.md)\n");
        let outcome = run(dir.path()).unwrap();
        assert_eq!(outcome, SyncOutcome::Skipped, "second run is a no-op");
        assert_eq!(read_backlog(&dir), after_first, "no second suffix appended");
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

    // @spec SYNC-ID-006, SYNC-WRITE-003
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
        assert_eq!(
            out, "- [vat-t1h] Title (see ./items/vat-t1h.md)\n",
            "notes cleared, pointer suffix added"
        );
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

    // ── Marker normalization (SYNC-MARK-001..003) ─────────────────────────────

    // @spec SYNC-MARK-001
    #[test]
    fn run_reorders_markers_to_canonical_order() {
        let dir = setup();
        write_backlog(
            &dir,
            "- [blocked-by:vat-f1w] [by:jared] [in-progress] [vat-t1h] Title\n",
        );
        run(dir.path()).unwrap();
        assert_eq!(
            read_backlog(&dir),
            "- [vat-t1h] [in-progress] [by:jared] [blocked-by:vat-f1w] Title\n"
        );
    }

    // @spec SYNC-MARK-001
    #[test]
    fn run_respaces_markers_to_single_spaces() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h]  [in-progress]\t Title\n");
        run(dir.path()).unwrap();
        assert_eq!(read_backlog(&dir), "- [vat-t1h] [in-progress] Title\n");
    }

    // @spec SYNC-MARK-002
    #[test]
    fn run_preserves_marker_values_while_reordering() {
        let dir = setup();
        write_backlog(
            &dir,
            "- [by:john.doe_2-dev] [vat-t1h] [blocked-by:vat-h8x] Title\n",
        );
        run(dir.path()).unwrap();
        assert_eq!(
            read_backlog(&dir),
            "- [vat-t1h] [by:john.doe_2-dev] [blocked-by:vat-h8x] Title\n"
        );
    }

    // @spec SYNC-MARK-002
    #[test]
    fn run_lowercases_id_values_as_canonicalization() {
        let dir = setup();
        write_backlog(&dir, "- [VAT-T1H] [blocked-by:VAT-F1W] Title\n");
        run(dir.path()).unwrap();
        assert_eq!(
            read_backlog(&dir),
            "- [vat-t1h] [blocked-by:vat-f1w] Title\n"
        );
    }

    // @spec SYNC-MARK-003
    #[test]
    fn run_keeps_dangling_blocked_by_marker() {
        let dir = setup();
        // vat-zzz appears nowhere else in the parsed region.
        let content = "- [vat-t1h] [blocked-by:vat-zzz] Title\n";
        write_backlog(&dir, content);
        run(dir.path()).unwrap();
        assert_eq!(read_backlog(&dir), content);
    }

    // @spec SYNC-MARK-004
    #[test]
    fn run_warns_when_a_second_blocked_by_is_dropped() {
        let dir = setup();
        // Two blockers on one bullet. FMT-MARK-007 keeps only the first, so
        // re-serialization drops vat-h8x — but the user must be told, not
        // silently robbed of a blocker on their first sync.
        write_backlog(
            &dir,
            "- [vat-t1h] [blocked-by:vat-f1w] [blocked-by:vat-h8x] Finish auth\n",
        );
        let mut warnings = Vec::new();
        let outcome = super::run_impl(dir.path(), &mut warnings).unwrap();
        assert_eq!(outcome, SyncOutcome::Wrote);
        // The first blocker survives; the second is gone from the line.
        assert_eq!(
            read_backlog(&dir),
            "- [vat-t1h] [blocked-by:vat-f1w] Finish auth\n"
        );
        // ...and the warning names the dropped target ID, the bullet number,
        // and the original line. Exact match so a reword fails loudly.
        assert_eq!(warnings.len(), 1, "exactly one warning: {warnings:?}");
        assert_eq!(
            warnings[0],
            r#"warning: bullet #1 drops extra [blocked-by:vat-h8x] (only the first [blocked-by:] is kept): "- [vat-t1h] [blocked-by:vat-f1w] [blocked-by:vat-h8x] Finish auth""#
        );
    }

    // @spec SYNC-MARK-004
    #[test]
    fn run_warns_once_per_dropped_blocker() {
        let dir = setup();
        // Three blockers: two are dropped, each warned about by target ID.
        write_backlog(
            &dir,
            "- [vat-t1h] [blocked-by:vat-f1w] [blocked-by:vat-h8x] [blocked-by:vat-k2m] Title\n",
        );
        let mut warnings = Vec::new();
        super::run_impl(dir.path(), &mut warnings).unwrap();
        assert_eq!(
            read_backlog(&dir),
            "- [vat-t1h] [blocked-by:vat-f1w] Title\n"
        );
        assert_eq!(
            warnings.len(),
            2,
            "one warning per dropped blocker: {warnings:?}"
        );
        assert!(warnings[0].contains("[blocked-by:vat-h8x]"));
        assert!(warnings[1].contains("[blocked-by:vat-k2m]"));
    }

    // ── Bullet identity follows the front-loaded parser (FMT-MARK-006) ────────

    // @spec SYNC-ID-001
    #[test]
    fn run_assigns_fresh_id_when_id_token_hides_behind_unknown_marker() {
        let dir = setup();
        // [TODO] is unknown, so [vat-xxx] is title text, not the bullet's ID:
        // the bullet has no ID and gets a fresh one front-loaded.
        write_backlog(&dir, "- [TODO] [vat-xxx] title\n");
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        let line = out.lines().next().unwrap();
        assert!(
            line.starts_with("- [vat-"),
            "fresh id front-loaded: {line:?}"
        );
        assert!(
            line.ends_with("] [TODO] [vat-xxx] title"),
            "title text (incl. the old token) preserved verbatim: {line:?}"
        );
    }

    // ── FMT-PARSE-006: title-less bullets warn and are skipped ────────────────

    // @spec FMT-PARSE-006
    #[test]
    fn run_warns_and_preserves_marker_only_bullet() {
        let dir = setup();
        // A canonical well-formed bullet precedes the malformed one so the
        // reported bullet number (#2) reflects the actual position, not just
        // a constant "#1".
        let content = "- [vat-t1h] First\n- [vat-g5y]\n";
        write_backlog(&dir, content);
        let mut warnings = Vec::new();
        let outcome = super::run_impl(dir.path(), &mut warnings).unwrap();
        assert_eq!(outcome, SyncOutcome::Skipped, "nothing else to change");
        assert_eq!(read_backlog(&dir), content, "line preserved verbatim");
        assert_eq!(warnings.len(), 1, "exactly one warning: {warnings:?}");
        // Exact match (not a substring check): a silent reword of the message
        // — or a drift in the reported bullet number / quoted line — fails the
        // test loudly instead of slipping through a `contains`.
        assert_eq!(
            warnings[0],
            r#"warning: bullet #2 has no title, skipping: "- [vat-g5y]""#
        );
    }

    // @spec FMT-PARSE-006
    #[test]
    fn run_does_not_extract_notes_of_title_less_bullet() {
        let dir = setup();
        let content = "- [vat-g5y]\n  orphaned note\n";
        write_backlog(&dir, content);
        run(dir.path()).unwrap();
        assert_eq!(
            read_backlog(&dir),
            content,
            "bullet line AND its notes preserved in place"
        );
        assert!(
            !dir.path().join("items").exists(),
            "no item file for a skipped bullet"
        );
    }

    // @spec FMT-PARSE-006
    #[test]
    fn run_skips_title_less_bullet_for_id_assignment() {
        let dir = setup();
        write_backlog(&dir, "- \n");
        let mut warnings = Vec::new();
        super::run_impl(dir.path(), &mut warnings).unwrap();
        assert_eq!(read_backlog(&dir), "- \n");
        assert!(!dir.path().join(".used-ids").exists(), "no id assigned");
        assert_eq!(warnings.len(), 1, "warned: {warnings:?}");
    }

    // @spec FMT-PARSE-006
    #[test]
    fn run_processes_well_formed_bullets_around_a_skipped_one() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] First\n- [vat-g5y]\n- New task\n");
        run(dir.path()).unwrap();
        let out = read_backlog(&dir);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "- [vat-t1h] First");
        assert_eq!(lines[1], "- [vat-g5y]", "malformed line untouched");
        assert!(
            lines[2].starts_with("- [vat-") && lines[2].ends_with("] New task"),
            "bullet after the skipped one still gets an id: {:?}",
            lines[2]
        );
    }

    // @spec FMT-PARSE-006
    #[test]
    fn run_title_less_bullet_id_token_is_inert_for_duplicate_detection() {
        let dir = setup();
        // A well-formed bullet and a malformed one carry the same id token;
        // the malformed bullet is inert, so this is NOT a duplicate-id error.
        let content = "- [vat-abc] Real task\n- [vat-abc]\n";
        write_backlog(&dir, content);
        let outcome = run(dir.path()).unwrap();
        assert_eq!(outcome, SyncOutcome::Skipped);
        assert_eq!(read_backlog(&dir), content);
    }

    // ── SYNC-WRITE-001: idempotence incl. marker normalization ───────────────

    // @spec SYNC-WRITE-001
    #[test]
    fn run_twice_is_byte_identical_and_second_run_skips() {
        let dir = setup();
        write_backlog(
            &dir,
            "- [in-progress]  [vat-t1h] Messy\n  a note\n- No id yet\n",
        );
        let first = run(dir.path()).unwrap();
        assert_eq!(first, SyncOutcome::Wrote);
        let after_first = read_backlog(&dir);
        let second = run(dir.path()).unwrap();
        assert_eq!(second, SyncOutcome::Skipped, "second run is a no-op");
        assert_eq!(read_backlog(&dir), after_first);
    }

    // @spec SYNC-WRITE-001
    #[test]
    fn run_normalizes_missing_trailing_newline() {
        let dir = setup();
        write_backlog(&dir, "- [vat-t1h] Title");
        let outcome = run(dir.path()).unwrap();
        assert_eq!(outcome, SyncOutcome::Wrote);
        assert_eq!(read_backlog(&dir), "- [vat-t1h] Title\n");
        assert_eq!(run(dir.path()).unwrap(), SyncOutcome::Skipped);
    }
}
