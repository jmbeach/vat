// @spec CMD-BLOCK-001, CMD-BLOCK-002, CMD-BLOCK-002a, CMD-BLOCK-003, CMD-BLOCK-004, CMD-BLOCK-005, CMD-BLOCK-006, CMD-CC-001, CMD-CC-002, CMD-CC-003, CMD-CC-004

use std::path::Path;

use anyhow::Context;

use crate::backlog_file::{BacklogFile, ParsedRegion, check_version};
use crate::cmd_start::{EntryLookup, find_entry_index, serialize_region_with_replaced_bullet};
use crate::errors::UserError;
use crate::file_io;

/// Add a `[blocked-by:<blocker_id>]` marker to the bullet carrying `id`. Returns
/// the user-facing confirmation message on success.
///
/// v1 supports a single blocker per task: an existing different blocker is
/// replaced, and re-blocking by the same id is an idempotent no-op (no write).
// @spec CMD-BLOCK-001, CMD-BLOCK-002, CMD-BLOCK-002a, CMD-BLOCK-003, CMD-BLOCK-004, CMD-BLOCK-005, CMD-BLOCK-006
pub(crate) fn run(id: &str, blocker_id: &str, backlog_dir: &Path) -> anyhow::Result<String> {
    let id_lower = id.to_lowercase();
    let blocker_lower = blocker_id.to_lowercase();

    // CMD-BLOCK-001: self-block guard. A pure-argument check independent of file
    // state, so it runs before any file read or lookup — the error names the real
    // mistake (the same id typed twice) even when that id is absent.
    if id_lower == blocker_lower {
        return Err(UserError(format!("cannot block {id_lower} by itself")).into());
    }

    let backlog_path = backlog_dir.join("backlog.md");
    let input = file_io::read_to_string(&backlog_path)
        .with_context(|| format!("reading {}", backlog_path.display()))?;

    let bf = BacklogFile::parse(&input);

    // CMD-CC-001: version gate before any other processing.
    check_version(bf.frontmatter()).context("backlog version check")?;

    let region = ParsedRegion::parse(bf.parsed());

    // CMD-CC-002 / CMD-CC-004: locate the target; abort without writes if not
    // found, or with the parse failure if the bullet is present but malformed.
    let (entry_idx, mut bullet) = match find_entry_index(&region, &id_lower) {
        EntryLookup::Found(idx, bullet) => (idx, bullet),
        EntryLookup::Malformed(err) => {
            return Err(UserError(format!(
                "{id} found but its bullet could not be parsed: {err}"
            ))
            .into());
        }
        EntryLookup::NotFound => return Err(UserError(format!("unknown id: {id}")).into()),
    };

    // CMD-BLOCK-002 / CMD-BLOCK-002a: the blocker must match a *well-formed*
    // bullet. Requiring a parseable bullet guarantees `blocker_lower` is a valid
    // <3>-<3> id before it is written into a `[blocked-by:...]` marker — a marker
    // pointing at an id the emitter would reject (FMT-MARK-004 invariant) must
    // never be produced. A bullet whose leading `[id]` marker matches the blocker
    // but fails to parse is reported as a parse failure (CMD-BLOCK-002a), mirroring
    // the target-id handling above (CMD-CC-004), so the user is not told a plainly
    // present id is "unknown".
    match find_entry_index(&region, &blocker_lower) {
        EntryLookup::Found(..) => {}
        EntryLookup::Malformed(err) => {
            return Err(UserError(format!(
                "blocker {blocker_id} found but its bullet could not be parsed: {err}"
            ))
            .into());
        }
        EntryLookup::NotFound => {
            return Err(UserError(format!("unknown blocker: {blocker_id}")).into());
        }
    }

    // Computed once and shared by both the no-op and the write path so the
    // message format can never drift between them.
    let success = format!("blocked {id_lower} by {blocker_lower}");

    // CMD-BLOCK-003: idempotent re-block by the same id is a no-op — return
    // success without touching the file.
    if bullet.blocked_by.as_deref() == Some(blocker_lower.as_str()) {
        return Ok(success);
    }

    // CMD-BLOCK-004 + CMD-BLOCK-005 + CMD-CC-003: set the blocker, replacing any
    // existing different one (v1 = single blocker). Bullet::serialize emits the
    // marker in canonical position ([blocked-by:...] after [by:...]).
    bullet.blocked_by = Some(blocker_lower);
    let new_bullet_line = bullet.serialize();

    let new_parsed = serialize_region_with_replaced_bullet(&region, entry_idx, &new_bullet_line);
    let output = bf.serialize(&new_parsed);
    file_io::write(&backlog_path, &output)
        .with_context(|| format!("writing {}", backlog_path.display()))?;

    Ok(success)
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

    // Two unblocked tasks: vat-g5y (target) and vat-f1w (blocker).
    fn two_tasks() -> String {
        format!("{HEADER}- [vat-g5y] First\n- [vat-f1w] Second\n")
    }

    // -----------------------------------------------------------------------
    // CMD-BLOCK-001 — self-block guard
    // -----------------------------------------------------------------------

    // @spec CMD-BLOCK-001
    #[test]
    fn block_aborts_when_id_equals_blocker() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &two_tasks());

        let err = run("vat-g5y", "vat-g5y", &backlog).unwrap_err();
        assert!(
            err.to_string().contains("cannot block vat-g5y by itself"),
            "{}",
            err.to_string()
        );
    }

    // @spec CMD-BLOCK-001
    #[test]
    fn block_self_block_guard_is_case_insensitive() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &two_tasks());

        let err = run("VAT-G5Y", "vat-g5y", &backlog).unwrap_err();
        assert!(
            err.to_string().contains("cannot block vat-g5y by itself"),
            "{}",
            err.to_string()
        );
    }

    // @spec CMD-BLOCK-001
    #[test]
    fn block_self_block_guard_fires_before_file_read() {
        // The pure-argument self-block check must win even when neither id is
        // present (here the backlog file does not even exist).
        let dir = TempDir::new().unwrap();
        let backlog = dir.path().join("backlog"); // not created

        let err = run("vat-zzz", "vat-zzz", &backlog).unwrap_err();
        assert!(
            err.to_string().contains("cannot block vat-zzz by itself"),
            "{}",
            err.to_string()
        );
    }

    // -----------------------------------------------------------------------
    // CMD-BLOCK-002 — unknown blocker
    // -----------------------------------------------------------------------

    // @spec CMD-BLOCK-002
    #[test]
    fn block_aborts_when_blocker_not_in_region() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] First\n"));

        let err = run("vat-g5y", "vat-h8x", &backlog).unwrap_err();
        assert!(
            err.to_string().contains("unknown blocker: vat-h8x"),
            "{}",
            err.to_string()
        );
    }

    // @spec CMD-BLOCK-002
    #[test]
    fn block_does_not_write_when_blocker_unknown() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y] First\n");
        write_backlog(&backlog, &original);

        let _ = run("vat-g5y", "vat-h8x", &backlog);
        assert_eq!(read_backlog(&backlog), original);
    }

    // @spec CMD-BLOCK-002a
    #[test]
    fn block_reports_parse_error_when_blocker_bullet_malformed() {
        // `- [vat-f1w]` carries the id but has no title, so it does not parse to
        // a well-formed bullet. The id is plainly present, so the user must see a
        // parse diagnostic — not a misleading "unknown blocker" — mirroring the
        // target-id handling (CMD-CC-004). It is still not written into a marker.
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y] First\n- [vat-f1w]\n");
        write_backlog(&backlog, &original);

        let err = run("vat-g5y", "vat-f1w", &backlog).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("blocker vat-f1w found but its bullet could not be parsed"),
            "{msg}"
        );
        assert!(
            !msg.contains("unknown blocker"),
            "must not report a present-but-malformed blocker as unknown: {msg}"
        );
        // No write on the error path.
        assert_eq!(read_backlog(&backlog), original);
    }

    // -----------------------------------------------------------------------
    // CMD-CC-001 — version gate
    // -----------------------------------------------------------------------

    // @spec CMD-CC-001
    #[test]
    fn block_aborts_on_unsupported_backlog_version() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let future = SUPPORTED_MAJOR + 1;
        write_backlog(
            &backlog,
            &format!("---\nversion: {future}\n---\n- [vat-g5y] First\n- [vat-f1w] Second\n"),
        );

        let err = run("vat-g5y", "vat-f1w", &backlog).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("version") || msg.contains("upgrade"),
            "expected version error: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // CMD-CC-002 — unknown target id
    // -----------------------------------------------------------------------

    // @spec CMD-CC-002
    #[test]
    fn block_aborts_on_unknown_target_id() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &two_tasks());

        let err = run("vat-x9z", "vat-f1w", &backlog).unwrap_err();
        assert!(
            err.to_string().contains("unknown id: vat-x9z"),
            "{}",
            err.to_string()
        );
    }

    // @spec CMD-CC-002
    #[test]
    fn block_does_not_write_when_target_unknown() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = two_tasks();
        write_backlog(&backlog, &original);

        let _ = run("vat-x9z", "vat-f1w", &backlog);
        assert_eq!(read_backlog(&backlog), original);
    }

    // -----------------------------------------------------------------------
    // CMD-CC-004 — malformed target bullet
    // -----------------------------------------------------------------------

    // @spec CMD-CC-004
    #[test]
    fn block_reports_parse_error_when_target_bullet_malformed() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y]\n- [vat-f1w] Second\n"),
        );

        let err = run("vat-g5y", "vat-f1w", &backlog).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("could not be parsed"), "{msg}");
        assert!(!msg.contains("unknown id"), "{msg}");
    }

    // -----------------------------------------------------------------------
    // CMD-BLOCK-005 — add marker when none present
    // -----------------------------------------------------------------------

    // @spec CMD-BLOCK-005
    #[test]
    fn block_adds_marker_in_canonical_position() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &two_tasks());

        let msg = run("vat-g5y", "vat-f1w", &backlog).unwrap();
        assert_eq!(msg, "blocked vat-g5y by vat-f1w");

        let content = read_backlog(&backlog);
        assert!(
            content.contains("- [vat-g5y] [blocked-by:vat-f1w] First\n"),
            "{content}"
        );
        // The blocker bullet itself is untouched.
        assert!(content.contains("- [vat-f1w] Second\n"), "{content}");
    }

    // @spec CMD-BLOCK-005, CMD-CC-003
    #[test]
    fn block_adds_marker_after_existing_claim_markers() {
        // Canonical order: [id] [in-progress] [by:...] [blocked-by:...] title.
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [in-progress] [by:alice] First\n- [vat-f1w] Second\n"),
        );

        run("vat-g5y", "vat-f1w", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert!(
            content.contains("- [vat-g5y] [in-progress] [by:alice] [blocked-by:vat-f1w] First\n"),
            "{content}"
        );
    }

    // @spec CMD-BLOCK-005
    #[test]
    fn block_accepts_ids_in_uppercase() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &two_tasks());

        let msg = run("VAT-G5Y", "VAT-F1W", &backlog).unwrap();
        assert_eq!(msg, "blocked vat-g5y by vat-f1w");

        let content = read_backlog(&backlog);
        assert!(
            content.contains("- [vat-g5y] [blocked-by:vat-f1w] First\n"),
            "{content}"
        );
    }

    // @spec CMD-BLOCK-005
    #[test]
    fn block_preserves_notes_under_target_entry() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] First\n  note line\n- [vat-f1w] Second\n"),
        );

        run("vat-g5y", "vat-f1w", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert!(content.contains("  note line\n"), "{content}");
    }

    // -----------------------------------------------------------------------
    // CMD-BLOCK-004 — replace an existing different blocker
    // -----------------------------------------------------------------------

    // @spec CMD-BLOCK-004
    #[test]
    fn block_replaces_existing_different_blocker() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!(
                "{HEADER}- [vat-g5y] [blocked-by:vat-k2m] First\n- [vat-f1w] Second\n- [vat-k2m] Third\n"
            ),
        );

        let msg = run("vat-g5y", "vat-f1w", &backlog).unwrap();
        assert_eq!(msg, "blocked vat-g5y by vat-f1w");

        let content = read_backlog(&backlog);
        assert!(
            content.contains("- [vat-g5y] [blocked-by:vat-f1w] First\n"),
            "{content}"
        );
        assert!(!content.contains("vat-k2m] First"), "{content}");
    }

    // -----------------------------------------------------------------------
    // CMD-BLOCK-003 — idempotent re-block by the same id is a no-op
    // -----------------------------------------------------------------------

    // @spec CMD-BLOCK-003
    #[test]
    fn block_same_blocker_is_noop_success() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [blocked-by:vat-f1w] First\n- [vat-f1w] Second\n"),
        );

        let msg = run("vat-g5y", "vat-f1w", &backlog).unwrap();
        assert_eq!(msg, "blocked vat-g5y by vat-f1w");
    }

    // @spec CMD-BLOCK-003
    #[test]
    fn block_same_blocker_does_not_write_file() {
        // No-op must not rewrite the file — verify the bytes are byte-for-byte
        // unchanged (even a normalizing rewrite would be a violation here).
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        // Deliberately non-canonical spacing the writer would otherwise normalize.
        let original =
            format!("{HEADER}- [vat-g5y]  [blocked-by:vat-f1w]  First\n- [vat-f1w] Second\n");
        write_backlog(&backlog, &original);

        run("vat-g5y", "vat-f1w", &backlog).unwrap();
        assert_eq!(read_backlog(&backlog), original);
    }

    // @spec CMD-BLOCK-003
    #[test]
    fn block_same_blocker_noop_is_case_insensitive() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original =
            format!("{HEADER}- [vat-g5y] [blocked-by:vat-f1w] First\n- [vat-f1w] Second\n");
        write_backlog(&backlog, &original);

        run("vat-g5y", "VAT-F1W", &backlog).unwrap();
        assert_eq!(read_backlog(&backlog), original);
    }

    // -----------------------------------------------------------------------
    // CMD-BLOCK-006 — no cycle detection
    // -----------------------------------------------------------------------

    // @spec CMD-BLOCK-006
    #[test]
    fn block_allows_cycles() {
        // A blocked-by B, then B blocked-by A — both succeed; v1 does not detect
        // the cycle.
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &two_tasks());

        run("vat-g5y", "vat-f1w", &backlog).unwrap();
        run("vat-f1w", "vat-g5y", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert!(
            content.contains("- [vat-g5y] [blocked-by:vat-f1w] First\n"),
            "{content}"
        );
        assert!(
            content.contains("- [vat-f1w] [blocked-by:vat-g5y] Second\n"),
            "{content}"
        );
    }

    // -----------------------------------------------------------------------
    // Preservation of unrelated regions
    // -----------------------------------------------------------------------

    // @spec CMD-BLOCK-005
    #[test]
    fn block_preserves_preamble_and_freeform_regions() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            "---\nversion: 1\n---\n# Title\n\n- [vat-g5y] First\n- [vat-f1w] Second\n---\nFreeform\n",
        );

        run("vat-g5y", "vat-f1w", &backlog).unwrap();

        let content = read_backlog(&backlog);
        assert!(content.contains("# Title\n"), "{content}");
        assert!(content.contains("---\nFreeform\n"), "{content}");
        assert!(content.contains("[blocked-by:vat-f1w]"), "{content}");
    }
}
