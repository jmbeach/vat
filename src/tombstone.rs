// @spec FMT-TOMB-001, FMT-TOMB-002, FMT-TOMB-003, FMT-TOMB-004, FMT-TOMB-005, FMT-TOMB-006, FMT-TOMB-007

#![allow(dead_code)]

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::base32::validate;

/// Each ID segment (prefix and suffix) is this many Crockford base32 chars.
const ID_SEGMENT_LEN: usize = 3;

#[derive(Debug, Error)]
pub(crate) enum TombstoneError {
    #[error("malformed line {line_no} in .used-ids: {content:?}")]
    MalformedLine { line_no: usize, content: String },
    #[error("backlog directory does not exist: {path:?}; run `vat init`")]
    NoBacklogDir { path: PathBuf },
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// A line is a well-formed ID iff it is `<3>-<3>` with both segments in the
/// Crockford base32 alphabet. We do not check the prefix against `vat.toml` —
/// cross-prefix lines are still well-formed at this layer; sync owns that check.
fn is_valid_id(s: &str) -> bool {
    match s.split_once('-') {
        Some((prefix, suffix)) => {
            validate(prefix, ID_SEGMENT_LEN).is_ok() && validate(suffix, ID_SEGMENT_LEN).is_ok()
        }
        None => false,
    }
}

/// If `path`'s parent is a named directory that does not exist, return the
/// `NoBacklogDir` error naming it. A missing file *within* an existing parent is
/// not this case — that is handled separately by each caller.
fn require_parent_dir(path: &Path) -> Result<(), TombstoneError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.is_dir()
    {
        return Err(TombstoneError::NoBacklogDir {
            path: parent.to_path_buf(),
        });
    }
    Ok(())
}

// @spec FMT-TOMB-001, FMT-TOMB-002, FMT-TOMB-003, FMT-TOMB-004, FMT-TOMB-005, FMT-TOMB-009
pub(crate) fn read(path: &Path) -> Result<HashSet<String>, TombstoneError> {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // A missing parent dir is a config bug (NoBacklogDir); a missing file
            // within an existing backlog/ is simply empty.
            require_parent_dir(path)?;
            return Ok(HashSet::new());
        }
        Err(e) => return Err(TombstoneError::Io(e)),
    };

    let mut ids = HashSet::new();
    // `str::lines()` does not yield a trailing empty element for a final newline,
    // so the always-present trailing `\n` that `append` writes is not a blank line.
    for (idx, raw) in contents.lines().enumerate() {
        let trimmed = raw.trim();
        if !is_valid_id(trimmed) {
            return Err(TombstoneError::MalformedLine {
                line_no: idx + 1,
                content: trimmed.to_string(),
            });
        }
        ids.insert(trimmed.to_ascii_lowercase());
    }
    Ok(ids)
}

// @spec FMT-TOMB-001, FMT-TOMB-002, FMT-TOMB-006, FMT-TOMB-007, FMT-TOMB-008
pub(crate) fn append(path: &Path, new_ids: &[&str]) -> Result<(), TombstoneError> {
    // Empty batch is a no-op: do not create or touch the file.
    if new_ids.is_empty() {
        return Ok(());
    }
    // The writer never creates backlog/ — that is vat init's job.
    require_parent_dir(path)?;

    let mut file = OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(path)?;

    // Defensive leading newline: if the existing file is non-empty and its last
    // byte isn't `\n`, a truncated hand-edit would otherwise fuse onto the tail.
    let needs_leading_newline = {
        let len = file.metadata()?.len();
        if len == 0 {
            false
        } else {
            let mut last = [0u8; 1];
            file.seek(SeekFrom::End(-1))?;
            file.read_exact(&mut last)?;
            last[0] != b'\n'
        }
    };

    let mut out = String::new();
    if needs_leading_newline {
        out.push('\n');
    }
    for id in new_ids {
        out.push_str(id);
        out.push('\n');
    }
    // O_APPEND directs the write to end-of-file regardless of the read cursor above.
    file.write_all(out.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{TombstoneError, append, read};
    use std::collections::HashSet;
    use std::fs;
    use tempfile::TempDir;

    fn project_dir() -> TempDir {
        let dir = TempDir::new().expect("create tempdir");
        fs::create_dir(dir.path().join("backlog")).expect("create backlog/");
        dir
    }

    fn used_ids_path(dir: &TempDir) -> std::path::PathBuf {
        dir.path().join("backlog/.used-ids")
    }

    fn set_of(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    // ---------- read ----------

    // @spec FMT-TOMB-002
    #[test]
    fn read_missing_file_returns_empty_set() {
        let dir = project_dir();
        let got = read(&used_ids_path(&dir)).expect("missing file should be empty, not error");
        assert!(got.is_empty());
    }

    // @spec FMT-TOMB-001
    #[test]
    fn read_parses_newline_delimited_ids() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\nbar-9p3\nbar-abc\n").unwrap();
        let got = read(&used_ids_path(&dir)).unwrap();
        assert_eq!(got, set_of(&["bar-7k2", "bar-9p3", "bar-abc"]));
    }

    // @spec FMT-TOMB-001
    #[test]
    fn read_handles_file_without_trailing_newline() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\nbar-9p3").unwrap();
        let got = read(&used_ids_path(&dir)).unwrap();
        assert_eq!(got, set_of(&["bar-7k2", "bar-9p3"]));
    }

    // @spec FMT-TOMB-003
    #[test]
    fn read_deduplicates_repeated_ids() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\nbar-9p3\nbar-7k2\n").unwrap();
        let got = read(&used_ids_path(&dir)).unwrap();
        assert_eq!(got, set_of(&["bar-7k2", "bar-9p3"]));
    }

    // @spec FMT-TOMB-005
    #[test]
    fn read_lowercases_uppercase_ids() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "BAR-7K2\nBaR-9P3\n").unwrap();
        let got = read(&used_ids_path(&dir)).unwrap();
        assert_eq!(got, set_of(&["bar-7k2", "bar-9p3"]));
    }

    // @spec FMT-TOMB-003, FMT-TOMB-005
    #[test]
    fn read_dedups_after_case_normalization() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\nBAR-7K2\n").unwrap();
        let got = read(&used_ids_path(&dir)).unwrap();
        assert_eq!(got, set_of(&["bar-7k2"]));
    }

    // @spec FMT-TOMB-004
    #[test]
    fn read_trims_surrounding_whitespace() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "  bar-7k2  \n\tbar-9p3\t\n").unwrap();
        let got = read(&used_ids_path(&dir)).unwrap();
        assert_eq!(got, set_of(&["bar-7k2", "bar-9p3"]));
    }

    // A blank line trims to empty content; the reported content is the trimmed text.
    // @spec FMT-TOMB-004
    #[test]
    fn read_rejects_blank_line_with_line_number() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\n\nbar-9p3\n").unwrap();
        match read(&used_ids_path(&dir)) {
            Err(TombstoneError::MalformedLine { line_no, content }) => {
                assert_eq!(line_no, 2);
                assert_eq!(content, "");
            }
            other => panic!("expected MalformedLine on line 2, got {other:?}"),
        }
    }

    // @spec FMT-TOMB-004
    #[test]
    fn read_rejects_whitespace_only_line_with_line_number() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\n   \nbar-9p3\n").unwrap();
        match read(&used_ids_path(&dir)) {
            Err(TombstoneError::MalformedLine { line_no, content }) => {
                assert_eq!(line_no, 2);
                assert_eq!(content, "", "content should be the trimmed (empty) text");
            }
            other => panic!("expected MalformedLine on line 2, got {other:?}"),
        }
    }

    // @spec FMT-TOMB-004
    #[test]
    fn read_rejects_wrong_shape_with_line_number() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\nbar7k2\nbar-9p3\n").unwrap();
        match read(&used_ids_path(&dir)) {
            Err(TombstoneError::MalformedLine { line_no, content }) => {
                assert_eq!(line_no, 2);
                assert_eq!(content, "bar7k2");
            }
            other => panic!("expected MalformedLine on line 2, got {other:?}"),
        }
    }

    // @spec FMT-TOMB-004
    #[test]
    fn read_rejects_invalid_base32_char_with_line_number() {
        let dir = project_dir();
        // 'l' is excluded from Crockford base32.
        fs::write(used_ids_path(&dir), "bar-7k2\nbar-lll\n").unwrap();
        match read(&used_ids_path(&dir)) {
            Err(TombstoneError::MalformedLine { line_no, content }) => {
                assert_eq!(line_no, 2);
                assert_eq!(content, "bar-lll");
            }
            other => panic!("expected MalformedLine on line 2, got {other:?}"),
        }
    }

    // The reported content is the trimmed text, not the raw line.
    // @spec FMT-TOMB-004
    #[test]
    fn read_rejects_wrong_length_segment_with_line_number() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\n  fo-9p3  \n").unwrap();
        match read(&used_ids_path(&dir)) {
            Err(TombstoneError::MalformedLine { line_no, content }) => {
                assert_eq!(line_no, 2);
                assert_eq!(content, "fo-9p3", "content should be trimmed");
            }
            other => panic!("expected MalformedLine on line 2, got {other:?}"),
        }
    }

    // A single trailing newline terminates the last ID; the empty slice after it
    // must not be treated as a blank (malformed) line. `append` always writes a
    // trailing newline, so every file `read` consumes has exactly this shape.
    // @spec FMT-TOMB-004
    #[test]
    fn read_trailing_newline_is_not_a_blank_line() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\nbar-9p3\n").unwrap();
        let got = read(&used_ids_path(&dir)).unwrap();
        assert_eq!(got, set_of(&["bar-7k2", "bar-9p3"]));
    }

    // @spec FMT-TOMB-009
    #[test]
    fn read_returns_no_backlog_dir_when_parent_missing() {
        let dir = TempDir::new().unwrap(); // no backlog/ subdir
        let path = dir.path().join("backlog/.used-ids");
        match read(&path) {
            Err(TombstoneError::NoBacklogDir { path: reported }) => {
                assert_eq!(reported, dir.path().join("backlog"));
            }
            other => panic!("expected NoBacklogDir, got {other:?}"),
        }
    }

    // ---------- append ----------

    // @spec FMT-TOMB-002
    #[test]
    fn append_creates_file_when_missing() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        assert!(!path.exists());
        append(&path, &["bar-7k2"]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "bar-7k2\n");
    }

    // @spec FMT-TOMB-001, FMT-TOMB-008
    #[test]
    fn append_writes_one_line_per_id_in_input_order() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        append(&path, &["bar-7k2", "bar-9p3", "bar-abc"]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "bar-7k2\nbar-9p3\nbar-abc\n");
    }

    // @spec FMT-TOMB-001
    #[test]
    fn append_to_existing_file_adds_after_existing_content() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        fs::write(&path, "bar-7k2\n").unwrap();
        append(&path, &["bar-9p3"]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "bar-7k2\nbar-9p3\n");
    }

    // Blind append: writer does not dedup against existing or within the batch.
    // @spec FMT-TOMB-008
    #[test]
    fn append_does_not_dedup_against_existing_file() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        fs::write(&path, "bar-7k2\n").unwrap();
        append(&path, &["bar-7k2"]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "bar-7k2\nbar-7k2\n");
    }

    // @spec FMT-TOMB-008
    #[test]
    fn append_does_not_dedup_within_batch() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        append(&path, &["bar-7k2", "bar-7k2"]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "bar-7k2\nbar-7k2\n");
    }

    // @spec FMT-TOMB-006
    #[test]
    fn append_prepends_newline_when_existing_file_lacks_trailing_newline() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        fs::write(&path, "bar-7k2").unwrap(); // no trailing \n
        append(&path, &["bar-9p3"]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "bar-7k2\nbar-9p3\n");
    }

    // The defensive leading-\n composes with one \n per id across a multi-id batch:
    // exactly one separator goes in, then each id is newline-terminated.
    // @spec FMT-TOMB-006, FMT-TOMB-008
    #[test]
    fn append_multi_id_batch_into_file_without_trailing_newline() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        fs::write(&path, "bar-7k2").unwrap(); // no trailing \n
        append(&path, &["bar-9p3", "bar-abc"]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "bar-7k2\nbar-9p3\nbar-abc\n");
    }

    // @spec FMT-TOMB-006
    #[test]
    fn append_does_not_double_newline_when_existing_file_ends_with_newline() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        fs::write(&path, "bar-7k2\n").unwrap();
        append(&path, &["bar-9p3"]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "bar-7k2\nbar-9p3\n");
    }

    // @spec FMT-TOMB-006
    #[test]
    fn append_to_empty_existing_file_does_not_prepend_newline() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        fs::write(&path, "").unwrap();
        append(&path, &["bar-7k2"]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "bar-7k2\n");
    }

    // Empty input batch is a no-op; should not create the file.
    // @spec FMT-TOMB-008
    #[test]
    fn append_empty_batch_to_missing_file_is_noop() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        append(&path, &[]).unwrap();
        assert!(!path.exists(), "empty append should not create the file");
    }

    // @spec FMT-TOMB-007
    #[test]
    fn append_returns_no_backlog_dir_when_parent_missing() {
        let dir = TempDir::new().unwrap(); // no backlog/ subdir
        let path = dir.path().join("backlog/.used-ids");
        match append(&path, &["bar-7k2"]) {
            Err(TombstoneError::NoBacklogDir { path: reported }) => {
                assert_eq!(reported, dir.path().join("backlog"));
            }
            other => panic!("expected NoBacklogDir, got {other:?}"),
        }
        assert!(
            !dir.path().join("backlog").exists(),
            "append must not create the backlog directory"
        );
    }

    // Round-trip: write then read returns the input as a set (post-normalization).
    // @spec FMT-TOMB-001, FMT-TOMB-002, FMT-TOMB-003
    #[test]
    fn round_trip_write_then_read() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        append(&path, &["bar-7k2", "bar-9p3"]).unwrap();
        append(&path, &["bar-abc"]).unwrap();
        let got = read(&path).unwrap();
        assert_eq!(got, set_of(&["bar-7k2", "bar-9p3", "bar-abc"]));
    }
}
