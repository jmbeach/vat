// @spec CMD-DONE-001, CMD-DONE-002, CMD-DONE-003, CMD-DONE-004, CMD-DONE-005, CMD-CC-001, CMD-CC-002, CMD-CC-003, CMD-CC-004

use std::path::Path;

use anyhow::Context;

use crate::backlog_file::{BacklogFile, ParsedRegion, check_version};
use crate::bullet::Bullet;
use crate::cmd_start::{EntryLookup, find_entry_index};
use crate::errors::UserError;
use crate::{file_io, tombstone};

/// Complete the task `id`: remove its bullet from `backlog.md`, delete its item
/// file, tombstone the id, and auto-unblock any dependents. Returns the
/// user-facing confirmation message on success.
// @spec CMD-DONE-001, CMD-DONE-002, CMD-DONE-003, CMD-DONE-004, CMD-DONE-005
pub(crate) fn run(id: &str, backlog_dir: &Path) -> anyhow::Result<String> {
    let backlog_path = backlog_dir.join("backlog.md");
    let input = file_io::read_to_string(&backlog_path)
        .with_context(|| format!("reading {}", backlog_path.display()))?;

    let bf = BacklogFile::parse(&input);

    // CMD-CC-001: version gate before any other processing.
    check_version(bf.frontmatter()).context("backlog version check")?;

    let id_lower = id.to_lowercase();
    let region = ParsedRegion::parse(bf.parsed());

    // CMD-CC-002 / CMD-CC-004: locate the matching entry; abort without writes if
    // not found, or with the parse failure if the bullet is present but malformed.
    let target_idx = match find_entry_index(&region, &id_lower) {
        // `done` removes the whole entry, so the parsed bullet is unused here.
        EntryLookup::Found(idx, _) => idx,
        EntryLookup::Malformed(err) => {
            return Err(UserError(format!(
                "{id} found but its bullet could not be parsed: {err}"
            ))
            .into());
        }
        EntryLookup::NotFound => return Err(UserError(format!("unknown id: {id}")).into()),
    };

    // CMD-DONE-001 (remove the entry), CMD-DONE-004 (auto-unblock dependents),
    // CMD-DONE-005 (the target's own [blocked-by:...] is irrelevant — the entry
    // is dropped regardless). Computed before any file write so the backlog
    // mutation is all-or-nothing.
    let new_parsed = serialize_completing(&region, target_idx, &id_lower);
    let output = bf.serialize(&new_parsed);

    // CMD-DONE-002: delete the item file if present. We call `remove_file`
    // directly and treat NotFound as success rather than guarding with
    // `exists()` first — `done` doesn't require an item file to exist, and the
    // single syscall has no TOCTOU window between a check and the removal.
    let item_path = backlog_dir.join("items").join(format!("{id_lower}.md"));
    match std::fs::remove_file(&item_path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(anyhow::Error::from(e))
                .with_context(|| format!("deleting {}", item_path.display()));
        }
    }

    // CMD-DONE-003: tombstone the id, but only if it isn't already recorded.
    let used_ids_path = backlog_dir.join(".used-ids");
    let used = tombstone::read(&used_ids_path).context("reading .used-ids")?;
    if !used.contains(&id_lower) {
        tombstone::append(&used_ids_path, &[id_lower.as_str()])
            .context("appending to .used-ids")?;
    }

    // CMD-DONE-001 / CMD-DONE-004: commit the backlog rewrite last. If a
    // bookkeeping step above fails the bullet is still present, so a re-run
    // finds it and completes idempotently.
    file_io::write(&backlog_path, &output)
        .with_context(|| format!("writing {}", backlog_path.display()))?;

    Ok(format!("done {id_lower}"))
}

/// Serialize `region` with the entry at `target_idx` dropped in full and every
/// other bullet whose `[blocked-by:...]` targets `done_id` re-emitted without
/// that marker.
///
/// Dropping the whole entry — bullet line *and* its note lines — removes the
/// blank line that the parsed-region grammar parks immediately after the bullet.
/// Since the blank separating neighbours lives in the *preceding* entry's notes,
/// the survivors stay separated by a single blank and no double blank is left at
/// the seam (CMD-DONE-001).
// @spec CMD-DONE-001, CMD-DONE-004, CMD-CC-003
fn serialize_completing(region: &ParsedRegion<'_>, target_idx: usize, done_id: &str) -> String {
    // Seed capacity from the whole parsed region (preamble + every entry's
    // bullet and notes). The output is that minus one dropped entry, so this is
    // a safe upper bound that avoids reallocation as the string grows.
    let estimated = region.preamble.len()
        + region
            .entries
            .iter()
            .map(|e| e.bullet_line.len() + e.notes.len())
            .sum::<usize>();
    let mut out = String::with_capacity(estimated);
    out.push_str(region.preamble);
    for (i, entry) in region.entries.iter().enumerate() {
        if i == target_idx {
            continue;
        }
        match Bullet::parse(entry.bullet_line) {
            // CMD-DONE-004 + CMD-CC-003: strip the now-satisfied blocker and
            // re-emit in canonical marker order.
            Ok(mut bullet) if bullet.blocked_by.as_deref() == Some(done_id) => {
                bullet.blocked_by = None;
                out.push_str(&bullet.serialize());
            }
            // Well-formed-but-unblocked, or malformed (FMT-PARSE-006 inert):
            // pass the bullet through verbatim so `done` only touches what it must.
            Ok(_) | Err(_) => out.push_str(entry.bullet_line),
        }
        out.push_str(entry.notes);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use super::run;
    use crate::backlog_file::SUPPORTED_MAJOR;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_backlog_dir(dir: &TempDir) -> PathBuf {
        let backlog = dir.path().join("backlog");
        fs::create_dir_all(&backlog).unwrap();
        backlog
    }

    fn write_backlog(backlog: &Path, content: &str) {
        fs::write(backlog.join("backlog.md"), content).unwrap();
    }

    fn read_backlog(backlog: &Path) -> String {
        fs::read_to_string(backlog.join("backlog.md")).unwrap()
    }

    fn write_item(backlog: &Path, id: &str, body: &str) -> PathBuf {
        let items = backlog.join("items");
        fs::create_dir_all(&items).unwrap();
        let path = items.join(format!("{id}.md"));
        fs::write(&path, body).unwrap();
        path
    }

    fn write_used_ids(backlog: &Path, content: &str) -> PathBuf {
        let path = backlog.join(".used-ids");
        fs::write(&path, content).unwrap();
        path
    }

    fn read_used_ids(backlog: &Path) -> String {
        fs::read_to_string(backlog.join(".used-ids")).unwrap()
    }

    const HEADER: &str = "---\nversion: 1\n---\n\n";

    // -----------------------------------------------------------------------
    // CMD-DONE-001 — remove the bullet line
    // -----------------------------------------------------------------------

    // @spec CMD-DONE-001
    #[test]
    fn done_removes_the_only_bullet() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));

        let msg = run("vat-g5y", &backlog).unwrap();
        assert_eq!(msg, "done vat-g5y");

        // The bullet is gone; the frontmatter and preamble blank line remain.
        assert_eq!(read_backlog(&backlog), HEADER);
    }

    // @spec CMD-DONE-001
    #[test]
    fn done_removes_only_the_matching_bullet_among_consecutive_bullets() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-aaa] First\n- [vat-bbb] Second\n- [vat-ccc] Third\n"),
        );

        run("vat-bbb", &backlog).unwrap();

        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-aaa] First\n- [vat-ccc] Third\n")
        );
    }

    // @spec CMD-DONE-001
    //
    // Blank-separated bullets: removing the middle entry (bullet + its trailing
    // blank) must leave the neighbours separated by exactly one blank line, never
    // two.
    #[test]
    fn done_blank_separated_bullets_leaves_single_blank_not_double() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-aaa] First\n\n- [vat-bbb] Second\n\n- [vat-ccc] Third\n"),
        );

        run("vat-bbb", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert_eq!(
            content,
            format!("{HEADER}- [vat-aaa] First\n\n- [vat-ccc] Third\n")
        );
        assert!(
            !content.contains("\n\n\n"),
            "must not leave a double blank line: {content:?}"
        );
    }

    // @spec CMD-DONE-001
    #[test]
    fn done_removes_first_blank_separated_entry() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-aaa] First\n\n- [vat-bbb] Second\n"),
        );

        run("vat-aaa", &backlog).unwrap();

        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-bbb] Second\n")
        );
    }

    // @spec CMD-DONE-001
    //
    // Removing a done entry also drops the note lines that belong to it, rather
    // than orphaning them under the preceding bullet.
    #[test]
    fn done_removes_the_entrys_own_notes() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!(
                "{HEADER}- [vat-aaa] First\n- [vat-bbb] Second\n  a note for bbb\n  more notes\n- [vat-ccc] Third\n"
            ),
        );

        run("vat-bbb", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert_eq!(
            content,
            format!("{HEADER}- [vat-aaa] First\n- [vat-ccc] Third\n")
        );
        assert!(!content.contains("a note for bbb"), "{content}");
    }

    // @spec CMD-DONE-001
    #[test]
    fn done_preserves_other_entries_notes() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-aaa] First\n  keep this note\n- [vat-bbb] Second\n"),
        );

        run("vat-bbb", &backlog).unwrap();

        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-aaa] First\n  keep this note\n")
        );
    }

    // @spec CMD-DONE-001
    #[test]
    fn done_preserves_preamble_and_freeform_regions() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            "---\nversion: 1\n---\n# Title\n\n- [vat-g5y] A task\n- [vat-h8x] Another\n---\nFreeform\n",
        );

        run("vat-g5y", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert!(content.contains("# Title\n"), "{content}");
        assert!(content.contains("---\nFreeform\n"), "{content}");
        assert!(content.contains("- [vat-h8x] Another\n"), "{content}");
        assert!(!content.contains("vat-g5y"), "{content}");
    }

    // @spec CMD-DONE-001
    #[test]
    fn done_accepts_uppercase_id() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));

        let msg = run("VAT-G5Y", &backlog).unwrap();
        assert_eq!(msg, "done vat-g5y");
        assert_eq!(read_backlog(&backlog), HEADER);
    }

    // -----------------------------------------------------------------------
    // CMD-CC-001 — version gate (before any other processing or writes)
    // -----------------------------------------------------------------------

    // @spec CMD-CC-001
    #[test]
    fn done_aborts_on_unsupported_backlog_version() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let future = SUPPORTED_MAJOR + 1;
        let original = format!("---\nversion: {future}\n---\n- [vat-g5y] A task\n");
        write_backlog(&backlog, &original);

        let err = run("vat-g5y", &backlog).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("version") || msg.contains("upgrade"),
            "expected version error: {msg}"
        );
        assert_eq!(read_backlog(&backlog), original, "must not write on abort");
    }

    // -----------------------------------------------------------------------
    // CMD-CC-002 — unknown id
    // -----------------------------------------------------------------------

    // @spec CMD-CC-002
    #[test]
    fn done_aborts_on_unknown_id() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y] A task\n");
        write_backlog(&backlog, &original);

        let err = run("vat-x9z", &backlog).unwrap_err();
        assert!(err.to_string().contains("unknown id"), "{err}");
    }

    // @spec CMD-CC-002
    #[test]
    fn done_does_not_touch_any_file_when_id_unknown() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y] A task\n");
        write_backlog(&backlog, &original);
        let item = write_item(&backlog, "vat-x9z", "notes\n");
        write_used_ids(&backlog, "");

        // Tie the no-write assertions to the error path they guard: an unknown id
        // must abort, not silently succeed having done nothing.
        let err = run("vat-x9z", &backlog).unwrap_err();
        assert!(err.to_string().contains("unknown id"), "{err}");

        assert_eq!(read_backlog(&backlog), original);
        assert!(item.exists(), "unknown-id abort must not delete item files");
        assert_eq!(read_used_ids(&backlog), "", "must not tombstone on abort");
    }

    // -----------------------------------------------------------------------
    // CMD-CC-004 — present but malformed bullet
    // -----------------------------------------------------------------------

    // @spec CMD-CC-004
    #[test]
    fn done_reports_parse_error_when_bullet_present_but_malformed() {
        // `- [vat-g5y]` carries the id but has no title.
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y]\n");
        write_backlog(&backlog, &original);

        let err = run("vat-g5y", &backlog).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("could not be parsed"), "{msg}");
        assert!(!msg.contains("unknown id"), "{msg}");
        assert_eq!(read_backlog(&backlog), original, "must not write on abort");
    }

    // -----------------------------------------------------------------------
    // CMD-DONE-002 — delete the item file
    // -----------------------------------------------------------------------

    // @spec CMD-DONE-002
    #[test]
    fn done_deletes_existing_item_file() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));
        let item = write_item(&backlog, "vat-g5y", "---\nid: vat-g5y\n---\n\nnotes\n");
        assert!(item.exists());

        run("vat-g5y", &backlog).unwrap();

        assert!(!item.exists(), "item file should be deleted");
    }

    // @spec CMD-DONE-002
    #[test]
    fn done_succeeds_when_no_item_file_exists() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));

        // No items/ directory at all.
        run("vat-g5y", &backlog).unwrap();
        assert_eq!(read_backlog(&backlog), HEADER);
    }

    // @spec CMD-DONE-002
    #[test]
    fn done_deletes_only_the_matching_item_file() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] A task\n- [vat-h8x] Other\n"),
        );
        let gone = write_item(&backlog, "vat-g5y", "notes\n");
        let kept = write_item(&backlog, "vat-h8x", "other notes\n");

        run("vat-g5y", &backlog).unwrap();

        assert!(!gone.exists());
        assert!(kept.exists(), "other item files must be untouched");
    }

    // -----------------------------------------------------------------------
    // CMD-DONE-003 — append to .used-ids if not already present
    // -----------------------------------------------------------------------

    // @spec CMD-DONE-003
    #[test]
    fn done_appends_id_to_used_ids() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));
        write_used_ids(&backlog, "vat-aaa\n");

        run("vat-g5y", &backlog).unwrap();

        assert_eq!(read_used_ids(&backlog), "vat-aaa\nvat-g5y\n");
    }

    // @spec CMD-DONE-003
    #[test]
    fn done_creates_used_ids_when_missing() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));

        run("vat-g5y", &backlog).unwrap();

        assert_eq!(read_used_ids(&backlog), "vat-g5y\n");
    }

    // @spec CMD-DONE-003
    #[test]
    fn done_does_not_duplicate_id_already_in_used_ids() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));
        write_used_ids(&backlog, "vat-g5y\n");

        run("vat-g5y", &backlog).unwrap();

        assert_eq!(
            read_used_ids(&backlog),
            "vat-g5y\n",
            "id already tombstoned must not be appended again"
        );
    }

    // @spec CMD-DONE-003
    #[test]
    fn done_treats_used_ids_membership_case_insensitively() {
        // The tombstone stores ids lowercased; a previously-recorded uppercase
        // entry must still count as present.
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));
        write_used_ids(&backlog, "VAT-G5Y\n");

        run("vat-g5y", &backlog).unwrap();

        assert_eq!(
            read_used_ids(&backlog),
            "VAT-G5Y\n",
            "case-different existing id must not be re-appended"
        );
    }

    // -----------------------------------------------------------------------
    // CMD-DONE-004 — auto-unblock dependents
    // -----------------------------------------------------------------------

    // @spec CMD-DONE-004
    #[test]
    fn done_strips_blocked_by_from_dependent() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] Blocker\n- [vat-h8x] [blocked-by:vat-g5y] Dependent\n"),
        );

        run("vat-g5y", &backlog).unwrap();

        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-h8x] Dependent\n")
        );
    }

    // @spec CMD-DONE-004
    #[test]
    fn done_strips_blocked_by_from_multiple_dependents() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!(
                "{HEADER}- [vat-g5y] Blocker\n- [vat-h8x] [blocked-by:vat-g5y] One\n- [vat-j2k] [blocked-by:vat-g5y] Two\n"
            ),
        );

        run("vat-g5y", &backlog).unwrap();

        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-h8x] One\n- [vat-j2k] Two\n")
        );
    }

    // @spec CMD-DONE-004
    #[test]
    fn done_leaves_blocked_by_targeting_a_different_id() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] Blocker\n- [vat-h8x] [blocked-by:vat-f1w] Dependent\n"),
        );

        run("vat-g5y", &backlog).unwrap();

        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-h8x] [blocked-by:vat-f1w] Dependent\n"),
            "a blocker on a different id must survive"
        );
    }

    // @spec CMD-DONE-004, CMD-CC-003
    #[test]
    fn done_unblock_preserves_other_markers_in_canonical_order() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!(
                "{HEADER}- [vat-g5y] Blocker\n- [vat-h8x] [in-progress] [by:alice] [blocked-by:vat-g5y] Dependent\n"
            ),
        );

        run("vat-g5y", &backlog).unwrap();

        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-h8x] [in-progress] [by:alice] Dependent\n"),
            "in-progress/by markers and title preserved; blocker stripped"
        );
    }

    // @spec CMD-DONE-004
    #[test]
    fn done_unblock_matches_id_case_insensitively() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] Blocker\n- [vat-h8x] [blocked-by:vat-g5y] Dependent\n"),
        );

        // Uppercase argument still unblocks the lowercase dependent marker.
        run("VAT-G5Y", &backlog).unwrap();

        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-h8x] Dependent\n")
        );
    }

    // -----------------------------------------------------------------------
    // CMD-DONE-005 — done on a blocked task is allowed
    // -----------------------------------------------------------------------

    // @spec CMD-DONE-005
    #[test]
    fn done_succeeds_on_a_blocked_task() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!(
                "{HEADER}- [vat-f1w] Blocker\n- [vat-g5y] [blocked-by:vat-f1w] Blocked task\n"
            ),
        );

        let msg = run("vat-g5y", &backlog).unwrap();
        assert_eq!(msg, "done vat-g5y");

        // The blocked task is removed; its blocker bullet is left untouched.
        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-f1w] Blocker\n")
        );
    }

    // @spec CMD-DONE-004, CMD-DONE-005
    //
    // Completing a task that is itself blocked AND blocks another: the entry is
    // removed and the dependent is auto-unblocked in the same pass.
    #[test]
    fn done_on_blocked_task_also_unblocks_its_own_dependent() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!(
                "{HEADER}- [vat-f1w] Root\n- [vat-g5y] [blocked-by:vat-f1w] Middle\n- [vat-h8x] [blocked-by:vat-g5y] Leaf\n"
            ),
        );

        run("vat-g5y", &backlog).unwrap();

        assert_eq!(
            read_backlog(&backlog),
            format!("{HEADER}- [vat-f1w] Root\n- [vat-h8x] Leaf\n")
        );
    }
}
