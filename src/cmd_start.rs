// @spec CMD-START-001, CMD-START-002, CMD-START-003, CMD-CC-001, CMD-CC-002, CMD-CC-003

use std::path::Path;

use anyhow::Context;

use crate::backlog_file::{BacklogFile, ParsedRegion, check_version};
use crate::bullet::Bullet;
use crate::errors::UserError;
use crate::file_io;
use crate::user_config;

/// Mark `id` as in-progress, claimed by the configured user.
// @spec CMD-START-001, CMD-START-002, CMD-START-003
pub(crate) fn run(id: &str, backlog_dir: &Path) -> anyhow::Result<()> {
    let user_cfg_path = user_config::config_path()?;
    run_impl(id, backlog_dir, &user_cfg_path)
}

// Testable inner implementation with an explicit user config path.
fn run_impl(id: &str, backlog_dir: &Path, user_cfg_path: &Path) -> anyhow::Result<()> {
    // CMD-START-001: resolve user.name; abort if unset.
    let user_name = resolve_user_name(user_cfg_path)?;

    let backlog_path = backlog_dir.join("backlog.md");
    let input = file_io::read_to_string(&backlog_path)
        .with_context(|| format!("reading {}", backlog_path.display()))?;

    let bf = BacklogFile::parse(&input);

    // CMD-CC-001: version gate before any other processing.
    check_version(bf.frontmatter()).context("backlog version check")?;

    let id_lower = id.to_lowercase();
    let region = ParsedRegion::parse(bf.parsed());

    // CMD-CC-002: locate the matching entry; abort without writes if not found.
    let entry_idx = find_entry_index(&region, &id_lower)
        .ok_or_else(|| UserError(format!("unknown id: {id}")))?;

    let mut bullet = Bullet::parse(region.entries[entry_idx].bullet_line)
        .expect("entry bullet must parse: it was just located by ID");

    // CMD-START-002: refuse on partial-or-full claim.
    if bullet.in_progress || bullet.by.is_some() {
        let msg = match bullet.by.as_deref() {
            Some(name) => format!("{id} already claimed by {name}"),
            None => format!("{id} already in progress"),
        };
        return Err(UserError(msg).into());
    }

    // CMD-START-003 + CMD-CC-003: add both markers. Bullet::serialize emits them in
    // canonical order ([id] → [in-progress] → [by:...] → [blocked-by:...] → title).
    bullet.in_progress = true;
    bullet.by = Some(user_name);
    let new_bullet_line = bullet.serialize();

    let new_parsed = serialize_region_with_replaced_bullet(&region, entry_idx, &new_bullet_line);
    let output = bf.serialize(&new_parsed);
    file_io::write(&backlog_path, &output)
        .with_context(|| format!("writing {}", backlog_path.display()))?;

    Ok(())
}

/// Find the index of the entry in `region` whose parsed bullet carries the given
/// (already-lowercased) `id`.
///
/// This is the lookup half of the `find_entry` helper described in the commands LLD:
/// "All bullet-mutating commands share a helper: `fn find_entry(id) -> (parsed_region,
/// entry_index)`". The parsed-region half lives at the call site (which holds the
/// borrowed string); this function handles only the index-location part.
// @spec CMD-CC-002
pub(crate) fn find_entry_index(region: &ParsedRegion<'_>, id_lower: &str) -> Option<usize> {
    region.entries.iter().position(|e| {
        Bullet::parse(e.bullet_line)
            .ok()
            .and_then(|b| b.id)
            .as_deref()
            == Some(id_lower)
    })
}

// @spec CMD-START-001
fn resolve_user_name(user_cfg_path: &Path) -> anyhow::Result<String> {
    use crate::user_config::UserConfigError;

    let cfg = match user_config::load(user_cfg_path) {
        Ok(cfg) => cfg,
        Err(UserConfigError::NotFound(_)) => return Err(no_user_name_error()),
        Err(e) => return Err(anyhow::Error::from(e).context("reading user config")),
    };

    cfg.user_name()
        .map(ToOwned::to_owned)
        .ok_or_else(no_user_name_error)
}

fn no_user_name_error() -> anyhow::Error {
    UserError("set user.name first: vat config set user.name <name>".to_owned()).into()
}

/// Serialize `region` with entry `entry_idx`'s bullet line replaced by `new_bullet_line`.
/// Notes for every entry are preserved verbatim; the preamble is preserved verbatim.
fn serialize_region_with_replaced_bullet(
    region: &ParsedRegion<'_>,
    entry_idx: usize,
    new_bullet_line: &str,
) -> String {
    let mut out = String::new();
    out.push_str(region.preamble);
    for (i, entry) in region.entries.iter().enumerate() {
        if i == entry_idx {
            out.push_str(new_bullet_line);
        } else {
            out.push_str(entry.bullet_line);
        }
        out.push_str(entry.notes);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::run_impl;
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

    fn write_user_config(dir: &TempDir, name: &str) -> PathBuf {
        let path = dir.path().join("user_config.toml");
        fs::write(&path, format!("[user]\nname = \"{name}\"\n")).unwrap();
        path
    }

    fn read_backlog(backlog: &std::path::Path) -> String {
        fs::read_to_string(backlog.join("backlog.md")).unwrap()
    }

    const HEADER: &str = "---\nversion: 1\n---\n\n";

    // -----------------------------------------------------------------------
    // CMD-START-001 — user.name precondition
    // -----------------------------------------------------------------------

    // @spec CMD-START-001
    #[test]
    fn start_aborts_when_user_config_is_missing() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));
        let missing = dir.path().join("nonexistent.toml");

        let err = run_impl("vat-g5y", &backlog, &missing).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("vat config set user.name"),
            "expected hint in error: {msg}"
        );
    }

    // @spec CMD-START-001
    #[test]
    fn start_aborts_when_user_name_key_is_absent() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));
        let cfg = dir.path().join("user_config.toml");
        fs::write(&cfg, "[user]\n").unwrap(); // [user] table present, name key absent

        let err = run_impl("vat-g5y", &backlog, &cfg).unwrap_err();
        assert!(
            err.to_string().contains("vat config set user.name"),
            "{err}"
        );
    }

    // -----------------------------------------------------------------------
    // CMD-CC-001 — version gate
    // -----------------------------------------------------------------------

    // @spec CMD-CC-001
    #[test]
    fn start_aborts_on_unsupported_backlog_version() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let future = SUPPORTED_MAJOR + 1;
        write_backlog(
            &backlog,
            &format!("---\nversion: {future}\n---\n- [vat-g5y] A task\n"),
        );
        let cfg = write_user_config(&dir, "alice");

        let err = run_impl("vat-g5y", &backlog, &cfg).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("version") || msg.contains("upgrade"),
            "expected version error: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // CMD-CC-002 — unknown ID
    // -----------------------------------------------------------------------

    // @spec CMD-CC-002
    #[test]
    fn start_aborts_on_unknown_id() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));
        let cfg = write_user_config(&dir, "alice");

        let err = run_impl("vat-x9z", &backlog, &cfg).unwrap_err();
        assert!(
            err.to_string().contains("unknown id"),
            "{}",
            err.to_string()
        );
    }

    // @spec CMD-CC-002
    #[test]
    fn start_does_not_write_file_when_id_is_unknown() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y] A task\n");
        write_backlog(&backlog, &original);
        let cfg = write_user_config(&dir, "alice");

        let _ = run_impl("vat-x9z", &backlog, &cfg);
        assert_eq!(read_backlog(&backlog), original);
    }

    // -----------------------------------------------------------------------
    // CMD-START-002 — refuse on partial or full claim
    // -----------------------------------------------------------------------

    // @spec CMD-START-002
    #[test]
    fn start_aborts_when_in_progress_only() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [in-progress] A task\n"),
        );
        let cfg = write_user_config(&dir, "alice");

        let err = run_impl("vat-g5y", &backlog, &cfg).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("already in progress") || msg.contains("already claimed"),
            "{msg}"
        );
    }

    // @spec CMD-START-002
    #[test]
    fn start_aborts_when_by_marker_present_and_names_claimer() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [by:bob] A task\n"),
        );
        let cfg = write_user_config(&dir, "alice");

        let err = run_impl("vat-g5y", &backlog, &cfg).unwrap_err();
        assert!(
            err.to_string().contains("already claimed by bob"),
            "{}",
            err.to_string()
        );
    }

    // @spec CMD-START-002
    #[test]
    fn start_aborts_when_fully_claimed_and_names_claimer() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [in-progress] [by:bob] A task\n"),
        );
        let cfg = write_user_config(&dir, "alice");

        let err = run_impl("vat-g5y", &backlog, &cfg).unwrap_err();
        assert!(
            err.to_string().contains("already claimed by bob"),
            "{}",
            err.to_string()
        );
    }

    // @spec CMD-START-002
    #[test]
    fn start_does_not_write_file_when_already_claimed() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let original = format!("{HEADER}- [vat-g5y] [in-progress] [by:bob] A task\n");
        write_backlog(&backlog, &original);
        let cfg = write_user_config(&dir, "alice");

        let _ = run_impl("vat-g5y", &backlog, &cfg);
        assert_eq!(read_backlog(&backlog), original);
    }

    // -----------------------------------------------------------------------
    // CMD-START-003 — success: adds both markers in canonical order
    // -----------------------------------------------------------------------

    // @spec CMD-START-003
    #[test]
    fn start_adds_in_progress_and_by_in_canonical_order() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));
        let cfg = write_user_config(&dir, "alice");

        run_impl("vat-g5y", &backlog, &cfg).unwrap();

        let content = read_backlog(&backlog);
        assert!(
            content.contains("- [vat-g5y] [in-progress] [by:alice] A task\n"),
            "expected canonical marker order in: {content}"
        );
    }

    // @spec CMD-START-003
    #[test]
    fn start_accepts_id_in_uppercase() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(&backlog, &format!("{HEADER}- [vat-g5y] A task\n"));
        let cfg = write_user_config(&dir, "alice");

        run_impl("VAT-G5Y", &backlog, &cfg).unwrap();

        let content = read_backlog(&backlog);
        assert!(content.contains("[in-progress]"), "{content}");
        assert!(content.contains("[by:alice]"), "{content}");
    }

    // @spec CMD-START-003
    #[test]
    fn start_preserves_other_bullets_unchanged() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] First\n- [vat-h8x] Second\n"),
        );
        let cfg = write_user_config(&dir, "alice");

        run_impl("vat-g5y", &backlog, &cfg).unwrap();

        let content = read_backlog(&backlog);
        assert!(content.contains("- [vat-h8x] Second\n"), "{content}");
    }

    // @spec CMD-START-003
    #[test]
    fn start_preserves_notes_under_claimed_entry() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] A task\n  note line\n  more notes\n"),
        );
        let cfg = write_user_config(&dir, "alice");

        run_impl("vat-g5y", &backlog, &cfg).unwrap();

        let content = read_backlog(&backlog);
        assert!(content.contains("  note line\n  more notes\n"), "{content}");
    }

    // @spec CMD-START-003
    #[test]
    fn start_preserves_existing_blocked_by_marker() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            &format!("{HEADER}- [vat-g5y] [blocked-by:vat-f1w] A task\n"),
        );
        let cfg = write_user_config(&dir, "alice");

        run_impl("vat-g5y", &backlog, &cfg).unwrap();

        let content = read_backlog(&backlog);
        // Canonical: [id] [in-progress] [by:...] [blocked-by:...]
        assert!(
            content.contains(
                "- [vat-g5y] [in-progress] [by:alice] [blocked-by:vat-f1w] A task\n"
            ),
            "{content}"
        );
    }

    // @spec CMD-START-003
    #[test]
    fn start_preserves_preamble_and_freeform_regions() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog(
            &backlog,
            "---\nversion: 1\n---\n# Title\n\n- [vat-g5y] A task\n---\nFreeform\n",
        );
        let cfg = write_user_config(&dir, "alice");

        run_impl("vat-g5y", &backlog, &cfg).unwrap();

        let content = read_backlog(&backlog);
        assert!(content.contains("# Title\n"), "{content}");
        assert!(content.contains("---\nFreeform\n"), "{content}");
        assert!(content.contains("[in-progress]"), "{content}");
    }
}
