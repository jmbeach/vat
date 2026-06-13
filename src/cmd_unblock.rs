// @spec CMD-UNBLOCK-001, CMD-UNBLOCK-002, CMD-CC-001, CMD-CC-002, CMD-CC-003, CMD-CC-004

use std::path::Path;

use anyhow::Context;

use crate::backlog_file::{BacklogFile, ParsedRegion, check_version};
use crate::bullet::Bullet;
use crate::cmd_start::{EntryLookup, find_entry_index, serialize_region_with_replaced_bullet};
use crate::errors::UserError;
use crate::file_io;

/// Remove the `[blocked-by:...]` marker from the bullet carrying `id`. Returns
/// the user-facing confirmation message on success. When the bullet has no
/// blocker the file is left untouched (CMD-UNBLOCK-001).
// @spec CMD-UNBLOCK-001, CMD-UNBLOCK-002
pub(crate) fn run(id: &str, backlog_dir: &Path) -> anyhow::Result<String> {
    let backlog_path = backlog_dir.join("backlog.md");
    let input = file_io::read_to_string(&backlog_path)
        .with_context(|| format!("reading {}", backlog_path.display()))?;

    let bf = BacklogFile::parse(&input);

    // CMD-CC-001: version gate before any other processing.
    check_version(bf.frontmatter()).context("backlog version check")?;

    let id_lower = id.to_lowercase();
    let region = ParsedRegion::parse(bf.parsed());

    // CMD-CC-002 / CMD-CC-004: locate the matching entry; abort without writes
    // if not found, or with the parse failure if the bullet is present but
    // malformed.
    let entry_idx = match find_entry_index(&region, &id_lower) {
        EntryLookup::Found(idx) => idx,
        EntryLookup::Malformed(err) => {
            return Err(UserError(format!(
                "{id} found but its bullet could not be parsed: {err}"
            ))
            .into());
        }
        EntryLookup::NotFound => return Err(UserError(format!("unknown id: {id}")).into()),
    };

    let mut bullet = Bullet::parse(region.entries[entry_idx].bullet_line)
        .expect("entry bullet must parse: find_entry_index only returns parsable entries");

    // CMD-UNBLOCK-001: no blocker → no-op success, leaving the file untouched.
    // We return before any write so the bullet's existing spacing/casing is not
    // re-serialized (which would otherwise count as a modification).
    if bullet.blocked_by.is_none() {
        return Ok(format!("unblocked {id_lower}"));
    }

    // CMD-UNBLOCK-002 + CMD-CC-003: drop the marker and re-serialize through the
    // shared emitter, which preserves canonical marker order by construction.
    // `Bullet::parse` already collapsed any extra `[blocked-by:...]` markers
    // (FMT-MARK-007), so clearing the single field unblocks the task fully.
    bullet.blocked_by = None;
    let new_bullet_line = bullet.serialize();

    let new_parsed = serialize_region_with_replaced_bullet(&region, entry_idx, &new_bullet_line);
    let output = bf.serialize(&new_parsed);
    file_io::write(&backlog_path, &output)
        .with_context(|| format!("writing {}", backlog_path.display()))?;

    Ok(format!("unblocked {id_lower}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

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

    fn write_backlog(backlog: &std::path::Path, content: &str) {
        fs::write(backlog.join("backlog.md"), content).unwrap();
    }

    fn read_backlog(backlog: &std::path::Path) -> String {
        fs::read_to_string(backlog.join("backlog.md")).unwrap()
    }

    const HEADER: &str = "---\nversion: 1\n---\n\n";

    // -----------------------------------------------------------------------
    // CMD-UNBLOCK-002 — remove the marker
    // -----------------------------------------------------------------------

    // @spec CMD-UNBLOCK-002
    #[test]
    fn unblock_removes_blocked_by_marker() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [blocked-by:vat-f1w] A task\n"),
        );

        let msg = run("vat-g5y", &backlog).unwrap();
        assert_eq!(msg, "unblocked vat-g5y");

        let content = read_backlog(&backlog);
        assert!(
            content.contains("- [vat-g5y] A task\n"),
            "blocker marker should be gone: {content}"
        );
        assert!(
            !content.contains("blocked-by"),
            "no blocked-by marker should remain: {content}"
        );
    }

    // @spec CMD-UNBLOCK-002
    #[test]
    fn unblock_preserves_other_markers_on_the_same_bullet() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [in-progress] [by:alice] [blocked-by:vat-f1w] A task\n"),
        );

        run("vat-g5y", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert!(
            content.contains("- [vat-g5y] [in-progress] [by:alice] A task\n"),
            "only blocked-by should be stripped: {content}"
        );
    }

    // FMT-MARK-007: `Bullet::parse` keeps only the first blocker, so unblock
    // collapses a multi-blocker bullet to no blocker at all — the unblock goal.
    // @spec CMD-UNBLOCK-002
    #[test]
    fn unblock_removes_all_when_multiple_blocked_by_present() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [blocked-by:vat-f1w] [blocked-by:vat-h8x] A task\n"),
        );

        run("vat-g5y", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert!(
            !content.contains("blocked-by"),
            "all blocked-by markers should be gone: {content}"
        );
        assert!(content.contains("- [vat-g5y] A task\n"), "{content}");
    }

    // @spec CMD-UNBLOCK-002
    #[test]
    fn unblock_accepts_id_in_uppercase() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [blocked-by:vat-f1w] A task\n"),
        );

        let msg = run("VAT-G5Y", &backlog).unwrap();
        assert_eq!(msg, "unblocked vat-g5y");

        let content = read_backlog(&backlog);
        assert!(!content.contains("blocked-by"), "{content}");
    }

    // @spec CMD-UNBLOCK-002
    #[test]
    fn unblock_preserves_notes_and_other_bullets() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!(
                "{HEADER}- [vat-g5y] [blocked-by:vat-f1w] A task\n  a note\n- [vat-h8x] Second\n"
            ),
        );

        run("vat-g5y", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert!(content.contains("  a note\n"), "notes preserved: {content}");
        assert!(
            content.contains("- [vat-h8x] Second\n"),
            "other bullet preserved: {content}"
        );
    }

    // @spec CMD-UNBLOCK-002
    #[test]
    fn unblock_preserves_preamble_and_freeform_regions() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            "---\nversion: 1\n---\n# Title\n\n- [vat-g5y] [blocked-by:vat-f1w] A task\n---\nFreeform\n",
        );

        run("vat-g5y", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert!(content.contains("# Title\n"), "{content}");
        assert!(content.contains("---\nFreeform\n"), "{content}");
        assert!(!content.contains("blocked-by"), "{content}");
    }

    // -----------------------------------------------------------------------
    // CMD-UNBLOCK-001 — no-op when not blocked
    // -----------------------------------------------------------------------

    // @spec CMD-UNBLOCK-001
    #[test]
    fn unblock_is_noop_when_no_blocked_by_and_leaves_file_unchanged() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y] A task\n");
        write_backlog(&backlog, &original);

        let msg = run("vat-g5y", &backlog).unwrap();
        assert_eq!(msg, "unblocked vat-g5y");
        // CMD-UNBLOCK-001: the file must be byte-for-byte unchanged.
        assert_eq!(read_backlog(&backlog), original);
    }

    // @spec CMD-UNBLOCK-001
    #[test]
    fn unblock_noop_does_not_renormalize_unrelated_markers() {
        // A bullet with non-canonical inter-marker spacing and no blocker must
        // pass through untouched — unblock must not re-serialize (which would
        // respace it) when there is nothing to remove.
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y]   [in-progress]   A task\n");
        write_backlog(&backlog, &original);

        run("vat-g5y", &backlog).unwrap();
        assert_eq!(read_backlog(&backlog), original);
    }

    // -----------------------------------------------------------------------
    // CMD-CC-002 — unknown ID
    // -----------------------------------------------------------------------

    // @spec CMD-CC-002
    #[test]
    fn unblock_aborts_on_unknown_id() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [blocked-by:vat-f1w] A task\n"),
        );

        let err = run("vat-x9z", &backlog).unwrap_err();
        assert!(
            err.to_string().contains("unknown id"),
            "{}",
            err.to_string()
        );
    }

    // @spec CMD-CC-002
    #[test]
    fn unblock_does_not_write_file_when_id_is_unknown() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y] [blocked-by:vat-f1w] A task\n");
        write_backlog(&backlog, &original);

        let _ = run("vat-x9z", &backlog);
        assert_eq!(read_backlog(&backlog), original);
    }

    // -----------------------------------------------------------------------
    // CMD-CC-004 — malformed bullet for a present id
    // -----------------------------------------------------------------------

    // @spec CMD-CC-004
    #[test]
    fn unblock_reports_parse_error_when_bullet_present_but_malformed() {
        // `- [vat-g5y]` carries the id but has no title: Bullet::parse rejects
        // it. The user must see a parse error, not a misleading "unknown id".
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y]\n"));

        let err = run("vat-g5y", &backlog).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("could not be parsed"),
            "expected parse error, got: {msg}"
        );
        assert!(
            !msg.contains("unknown id"),
            "must not report unknown id for a present-but-malformed bullet: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // CMD-CC-001 — version gate
    // -----------------------------------------------------------------------

    // @spec CMD-CC-001
    #[test]
    fn unblock_aborts_on_unsupported_backlog_version() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let future = SUPPORTED_MAJOR + 1;
        write_backlog(
            &backlog,
            &format!("---\nversion: {future}\n---\n- [vat-g5y] [blocked-by:vat-f1w] A task\n"),
        );

        let err = run("vat-g5y", &backlog).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("version") || msg.contains("upgrade"),
            "expected version error: {msg}"
        );
    }
}
