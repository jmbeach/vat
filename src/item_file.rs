// @spec FMT-ITEM-001, FMT-ITEM-002, FMT-ITEM-003, SYNC-NOTES-004, FMT-WS-001

// Functions here are wired into `vat sync` by vat-t1h; until then they are
// unreferenced from `main`. Matches the module-wide allow used by the sibling
// `backlog_file` and `project_config` modules.
#![allow(dead_code)]

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::Utf8Error;

use thiserror::Error;

use crate::{backlog_file, file_io};

#[derive(Debug, Error)]
pub(crate) enum ItemFileError {
    #[error("io error on {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("item file already exists: {0}")]
    AlreadyExists(PathBuf),
    #[error("item file is missing frontmatter")]
    MissingFrontmatter,
    #[error("item file contains invalid utf-8: {0}")]
    Utf8(#[from] Utf8Error),
}

fn io_err(path: &Path, source: io::Error) -> ItemFileError {
    ItemFileError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[derive(Debug)]
pub(crate) struct ItemFile {
    id: String,
    body: String,
}

impl ItemFile {
    /// The note body of the item file, **including the leading blank line** that
    /// separates the frontmatter from the content: a well-formed item file's
    /// body begins with `\n`. Callers that want only the note text should
    /// `trim_start_matches('\n')` or run it through [`strip_notes`].
    pub(crate) fn body(&self) -> &str {
        &self.body
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }
}

// @spec FMT-ITEM-003, FMT-WS-001
pub(crate) fn read(path: &Path) -> Result<ItemFile, ItemFileError> {
    // `read_split` validates the frontmatter block is present but does not
    // inspect its contents — the filename stem is the id (FMT-ITEM-003).
    let (_frontmatter, body) = read_split(path)?;
    let id = filename_stem(path);
    Ok(ItemFile { id, body })
}

/// Create a new item file at `path` with frontmatter `id: <id>` and the stripped
/// `body`. Fails with [`ItemFileError::AlreadyExists`] rather than clobbering an
/// existing file: FMT-ITEM-001 is a *create*, and the sync caller routes
/// existing files through [`append_notes`]. `create_new` makes the
/// existence check and the write a single atomic operation (no TOCTOU window).
// @spec FMT-ITEM-001, FMT-WS-001
pub(crate) fn write_new(path: &Path, id: &str, body: &str) -> Result<(), ItemFileError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    let stripped = strip_notes(body);
    let contents = format!("---\nid: {id}\n---\n\n{stripped}\n");
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ItemFileError::AlreadyExists(path.to_path_buf()));
        }
        Err(e) => return Err(io_err(path, e)),
    };
    file.write_all(contents.as_bytes())
        .map_err(|e| io_err(path, e))
}

/// Append `new_notes` to an existing item file, preserving its frontmatter.
///
/// The new notes are stripped ([`strip_notes`]) and joined to the existing body
/// with exactly one blank line, so repeated appends produce a stable,
/// idempotent shape.
///
/// Edge case (Q12 contract): if `new_notes` is empty after stripping, the file
/// simply gains a trailing blank line — the body is `…existing\n\n`. `vat sync`
/// never calls this with empty notes (SYNC-NOTES-005 guards that upstream); the
/// guard belongs to the caller, not this primitive.
///
/// The write is atomic (stage-then-rename), so an interrupted append never
/// truncates the accumulated notes.
// @spec FMT-ITEM-002, FMT-WS-001
pub(crate) fn append_notes(path: &Path, new_notes: &str) -> Result<(), ItemFileError> {
    let (frontmatter, existing_body) = read_split(path)?;
    // Collapse the existing body's trailing newlines to nothing, then re-join with a
    // single blank line so repeated appends produce a predictable shape (Q3A).
    let trimmed_body = existing_body.trim_end_matches('\n');
    let stripped_new = strip_notes(new_notes);
    let contents = format!("{frontmatter}{trimmed_body}\n\n{stripped_new}\n");
    write_atomic(path, &contents)
}

// @spec SYNC-NOTES-004, FMT-WS-001
pub(crate) fn strip_notes(notes: &str) -> String {
    // Normalize line endings up front so the byte-prefix dedent and the joined
    // output never carry a stray `\r` into a written file (FMT-WS-001).
    let notes = normalize_crlf(notes);
    let lines: Vec<&str> = notes.split('\n').collect();

    // Trim leading and trailing blank lines.
    let is_blank = |l: &str| l.trim().is_empty();
    let Some(start) = lines.iter().position(|l| !is_blank(l)) else {
        return String::new();
    };
    let end = lines
        .iter()
        .rposition(|l| !is_blank(l))
        .expect("a non-blank line exists because `start` was found")
        + 1;
    let kept = &lines[start..end];

    // Longest common leading-whitespace byte prefix across the non-blank lines.
    // Interior blank lines are excluded from the computation and emitted as empty.
    let prefix_len = common_leading_ws_prefix_len(kept.iter().copied().filter(|l| !is_blank(l)));

    kept.iter()
        .map(|l| if is_blank(l) { "" } else { &l[prefix_len..] })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read a VAT item file, normalize CRLF→LF, and split it into the serialized
/// frontmatter block and the body. Errors if no frontmatter block is present.
// @spec FMT-WS-001, FMT-ITEM-002
fn read_split(path: &Path) -> Result<(String, String), ItemFileError> {
    let bytes = fs::read(path).map_err(|e| io_err(path, e))?;
    let raw = std::str::from_utf8(&bytes)?;
    let normalized = normalize_crlf(raw);
    let parsed = backlog_file::parse_frontmatter(&normalized);
    if !parsed.frontmatter.present() {
        return Err(ItemFileError::MissingFrontmatter);
    }
    Ok((parsed.frontmatter.serialize(), parsed.body.to_string()))
}

/// Write `contents` to `path` atomically: stage in a sibling temp file, then
/// rename over the target. `rename` is atomic on a single filesystem, so a
/// failure partway through leaves the original file untouched.
fn write_atomic(path: &Path, contents: &str) -> Result<(), ItemFileError> {
    let tmp = temp_sibling(path);
    fs::write(&tmp, contents).map_err(|e| io_err(&tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp); // best-effort cleanup of the stage file
        io_err(path, e)
    })
}

/// A temp path next to `path` (same directory, so `rename` stays on one
/// filesystem). The PID disambiguates concurrent processes.
fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

fn filename_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Normalize all line-ending conventions (CRLF *and* bare CR) to LF, so no stray
/// `\r` byte survives into a written item file. Delegates to the single shared
/// implementation in `file_io` rather than re-deriving a CRLF-only variant here:
/// FMT-WS-001 requires bare-CR normalization too, and item files are
/// VAT-managed files.
// @spec FMT-WS-001
fn normalize_crlf(s: &str) -> String {
    file_io::normalize_line_endings(s)
}

/// The length, in bytes, of the longest leading-whitespace prefix shared by all
/// the given lines. "Leading whitespace" is the run of space/tab bytes at the
/// start of a line. The comparison is byte-for-byte, so a tab and a space do not
/// match. Returns 0 for an empty iterator or when no prefix is shared.
fn common_leading_ws_prefix_len<S: AsRef<str>>(mut lines: impl Iterator<Item = S>) -> usize {
    let leading_ws = |s: &str| s.bytes().take_while(|b| *b == b' ' || *b == b'\t').count();
    let Some(first) = lines.next() else {
        return 0;
    };
    let first = first.as_ref();
    let mut prefix = &first[..leading_ws(first)];
    for line in lines {
        let line = line.as_ref();
        let ws = &line[..leading_ws(line)];
        let common = prefix
            .bytes()
            .zip(ws.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        prefix = &prefix[..common];
        if prefix.is_empty() {
            break;
        }
    }
    prefix.len()
}

#[cfg(test)]
mod tests {
    use super::{ItemFileError, append_notes, read, strip_notes, write_new};
    use std::fs;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        tempfile::tempdir().expect("create tempdir")
    }

    // -------------------- strip_notes / SYNC-NOTES-004 --------------------

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_empty_input_is_empty() {
        assert_eq!(strip_notes(""), "");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_only_blank_lines_is_empty() {
        assert_eq!(strip_notes("\n\n   \n\t\n"), "");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_single_line_no_indent_unchanged() {
        assert_eq!(strip_notes("foo"), "foo");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_strips_common_space_indent() {
        assert_eq!(strip_notes("  foo\n  bar"), "foo\nbar");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_strips_common_tab_indent() {
        assert_eq!(strip_notes("\tfoo\n\tbar"), "foo\nbar");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_uses_byte_prefix_for_mixed_ws() {
        // first leading ws = "\t  ", second = "\t ", common byte prefix = "\t "
        assert_eq!(strip_notes("\t  foo\n\t bar"), " foo\nbar");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_no_common_prefix_means_no_strip() {
        // tab-led vs space-led share no leading-ws prefix
        assert_eq!(strip_notes("\tfoo\n bar"), "\tfoo\n bar");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_trims_leading_blank_lines() {
        assert_eq!(strip_notes("\n\n  foo\n  bar"), "foo\nbar");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_trims_trailing_blank_lines() {
        assert_eq!(strip_notes("  foo\n  bar\n\n  \n"), "foo\nbar");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_preserves_interior_blank_lines_as_empty() {
        assert_eq!(strip_notes("  foo\n\n  bar"), "foo\n\nbar");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_interior_blank_does_not_force_zero_indent() {
        // The interior blank line (zero-indent) must not pull the common prefix to "".
        assert_eq!(strip_notes("  foo\n\n  bar\n  baz"), "foo\n\nbar\nbaz");
    }

    // @spec SYNC-NOTES-004
    #[test]
    fn strip_notes_is_idempotent() {
        let once = strip_notes("  foo\n  bar");
        let twice = strip_notes(&once);
        assert_eq!(once, twice);
    }

    // @spec SYNC-NOTES-004, FMT-WS-001
    #[test]
    fn strip_notes_normalizes_crlf() {
        // CRLF input must not leave stray `\r` bytes on the kept lines.
        let out = strip_notes("  foo\r\n  bar\r\n");
        assert_eq!(out, "foo\nbar");
        assert!(!out.contains('\r'));
    }

    // @spec SYNC-NOTES-004, FMT-WS-001
    #[test]
    fn strip_notes_normalizes_bare_cr() {
        // Old-Mac bare-CR line endings must also be normalized (FMT-WS-001),
        // otherwise the dedent and join would carry a stray `\r` into the file.
        let out = strip_notes("  foo\r  bar\r");
        assert_eq!(out, "foo\nbar");
        assert!(!out.contains('\r'));
    }

    // -------------------- write_new / FMT-ITEM-001 --------------------

    // @spec FMT-ITEM-001
    #[test]
    fn write_new_creates_file_with_frontmatter_and_body() {
        let dir = tmp();
        let path = dir.path().join("items").join("foo-7k2.md");
        write_new(&path, "foo-7k2", "Notes here.").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "---\nid: foo-7k2\n---\n\nNotes here.\n");
    }

    // @spec FMT-ITEM-001
    #[test]
    fn write_new_creates_parent_directory_if_missing() {
        let dir = tmp();
        let path = dir.path().join("a").join("b").join("foo-7k2.md");
        assert!(!path.parent().unwrap().exists());
        write_new(&path, "foo-7k2", "body").unwrap();
        assert!(path.exists());
    }

    // @spec FMT-ITEM-001, SYNC-NOTES-004
    #[test]
    fn write_new_strips_indentation_in_body() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        write_new(&path, "foo-7k2", "  line one\n  line two").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "---\nid: foo-7k2\n---\n\nline one\nline two\n");
    }

    // @spec FMT-ITEM-001
    #[test]
    fn write_new_normalizes_trailing_newlines_to_one() {
        let dir = tmp();
        // Separate paths: write_new refuses to overwrite (see below), so a body
        // with no trailing newline and one with many go to distinct files.
        let no_nl_path = dir.path().join("foo-7k2.md");
        let many_nl_path = dir.path().join("foo-9hf.md");
        write_new(&no_nl_path, "foo-7k2", "body").unwrap();
        write_new(&many_nl_path, "foo-9hf", "body\n\n\n\n").unwrap();
        let no_nl = fs::read_to_string(&no_nl_path).unwrap();
        let many_nl = fs::read_to_string(&many_nl_path).unwrap();
        assert_eq!(no_nl, "---\nid: foo-7k2\n---\n\nbody\n");
        assert_eq!(many_nl, "---\nid: foo-9hf\n---\n\nbody\n");
    }

    // @spec FMT-ITEM-001
    #[test]
    fn write_new_refuses_to_overwrite_existing_file() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        write_new(&path, "foo-7k2", "original").unwrap();
        let err = write_new(&path, "foo-7k2", "clobber").unwrap_err();
        assert!(matches!(err, ItemFileError::AlreadyExists(_)));
        // Original content is untouched.
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "---\nid: foo-7k2\n---\n\noriginal\n");
    }

    // @spec FMT-ITEM-001, FMT-WS-001
    #[test]
    fn write_new_normalizes_crlf_in_body() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        write_new(&path, "foo-7k2", "line one\r\nline two\r\n").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains('\r'));
        assert_eq!(contents, "---\nid: foo-7k2\n---\n\nline one\nline two\n");
    }

    // -------------------- append_notes / FMT-ITEM-002 --------------------

    // @spec FMT-ITEM-002
    #[test]
    fn append_notes_adds_blank_line_and_new_notes() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        write_new(&path, "foo-7k2", "first").unwrap();
        append_notes(&path, "second").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "---\nid: foo-7k2\n---\n\nfirst\n\nsecond\n");
    }

    // @spec FMT-ITEM-002
    #[test]
    fn append_notes_preserves_frontmatter_unchanged() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        fs::write(&path, "---\nid: foo-7k2\ncustom: keep\n---\n\nfirst\n").unwrap();
        append_notes(&path, "second").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("---\nid: foo-7k2\ncustom: keep\n---\n"));
        assert!(contents.ends_with("first\n\nsecond\n"));
    }

    // @spec FMT-ITEM-002
    #[test]
    fn append_notes_collapses_extra_trailing_newlines_in_existing_body() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        fs::write(&path, "---\nid: foo-7k2\n---\n\nfirst\n\n\n\n").unwrap();
        append_notes(&path, "second").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "---\nid: foo-7k2\n---\n\nfirst\n\nsecond\n");
    }

    // @spec FMT-ITEM-002, SYNC-NOTES-004
    #[test]
    fn append_notes_strips_indentation_of_new_notes_independently() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        // existing body has no indent; new notes have a 4-space indent that should be stripped
        // independently (not influenced by the existing body's indent).
        write_new(&path, "foo-7k2", "first").unwrap();
        append_notes(&path, "    indented\n    second").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents,
            "---\nid: foo-7k2\n---\n\nfirst\n\nindented\nsecond\n"
        );
    }

    // @spec FMT-ITEM-002
    #[test]
    fn append_notes_errors_when_frontmatter_missing() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        fs::write(&path, "no frontmatter here\n").unwrap();
        let err = append_notes(&path, "second").unwrap_err();
        assert!(matches!(err, ItemFileError::MissingFrontmatter));
    }

    // @spec FMT-ITEM-002
    #[test]
    fn append_notes_collapses_trailing_newlines_in_new_notes_argument() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        write_new(&path, "foo-7k2", "first").unwrap();
        append_notes(&path, "second\n\n\n").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "---\nid: foo-7k2\n---\n\nfirst\n\nsecond\n");
    }

    // Empty-after-strip contract (Q12A): the primitive writes/appends the empty
    // body anyway. SYNC-NOTES-005's "don't touch the file" guard lives in the
    // caller (vat sync), not here.
    // @spec FMT-ITEM-001
    #[test]
    fn write_new_with_body_empty_after_strip_writes_empty_body() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        write_new(&path, "foo-7k2", "   \n\n").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "---\nid: foo-7k2\n---\n\n\n");
    }

    // @spec FMT-ITEM-002
    #[test]
    fn append_notes_with_empty_new_notes_adds_blank_line_only() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        write_new(&path, "foo-7k2", "first").unwrap();
        append_notes(&path, "   \n\n").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "---\nid: foo-7k2\n---\n\nfirst\n\n\n");
    }

    // @spec FMT-ITEM-002
    #[test]
    fn append_notes_twice_in_a_row_is_predictable() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        write_new(&path, "foo-7k2", "first").unwrap();
        append_notes(&path, "second").unwrap();
        append_notes(&path, "third").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents,
            "---\nid: foo-7k2\n---\n\nfirst\n\nsecond\n\nthird\n"
        );
    }

    // -------------------- read / FMT-ITEM-003 + FMT-WS-001 --------------------

    // @spec FMT-ITEM-003
    #[test]
    fn read_exposes_filename_stem_as_id() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        write_new(&path, "foo-7k2", "body").unwrap();
        let f = read(&path).unwrap();
        assert_eq!(f.id(), "foo-7k2");
    }

    // @spec FMT-ITEM-003
    #[test]
    fn read_does_not_validate_frontmatter_id_against_filename() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        // frontmatter id mismatches filename stem; read must not error
        fs::write(&path, "---\nid: bar-9hf\n---\n\nbody\n").unwrap();
        let f = read(&path).unwrap();
        assert_eq!(f.id(), "foo-7k2"); // filename wins
        assert_eq!(f.body(), "\nbody\n");
    }

    // @spec FMT-ITEM-003
    #[test]
    fn read_errors_when_frontmatter_missing() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        fs::write(&path, "just body\n").unwrap();
        let err = read(&path).unwrap_err();
        assert!(matches!(err, ItemFileError::MissingFrontmatter));
    }

    // @spec FMT-WS-001
    #[test]
    fn read_normalizes_crlf_to_lf() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        fs::write(&path, "---\r\nid: foo-7k2\r\n---\r\n\r\nbody line\r\n").unwrap();
        let f = read(&path).unwrap();
        assert!(!f.body().contains('\r'));
        assert_eq!(f.body(), "\nbody line\n");
    }

    // @spec FMT-WS-001
    #[test]
    fn read_normalizes_bare_cr_to_lf() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        // Bare-CR line endings throughout, including in the frontmatter delimiters.
        fs::write(&path, "---\rid: foo-7k2\r---\r\rbody line\r").unwrap();
        let f = read(&path).unwrap();
        assert!(!f.body().contains('\r'));
        assert_eq!(f.body(), "\nbody line\n");
    }

    // @spec FMT-WS-001
    #[test]
    fn append_notes_normalizes_crlf_in_existing_file() {
        let dir = tmp();
        let path = dir.path().join("foo-7k2.md");
        fs::write(&path, "---\r\nid: foo-7k2\r\n---\r\n\r\nfirst\r\n").unwrap();
        append_notes(&path, "second").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains('\r'));
        assert!(contents.ends_with("first\n\nsecond\n"));
    }
}
