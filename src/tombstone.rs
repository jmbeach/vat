// @spec FMT-TOMB-001, FMT-TOMB-002, FMT-TOMB-003, FMT-TOMB-004, FMT-TOMB-005, FMT-TOMB-006, FMT-TOMB-007, FMT-TOMB-008, FMT-TOMB-009

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
    #[error("expected a backlog directory but {path:?} is not a directory")]
    BacklogNotDirectory { path: PathBuf },
    #[error(transparent)]
    Io(#[from] io::Error),
}

// `io::Error` is not `PartialEq`, so we can't derive it the way `Base32Error` and
// `ConfigError` do. Compare the structured variants by value and `Io` by kind,
// which is enough for `assert_eq!`-style assertions in tests.
impl PartialEq for TombstoneError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::MalformedLine {
                    line_no: a_no,
                    content: a_content,
                },
                Self::MalformedLine {
                    line_no: b_no,
                    content: b_content,
                },
            ) => a_no == b_no && a_content == b_content,
            (Self::NoBacklogDir { path: a }, Self::NoBacklogDir { path: b })
            | (Self::BacklogNotDirectory { path: a }, Self::BacklogNotDirectory { path: b }) => {
                a == b
            }
            (Self::Io(a), Self::Io(b)) => a.kind() == b.kind(),
            _ => false,
        }
    }
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

/// If `path`'s named parent directory is missing, return `NoBacklogDir`; if it
/// exists but is not a directory, return `BacklogNotDirectory`. A missing file
/// *within* an existing parent is neither case — that is handled by each caller.
fn require_parent_dir(path: &Path) -> Result<(), TombstoneError> {
    // A bare filename (e.g. `.used-ids`) has parent `Some("")`; the empty-parent
    // guard treats that as "current directory", which always exists, so we do not
    // raise a missing-directory error for it.
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.is_dir()
    {
        // `is_dir()` is false both when `parent` is missing and when it exists as a
        // non-directory (e.g. a regular file named `backlog`). Distinguish the two
        // so the error tells the user which situation they are actually in.
        return Err(if parent.exists() {
            TombstoneError::BacklogNotDirectory {
                path: parent.to_path_buf(),
            }
        } else {
            TombstoneError::NoBacklogDir {
                path: parent.to_path_buf(),
            }
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

// @spec FMT-TOMB-001, FMT-TOMB-002, FMT-TOMB-006, FMT-TOMB-007, FMT-TOMB-008, FMT-TOMB-009
pub(crate) fn append(path: &Path, new_ids: &[&str]) -> Result<(), TombstoneError> {
    // The writer never creates backlog/ — that is vat init's job. Checked before the
    // empty-batch shortcut so a zero-ID append can't be a silent "is this project
    // valid?" probe that succeeds against a missing project directory.
    require_parent_dir(path)?;

    // Empty batch is a no-op: do not create or touch the file.
    if new_ids.is_empty() {
        return Ok(());
    }

    // Validate before opening. The reader is strict, so writing an unvalidated ID
    // (a bare word, or one containing an embedded `\n`) would permanently corrupt
    // the tombstone — the next `read` would hard-error and need manual repair. We
    // bail before creating or touching the file, mirroring the reader's `<3>-<3>`
    // shape check, and report the 1-based position within the batch.
    for (idx, id) in new_ids.iter().enumerate() {
        if !is_valid_id(id) {
            return Err(TombstoneError::MalformedLine {
                line_no: idx + 1,
                content: (*id).to_string(),
            });
        }
    }

    let mut file = OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(path)?;

    // Defensive leading newline: if the existing file is non-empty and its last
    // byte isn't `\n`, a truncated hand-edit would otherwise fuse onto the tail.
    // Concurrent appends can still fuse IDs through this check — see the
    // concurrency note in docs/llds/backlog-format.md (out of scope for v1).
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
    use super::{TombstoneError, append, read, require_parent_dir};
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;
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

    // An empty-but-existing file branches differently from a missing file (it enters
    // the parse loop on `""` rather than hitting the NotFound arm); pin both paths.
    // @spec FMT-TOMB-002
    #[test]
    fn read_empty_existing_file_returns_empty_set() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "").unwrap();
        let got = read(&used_ids_path(&dir)).expect("empty file should be empty, not error");
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
        assert_eq!(
            read(&used_ids_path(&dir)).unwrap_err(),
            TombstoneError::MalformedLine {
                line_no: 2,
                content: String::new(),
            }
        );
    }

    // @spec FMT-TOMB-004
    #[test]
    fn read_rejects_whitespace_only_line_with_line_number() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\n   \nbar-9p3\n").unwrap();
        // Content is reported as the trimmed (empty) text, not the raw whitespace.
        assert_eq!(
            read(&used_ids_path(&dir)).unwrap_err(),
            TombstoneError::MalformedLine {
                line_no: 2,
                content: String::new(),
            }
        );
    }

    // @spec FMT-TOMB-004
    #[test]
    fn read_rejects_wrong_shape_with_line_number() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\nbar7k2\nbar-9p3\n").unwrap();
        assert_eq!(
            read(&used_ids_path(&dir)).unwrap_err(),
            TombstoneError::MalformedLine {
                line_no: 2,
                content: "bar7k2".to_string(),
            }
        );
    }

    // @spec FMT-TOMB-004
    #[test]
    fn read_rejects_invalid_base32_char_with_line_number() {
        let dir = project_dir();
        // 'l' is excluded from Crockford base32.
        fs::write(used_ids_path(&dir), "bar-7k2\nbar-lll\n").unwrap();
        assert_eq!(
            read(&used_ids_path(&dir)).unwrap_err(),
            TombstoneError::MalformedLine {
                line_no: 2,
                content: "bar-lll".to_string(),
            }
        );
    }

    // The reported content is the trimmed text, not the raw line.
    // @spec FMT-TOMB-004
    #[test]
    fn read_rejects_wrong_length_segment_with_line_number() {
        let dir = project_dir();
        fs::write(used_ids_path(&dir), "bar-7k2\n  fo-9p3  \n").unwrap();
        // Content is reported trimmed, not as the raw padded line.
        assert_eq!(
            read(&used_ids_path(&dir)).unwrap_err(),
            TombstoneError::MalformedLine {
                line_no: 2,
                content: "fo-9p3".to_string(),
            }
        );
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
        assert_eq!(
            read(&path).unwrap_err(),
            TombstoneError::NoBacklogDir {
                path: dir.path().join("backlog"),
            }
        );
    }

    // The empty-parent guard in `require_parent_dir` must not flag a bare filename
    // (whose parent is `Some("")`) as a missing backlog directory.
    #[test]
    fn path_with_no_directory_component_does_not_trigger_no_backlog_dir() {
        assert!(require_parent_dir(Path::new(".used-ids")).is_ok());
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
        assert_eq!(
            append(&path, &["bar-7k2"]).unwrap_err(),
            TombstoneError::NoBacklogDir {
                path: dir.path().join("backlog"),
            }
        );
        assert!(
            !dir.path().join("backlog").exists(),
            "append must not create the backlog directory"
        );
    }

    // The parent-dir check runs before the empty-batch shortcut, so a zero-ID append
    // against a missing project directory still fails loudly rather than silently
    // succeeding (which a caller could misread as "this project path is valid").
    // @spec FMT-TOMB-007, FMT-TOMB-008
    #[test]
    fn append_empty_batch_returns_no_backlog_dir_when_parent_missing() {
        let dir = TempDir::new().unwrap(); // no backlog/ subdir
        let path = dir.path().join("backlog/.used-ids");
        assert_eq!(
            append(&path, &[]).unwrap_err(),
            TombstoneError::NoBacklogDir {
                path: dir.path().join("backlog"),
            }
        );
    }

    // When `backlog` exists as a regular file (not a directory), the error must say
    // so rather than claiming the directory is missing.
    // @spec FMT-TOMB-007
    #[test]
    fn append_reports_backlog_not_directory_when_backlog_is_a_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("backlog"), "i am a file, not a dir").unwrap();
        let path = dir.path().join("backlog/.used-ids");
        assert_eq!(
            append(&path, &["bar-7k2"]).unwrap_err(),
            TombstoneError::BacklogNotDirectory {
                path: dir.path().join("backlog"),
            }
        );
    }

    // Symmetry with the reader: a write of a malformed ID would produce a tombstone
    // the reader then hard-errors on, so the writer rejects it up front.
    // @spec FMT-TOMB-008
    #[test]
    fn append_rejects_malformed_id() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        assert_eq!(
            append(&path, &["bar-7k2", "not-an-id-shape"]).unwrap_err(),
            TombstoneError::MalformedLine {
                line_no: 2,
                content: "not-an-id-shape".to_string(),
            }
        );
        assert!(
            !path.exists(),
            "a rejected batch must not create or modify the tombstone"
        );
    }

    // An embedded newline would split one element across two lines, the second of
    // which is malformed. Rejecting it up front prevents permanent corruption.
    // @spec FMT-TOMB-008
    #[test]
    fn append_rejects_id_with_embedded_newline() {
        let dir = project_dir();
        let path = used_ids_path(&dir);
        assert_eq!(
            append(&path, &["foo\nbar-7k2"]).unwrap_err(),
            TombstoneError::MalformedLine {
                line_no: 1,
                content: "foo\nbar-7k2".to_string(),
            }
        );
        assert!(!path.exists(), "a rejected batch must not create the file");
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
