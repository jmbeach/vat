// @spec FMT-FM-001, FMT-FM-002, FMT-FM-003, FMT-FM-004, FMT-RGN-001, FMT-RGN-002, FMT-RGN-003, FMT-RGN-004, FMT-RGN-005, FMT-RGN-006, FMT-RGN-007, FMT-PARSE-001, FMT-PARSE-002, FMT-PARSE-003, FMT-PARSE-004, FMT-PARSE-005

#![allow(dead_code)]

use serde_yaml::{Mapping, Value};
use thiserror::Error;

/// A parsed view of `backlog/backlog.md`: optional YAML frontmatter, a parsed
/// region, and an optional freeform region. Borrows from the input string.
///
/// Pre-condition: `input` is LF-normalized (CRLF normalization is FMT-WS-001's
/// concern and lives in the file-reading layer, not here). The frontmatter and
/// body separators are matched as the exact bytes `---\n`, so a CRLF-terminated
/// `---\r\n` line is NOT recognized as a delimiter. Callers must normalize line
/// endings upstream.
pub(crate) struct BacklogFile<'a> {
    frontmatter: Frontmatter,
    parsed: &'a str,
    freeform: Option<&'a str>,
}

impl<'a> BacklogFile<'a> {
    // @spec FMT-FM-001, FMT-RGN-001, FMT-RGN-002, FMT-RGN-007
    pub(crate) fn parse(input: &'a str) -> Self {
        let fm = parse_frontmatter(input);
        let (parsed, freeform) = split_body(fm.body);
        BacklogFile {
            frontmatter: fm.frontmatter,
            parsed,
            freeform,
        }
    }

    pub(crate) fn frontmatter(&self) -> &Frontmatter {
        &self.frontmatter
    }

    pub(crate) fn parsed(&self) -> &str {
        self.parsed
    }

    pub(crate) fn freeform(&self) -> Option<&str> {
        self.freeform
    }

    // @spec FMT-RGN-003
    //
    // Composes the file from `new_parsed` (the caller's mutated parsed region)
    // plus the preserved frontmatter and freeform regions. When the freeform
    // region is present, the structural `---\n` separator is re-emitted between
    // the parsed region and the byte-for-byte-preserved freeform; when absent,
    // only the frontmatter and parsed region are emitted.
    //
    // Pre-condition: when the freeform region is present, `new_parsed` must be
    // empty or end with `\n`. Otherwise the re-emitted `---\n` separator
    // butt-joins `new_parsed`'s last line (e.g. `"- task" + "---\n"` →
    // `"- task---\n"`), which `split_body` would NOT recognize as a separator on
    // the next read — silently swallowing the freeform region into the parsed
    // region. Callers that serialize a parsed region always terminate their
    // bullets with `\n`; the debug assert pins the invariant in tests without
    // costing anything in release builds.
    pub(crate) fn serialize(&self, new_parsed: &str) -> String {
        let mut out = self.frontmatter.serialize();
        out.push_str(new_parsed);
        if let Some(ff) = self.freeform {
            debug_assert!(
                new_parsed.is_empty() || new_parsed.ends_with('\n'),
                "serialize: new_parsed must be empty or end with '\\n' when the \
                 freeform region is present, else the re-emitted separator merges \
                 into the parsed region's last line"
            );
            out.push_str("---\n");
            out.push_str(ff);
        }
        out
    }
}

/// Split a frontmatter-stripped body into `(parsed, freeform)`.
///
/// The boundary is the first line equal to exactly the bytes `---\n`. The
/// separator itself belongs to neither region: `parsed` ends just before it and
/// `freeform` begins just after it. When no such line exists the whole body is
/// `parsed` and `freeform` is `None` (distinct from a present-but-empty
/// freeform, which is `Some("")`).
// @spec FMT-RGN-001, FMT-RGN-002, FMT-RGN-004, FMT-RGN-005, FMT-RGN-006, FMT-RGN-007
fn split_body(body: &str) -> (&str, Option<&str>) {
    const SEP: &str = "---\n";
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if line == SEP {
            let parsed = &body[..offset];
            let freeform = &body[offset + SEP.len()..];
            return (parsed, Some(freeform));
        }
        offset += line.len();
    }
    (body, None)
}

// ---------------------------------------------------------------------------
// Parsed region: preamble + task entries
// ---------------------------------------------------------------------------

/// A single task entry inside the parsed region of `backlog.md`.
///
/// `bullet` is the bullet line verbatim (including the leading `- ` and the
/// trailing `\n`). `notes` is the slice of the parsed region that covers all
/// subsequent non-bullet lines up until the next bullet or the end of the
/// parsed region, with any trailing blank lines that immediately precede the
/// next bullet removed.
///
/// Both slices borrow from the input string passed to `parse_region`.
// @spec FMT-PARSE-001, FMT-PARSE-003
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TaskEntry<'a> {
    /// The bullet line verbatim, e.g. `"- [foo-7k2] My task\n"`.
    pub(crate) bullet: &'a str,
    /// All non-bullet lines following the bullet, with trailing blank lines
    /// before the next bullet stripped.  Empty string when there are no notes.
    pub(crate) notes: &'a str,
}

/// The result of parsing the parsed region of `backlog.md`.
///
/// - `preamble`: everything before the first bullet line (may be empty but is
///   never `None`). Borrows from the input.
/// - `entries`: the ordered list of task entries.
// @spec FMT-PARSE-004, FMT-PARSE-005
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParsedRegion<'a> {
    /// Text before the first bullet line; preserved byte-for-byte on rewrite.
    pub(crate) preamble: &'a str,
    /// Task entries in the order they appeared in the input.
    pub(crate) entries: Vec<TaskEntry<'a>>,
}

/// Parse the parsed region of `backlog.md` into preamble + task entries.
///
/// A bullet line is any line that starts at column 0 with `"- "` (hyphen
/// followed by a single space).  Lines starting with `"* "` or `"+ "` are
/// **not** bullets (FMT-PARSE-002); they are treated as note content of the
/// preceding bullet, or as preamble if no bullet has been seen yet.
///
/// Pre-condition: `parsed_region` is LF-normalized (FMT-WS-001).
// @spec FMT-PARSE-001, FMT-PARSE-002, FMT-PARSE-003, FMT-PARSE-004, FMT-PARSE-005
pub(crate) fn parse_region(parsed_region: &str) -> ParsedRegion<'_> {
    // We work by tracking byte-level offsets within `parsed_region` so we can
    // hand out `&str` slices that borrow directly from the input.
    let mut entries: Vec<TaskEntry<'_>> = Vec::new();

    // `preamble_end` is updated forward until the first bullet is found.
    let mut preamble_end: usize = 0;
    // `current_bullet_start` / `current_notes_start` track the bullet we are
    // currently accumulating.  `None` while we are still in the preamble.
    let mut current_bullet_start: Option<usize> = None;
    let mut current_bullet_end: usize = 0; // end of the bullet line itself
    let mut current_notes_end: usize = 0; // running end of notes for current bullet

    let mut offset: usize = 0;

    for line in parsed_region.split_inclusive('\n') {
        let line_end = offset + line.len();
        if is_bullet(line) {
            // Commit the previous entry (if any).
            if let Some(bstart) = current_bullet_start {
                let bullet = &parsed_region[bstart..current_bullet_end];
                // Notes: from current_bullet_end up to current_notes_end,
                // but with trailing blank lines stripped (FMT-PARSE-003).
                let raw_notes = &parsed_region[current_bullet_end..current_notes_end];
                let notes = strip_trailing_blank_lines(raw_notes);
                entries.push(TaskEntry { bullet, notes });
            } else {
                // First bullet found — lock in the preamble end.
                preamble_end = offset;
            }
            // Start a new entry.
            current_bullet_start = Some(offset);
            current_bullet_end = line_end;
            current_notes_end = line_end;
        } else if current_bullet_start.is_some() {
            // A non-bullet line after a bullet — note content.
            current_notes_end = line_end;
        } else {
            // No bullet seen yet — extend the preamble.
            preamble_end = line_end;
        }
        offset = line_end;
    }

    // Commit the last entry (if any).
    if let Some(bstart) = current_bullet_start {
        let bullet = &parsed_region[bstart..current_bullet_end];
        let raw_notes = &parsed_region[current_bullet_end..current_notes_end];
        let notes = strip_trailing_blank_lines(raw_notes);
        entries.push(TaskEntry { bullet, notes });
    }

    ParsedRegion {
        preamble: &parsed_region[..preamble_end],
        entries,
    }
}

/// Returns `true` iff `line` is a bullet line: starts with exactly `"- "`.
///
/// Lines starting with `"* "`, `"+ "`, `"-"` (no space), or `"--"` etc. are
/// NOT bullets.
// @spec FMT-PARSE-001, FMT-PARSE-002
fn is_bullet(line: &str) -> bool {
    line.starts_with("- ")
}

/// Strip trailing blank lines from a notes block.
///
/// A "blank line" here is a line that, after trimming, contains no non-whitespace
/// characters.  We strip them from the end because the LLD says notes end at
/// "the next bullet at column 0 or the end of the parsed region, excluding any
/// trailing blank lines that immediately precede the next bullet."
///
/// Returns a slice of `notes` with the trailing blank lines removed.
fn strip_trailing_blank_lines(notes: &str) -> &str {
    // Scan from the end, dropping lines that are blank.
    let mut end = notes.len();
    // Walk backward line by line.  `split_inclusive('\n')` goes forward, so
    // instead we iterate over the string by finding the last `\n` each time.
    loop {
        let trimmed = &notes[..end];
        // Find the last line.
        let last_line_start = match trimmed.rfind('\n') {
            // rfind returns the position of the \n; the line starts one byte after.
            Some(pos) if pos + 1 < trimmed.len() => pos + 1,
            // The \n is at the very end — the line is everything before the \n.
            Some(pos) => {
                // Back up further: find the newline before this one.
                let before = &trimmed[..pos];
                match before.rfind('\n') {
                    Some(p2) => p2 + 1,
                    None => 0,
                }
            }
            None => 0,
        };
        let last_line = &notes[last_line_start..end];
        if last_line.trim().is_empty() {
            end = last_line_start;
            if end == 0 {
                break;
            }
        } else {
            break;
        }
    }
    &notes[..end]
}

// ---------------------------------------------------------------------------
// Frontmatter (absorbed from the former `frontmatter` module).
// ---------------------------------------------------------------------------

pub(crate) struct Frontmatter {
    present: bool,
    // Owned (`String`) rather than `&'a str` on purpose: `Frontmatter` is
    // deliberately decoupled from the input's lifetime so it can be carried
    // independently of the borrowing `BacklogFile<'a>` (e.g. read once, then
    // outlive the buffer). The copy is a small contiguous substring; the
    // asymmetry with `BacklogFile.parsed`/`.freeform` (which do borrow) is
    // intentional, not an oversight to "fix" into a lifetime parameter.
    raw_body: String,
    parsed: Mapping,
}

pub(crate) struct FrontmatterParse<'a> {
    pub(crate) frontmatter: Frontmatter,
    pub(crate) body: &'a str,
}

impl Frontmatter {
    pub(crate) fn present(&self) -> bool {
        self.present
    }

    // @spec FMT-FM-003
    pub(crate) fn version(&self) -> u64 {
        self.parsed
            .get("version")
            .and_then(Value::as_u64)
            .unwrap_or(1)
    }

    // @spec FMT-FM-004
    pub(crate) fn serialize(&self) -> String {
        if !self.present {
            return String::new();
        }
        format!("---\n{}---\n", self.raw_body)
    }
}

/// The highest backlog schema major version this CLI understands.
// @spec FMT-FM-002
pub(crate) const SUPPORTED_MAJOR: u64 = 1;

// @spec FMT-FM-002
#[derive(Debug, Error, PartialEq, Eq)]
#[error(
    "backlog file is version {found}, this CLI supports up to version {supported}; please upgrade vat."
)]
pub(crate) struct UnsupportedVersion {
    pub(crate) found: u64,
    pub(crate) supported: u64,
}

// @spec FMT-FM-002, CMD-CC-001
//
// Cross-cutting gate run at the top of every read-path command, after parsing
// frontmatter and before any other work. Pure — it never writes; aborting on
// the returned `Err` is what gives the "no writes when too new" guarantee.
pub(crate) fn check_version(fm: &Frontmatter) -> Result<(), UnsupportedVersion> {
    let found = fm.version();
    if found > SUPPORTED_MAJOR {
        return Err(UnsupportedVersion {
            found,
            supported: SUPPORTED_MAJOR,
        });
    }
    Ok(())
}

// @spec FMT-FM-001, FMT-FM-003, FMT-FM-004
//
// Pre-condition: `input` is LF-normalized per FMT-WS-001. The opening and
// closing delimiters are matched as `"---\n"`, so a CRLF-terminated input
// silently parses as no-frontmatter — which would let a `version: N` file
// from a Windows editor skip the FMT-FM-002 version check. Callers must
// normalize line endings upstream.
pub(crate) fn parse_frontmatter(input: &str) -> FrontmatterParse<'_> {
    if let Some(after_open) = input.strip_prefix("---\n")
        && let Some(raw_body_end) = find_closing_delimiter(after_open)
    {
        let raw_body = &after_open[..raw_body_end];
        let body_start = (input.len() - after_open.len()) + raw_body_end + "---\n".len();
        return FrontmatterParse {
            frontmatter: Frontmatter {
                present: true,
                raw_body: raw_body.to_string(),
                parsed: parse_yaml_mapping(raw_body),
            },
            body: &input[body_start..],
        };
    }
    FrontmatterParse {
        frontmatter: Frontmatter {
            present: false,
            raw_body: String::new(),
            parsed: Mapping::new(),
        },
        body: input,
    }
}

/// Scan `after_open` for the first line that is exactly `"---\n"`.
/// Returns the byte offset of that line's first byte (i.e. the end of the raw body).
/// An unterminated `---` at EOF does NOT count — the closing delimiter must be newline-terminated.
fn find_closing_delimiter(after_open: &str) -> Option<usize> {
    let mut byte_offset = 0;
    for line in after_open.split_inclusive('\n') {
        if line == "---\n" {
            return Some(byte_offset);
        }
        byte_offset += line.len();
    }
    None
}

fn parse_yaml_mapping(raw_body: &str) -> Mapping {
    if raw_body.trim().is_empty() {
        return Mapping::new();
    }
    match serde_yaml::from_str::<Value>(raw_body) {
        Ok(Value::Mapping(m)) => m,
        _ => Mapping::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BacklogFile, SUPPORTED_MAJOR, UnsupportedVersion, check_version, parse_frontmatter,
        parse_region, split_body,
    };

    // ===================================================================
    // Version check tests (FMT-FM-002, FMT-FM-003)
    // ===================================================================

    // @spec FMT-FM-002
    #[test]
    fn version_equal_to_supported_passes() {
        let r = parse_frontmatter("---\nversion: 1\n---\nbody\n");
        assert_eq!(check_version(&r.frontmatter), Ok(()));
    }

    // @spec FMT-FM-002
    #[test]
    fn version_below_supported_passes() {
        let r = parse_frontmatter("---\nversion: 0\n---\nbody\n");
        assert_eq!(check_version(&r.frontmatter), Ok(()));
    }

    // @spec FMT-FM-002
    #[test]
    fn version_above_supported_is_rejected() {
        let r = parse_frontmatter("---\nversion: 2\n---\nbody\n");
        assert_eq!(
            check_version(&r.frontmatter),
            Err(UnsupportedVersion {
                found: 2,
                supported: SUPPORTED_MAJOR,
            })
        );
    }

    // @spec FMT-FM-002, FMT-FM-003
    #[test]
    fn absent_frontmatter_passes_as_version_1() {
        let r = parse_frontmatter("just body\n");
        assert_eq!(check_version(&r.frontmatter), Ok(()));
    }

    // @spec FMT-FM-002, FMT-FM-003
    #[test]
    fn empty_frontmatter_passes_as_version_1() {
        let r = parse_frontmatter("---\n---\nbody\n");
        assert_eq!(check_version(&r.frontmatter), Ok(()));
    }

    // @spec FMT-FM-002, FMT-FM-003
    #[test]
    fn non_integer_version_passes_as_version_1() {
        let r = parse_frontmatter("---\nversion: \"two\"\n---\nbody\n");
        assert_eq!(check_version(&r.frontmatter), Ok(()));
    }

    // @spec FMT-FM-002
    #[test]
    fn error_message_names_both_the_file_and_supported_versions() {
        let r = parse_frontmatter("---\nversion: 7\n---\nbody\n");
        let msg = check_version(&r.frontmatter)
            .expect_err("version 7 should be rejected")
            .to_string();
        assert!(
            msg.contains('7'),
            "message should name the file version: {msg:?}"
        );
        assert!(
            msg.contains('1'),
            "message should name the supported version: {msg:?}"
        );
        assert!(
            msg.contains("upgrade vat"),
            "message should point the user at upgrading: {msg:?}"
        );
    }

    // ===================================================================
    // Frontmatter unit tests (FMT-FM-001, FMT-FM-003, FMT-FM-004)
    // exercise the internal `parse_frontmatter` directly.
    // ===================================================================

    // @spec FMT-FM-001
    #[test]
    fn fm_detects_when_file_begins_with_delimiter() {
        let r = parse_frontmatter("---\nversion: 1\n---\nbody\n");
        assert!(r.frontmatter.present());
        assert_eq!(r.body, "body\n");
    }

    // @spec FMT-FM-001
    #[test]
    fn fm_no_frontmatter_when_file_does_not_begin_with_delimiter() {
        let input = "hello\n---\nversion: 1\n---\n";
        let r = parse_frontmatter(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn fm_unclosed_frontmatter_is_treated_as_no_frontmatter() {
        let input = "---\nversion: 1\nno closing delimiter\n";
        let r = parse_frontmatter(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn fm_empty_frontmatter_is_recognized() {
        let r = parse_frontmatter("---\n---\nbody\n");
        assert!(r.frontmatter.present());
        assert_eq!(r.body, "body\n");
    }

    // @spec FMT-FM-001
    #[test]
    fn fm_delimiter_with_trailing_whitespace_is_not_recognized() {
        let input = "--- \nversion: 1\n---\n";
        let r = parse_frontmatter(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn fm_line_starting_with_three_dashes_but_not_solely_is_not_a_closing_delimiter() {
        let r = parse_frontmatter("---\nversion: 1\n---foo\n---\nbody\n");
        assert!(r.frontmatter.present());
        assert_eq!(r.body, "body\n");
    }

    // @spec FMT-FM-001
    #[test]
    fn fm_closing_delimiter_without_trailing_newline_is_not_recognized() {
        let input = "---\nversion: 1\n---";
        let r = parse_frontmatter(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    //
    // Zero-body variant of the above: an empty frontmatter whose closing `---`
    // lacks a trailing newline. Regression anchor so a future
    // `find_closing_delimiter` relaxation (e.g. trimming) can't silently start
    // recognizing `"---\n---"` as a closed frontmatter block.
    #[test]
    fn fm_empty_frontmatter_without_trailing_newline_is_not_recognized() {
        let input = "---\n---";
        let r = parse_frontmatter(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    //
    // The CRLF hazard named in `parse_frontmatter`'s doc comment: a `---\r\n`
    // opening delimiter does NOT match the exact `---\n` and so parses as
    // no-frontmatter, which would let a `version: N` Windows-edited file skip
    // the FMT-FM-002 version check. CRLF normalization (FMT-WS-001) is the
    // file-reading layer's job; this test pins the un-normalized behavior here.
    #[test]
    fn fm_crlf_opening_delimiter_is_not_recognized() {
        let input = "---\r\nversion: 2\n---\r\n";
        let r = parse_frontmatter(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
        // Because frontmatter is not detected, the version check never sees the 2.
        assert_eq!(r.frontmatter.version(), 1);
    }

    // @spec FMT-FM-001
    #[test]
    fn fm_opening_delimiter_alone_at_eof_is_no_frontmatter() {
        let input = "---\n";
        let r = parse_frontmatter(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn fm_line_scan_wins_over_yaml_quoted_multiline() {
        let input = "---\nweird: \"line1\n---\nline3\"\n---\nbody\n";
        let r = parse_frontmatter(input);
        assert!(r.frontmatter.present());
        assert_eq!(r.body, "line3\"\n---\nbody\n");
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    // @spec FMT-FM-003
    #[test]
    fn fm_missing_version_key_defaults_to_1() {
        let r = parse_frontmatter("---\nother: foo\n---\n");
        assert_eq!(r.frontmatter.version(), 1);
    }

    // @spec FMT-FM-003
    #[test]
    fn fm_absent_frontmatter_defaults_to_version_1() {
        let r = parse_frontmatter("just body\n");
        assert_eq!(r.frontmatter.version(), 1);
    }

    // @spec FMT-FM-003
    #[test]
    fn fm_empty_frontmatter_defaults_to_version_1() {
        let r = parse_frontmatter("---\n---\nbody\n");
        assert_eq!(r.frontmatter.version(), 1);
    }

    // @spec FMT-FM-003
    #[test]
    fn fm_non_integer_version_value_defaults_to_1() {
        let r = parse_frontmatter("---\nversion: \"two\"\n---\n");
        assert_eq!(r.frontmatter.version(), 1);
    }

    #[test]
    fn fm_version_key_is_exposed_as_u64() {
        let r = parse_frontmatter("---\nversion: 2\n---\n");
        assert_eq!(r.frontmatter.version(), 2);
    }

    // @spec FMT-FM-004
    #[test]
    fn fm_serialize_preserves_unknown_keys_verbatim() {
        let r = parse_frontmatter("---\nversion: 1\ncustom: hello\nother: value\n---\nbody\n");
        assert_eq!(
            r.frontmatter.serialize(),
            "---\nversion: 1\ncustom: hello\nother: value\n---\n"
        );
    }

    // @spec FMT-FM-004
    #[test]
    fn fm_round_trip_preserves_comments_and_blank_lines() {
        let input = "---\n# leading comment\nversion: 1\n\nbar: 42\n---\nbody\n";
        let r = parse_frontmatter(input);
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    // @spec FMT-FM-004
    #[test]
    fn fm_round_trip_preserves_nested_yaml_verbatim() {
        let input = "---\nnested:\n  key: value\n  list:\n    - one\n    - two\n---\nbody\n";
        let r = parse_frontmatter(input);
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    // @spec FMT-FM-004
    #[test]
    fn fm_round_trip_preserves_multibyte_unicode_in_keys_and_values() {
        let input = "---\nкл: значение\nemoji: \u{1F31F}\n---\nbody\n";
        let r = parse_frontmatter(input);
        assert!(r.frontmatter.present());
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    // @spec FMT-FM-004
    #[test]
    fn fm_serialize_when_absent_returns_empty_string() {
        let r = parse_frontmatter("no frontmatter here\n");
        assert_eq!(r.frontmatter.serialize(), "");
    }

    #[test]
    fn fm_malformed_yaml_still_round_trips_with_default_version() {
        let input = "---\nkey: value\n  bad: indent under scalar\n---\nbody\n";
        let r = parse_frontmatter(input);
        assert_eq!(r.frontmatter.version(), 1);
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    #[test]
    fn fm_non_mapping_yaml_round_trips_with_default_version() {
        let input = "---\n- list\n- items\n---\nbody\n";
        let r = parse_frontmatter(input);
        assert_eq!(r.frontmatter.version(), 1);
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    #[test]
    fn fm_empty_input_is_no_frontmatter() {
        let r = parse_frontmatter("");
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, "");
    }

    // ===================================================================
    // split_body unit tests (FMT-RGN-001..007) exercise the splitter in
    // isolation so each assertion targets the split rule directly.
    // ===================================================================

    // @spec FMT-RGN-001
    #[test]
    fn split_separates_parsed_from_freeform() {
        let (parsed, freeform) = split_body("tasks here\n---\nfree text\n");
        assert_eq!(parsed, "tasks here\n");
        assert_eq!(freeform, Some("free text\n"));
    }

    // @spec FMT-RGN-001
    #[test]
    fn split_only_first_separator_splits_rest_stays_in_freeform() {
        let (parsed, freeform) = split_body("parsed\n---\nfree1\n---\nfree2\n");
        assert_eq!(parsed, "parsed\n");
        assert_eq!(freeform, Some("free1\n---\nfree2\n"));
    }

    // @spec FMT-RGN-002
    #[test]
    fn split_no_separator_whole_body_is_parsed_freeform_none() {
        let (parsed, freeform) = split_body("just tasks\nno separator here\n");
        assert_eq!(parsed, "just tasks\nno separator here\n");
        assert_eq!(freeform, None);
    }

    // @spec FMT-RGN-004
    #[test]
    fn split_triple_asterisk_is_not_a_separator() {
        let (parsed, freeform) = split_body("parsed\n***\nstill parsed\n");
        assert_eq!(parsed, "parsed\n***\nstill parsed\n");
        assert_eq!(freeform, None);
    }

    // @spec FMT-RGN-004
    #[test]
    fn split_triple_underscore_is_not_a_separator() {
        let (parsed, freeform) = split_body("parsed\n___\nstill parsed\n");
        assert_eq!(parsed, "parsed\n___\nstill parsed\n");
        assert_eq!(freeform, None);
    }

    // @spec FMT-RGN-005
    //
    // Paired near-miss + real separator in ONE fixture: the strict matcher must
    // skip `--- \n` yet still split on the later exact `---\n`. Fails if the
    // matcher is too lenient (would split early) OR never runs (would not split).
    #[test]
    fn split_trailing_space_near_miss_then_real_separator() {
        let (parsed, freeform) = split_body("parsed\n--- \n---\nfree\n");
        assert_eq!(parsed, "parsed\n--- \n");
        assert_eq!(freeform, Some("free\n"));
    }

    // @spec FMT-RGN-005
    #[test]
    fn split_leading_space_near_miss_then_real_separator() {
        let (parsed, freeform) = split_body("parsed\n ---\n---\nfree\n");
        assert_eq!(parsed, "parsed\n ---\n");
        assert_eq!(freeform, Some("free\n"));
    }

    // @spec FMT-RGN-005
    #[test]
    fn split_four_dashes_near_miss_then_real_separator() {
        let (parsed, freeform) = split_body("parsed\n----\n---\nfree\n");
        assert_eq!(parsed, "parsed\n----\n");
        assert_eq!(freeform, Some("free\n"));
    }

    // @spec FMT-RGN-005
    #[test]
    fn split_trailing_space_alone_is_not_a_separator() {
        let (parsed, freeform) = split_body("parsed\n--- \nstill parsed\n");
        assert_eq!(parsed, "parsed\n--- \nstill parsed\n");
        assert_eq!(freeform, None);
    }

    // @spec FMT-RGN-006
    #[test]
    fn split_unterminated_dashes_at_eof_is_not_a_separator() {
        let (parsed, freeform) = split_body("parsed\n---");
        assert_eq!(parsed, "parsed\n---");
        assert_eq!(freeform, None);
    }

    // @spec FMT-RGN-006
    #[test]
    fn split_dashes_followed_by_content_then_eof_is_not_a_separator() {
        let (parsed, freeform) = split_body("parsed\n---foo");
        assert_eq!(parsed, "parsed\n---foo");
        assert_eq!(freeform, None);
    }

    // @spec FMT-RGN-006
    //
    // Q6 (Option A): CRLF normalization is FMT-WS-001's concern, not this
    // module's. A `---\r\n` line must NOT be treated as a separator here.
    #[test]
    fn split_crlf_separator_line_is_not_recognized() {
        let (parsed, freeform) = split_body("parsed\r\n---\r\nstill parsed\r\n");
        assert_eq!(parsed, "parsed\r\n---\r\nstill parsed\r\n");
        assert_eq!(freeform, None);
    }

    // @spec FMT-RGN-007
    #[test]
    fn split_separator_as_first_line_gives_empty_parsed() {
        let (parsed, freeform) = split_body("---\nfree text\n");
        assert_eq!(parsed, "");
        assert_eq!(freeform, Some("free text\n"));
    }

    // @spec FMT-RGN-001
    #[test]
    fn split_separator_as_last_line_gives_empty_present_freeform() {
        let (parsed, freeform) = split_body("parsed\n---\n");
        assert_eq!(parsed, "parsed\n");
        assert_eq!(freeform, Some(""));
    }

    // @spec FMT-RGN-001
    #[test]
    fn split_preserves_multibyte_unicode_across_boundary() {
        let (parsed, freeform) = split_body("café ☕\n---\nναι 🌟\n");
        assert_eq!(parsed, "café ☕\n");
        assert_eq!(freeform, Some("ναι 🌟\n"));
    }

    // ===================================================================
    // BacklogFile integration tests: frontmatter + split composed, and
    // serialize round-trips (FMT-RGN-003).
    // ===================================================================

    // @spec FMT-RGN-001
    #[test]
    fn file_splits_after_frontmatter() {
        let f = BacklogFile::parse("---\nversion: 1\n---\ntasks here\n---\nfree text\n");
        assert!(f.frontmatter().present());
        assert_eq!(f.parsed(), "tasks here\n");
        assert_eq!(f.freeform(), Some("free text\n"));
    }

    // @spec FMT-RGN-001
    #[test]
    fn file_splits_without_frontmatter() {
        let f = BacklogFile::parse("- task one\n- task two\n---\nfree notes\n");
        assert!(!f.frontmatter().present());
        assert_eq!(f.parsed(), "- task one\n- task two\n");
        assert_eq!(f.freeform(), Some("free notes\n"));
    }

    // @spec FMT-RGN-002
    #[test]
    fn file_no_separator_after_frontmatter() {
        let f = BacklogFile::parse("---\nversion: 1\n---\nonly parsed body\n");
        assert_eq!(f.parsed(), "only parsed body\n");
        assert_eq!(f.freeform(), None);
    }

    // @spec FMT-RGN-007
    #[test]
    fn file_separator_immediately_after_frontmatter_gives_empty_parsed() {
        let f = BacklogFile::parse("---\nversion: 1\n---\n---\nfree text\n");
        assert!(f.frontmatter().present());
        assert_eq!(f.parsed(), "");
        assert_eq!(f.freeform(), Some("free text\n"));
    }

    // @spec FMT-RGN-003
    #[test]
    fn file_serialize_composes_parsed_separator_freeform() {
        let input = "parsed\n---\nfree\n";
        let f = BacklogFile::parse(input);
        assert_eq!(f.serialize(f.parsed()), input);
    }

    // @spec FMT-RGN-003
    #[test]
    fn file_serialize_without_freeform_emits_only_parsed() {
        let input = "---\nversion: 1\n---\nparsed only\n";
        let f = BacklogFile::parse(input);
        assert_eq!(f.serialize(f.parsed()), input);
    }

    // @spec FMT-RGN-003
    #[test]
    fn file_serialize_freeform_present_but_empty_emits_separator_only() {
        let input = "parsed\n---\n";
        let f = BacklogFile::parse(input);
        assert_eq!(f.freeform(), Some(""));
        assert_eq!(f.serialize(f.parsed()), input);
    }

    // @spec FMT-RGN-003
    #[test]
    fn file_serialize_frontmatter_plus_present_but_empty_freeform() {
        let input = "---\nversion: 1\n---\nparsed\n---\n";
        let f = BacklogFile::parse(input);
        assert_eq!(f.parsed(), "parsed\n");
        assert_eq!(f.freeform(), Some(""));
        assert_eq!(f.serialize(f.parsed()), input);
    }

    // @spec FMT-RGN-003
    #[test]
    fn file_serialize_preserves_freeform_byte_for_byte() {
        let input = "parsed\n---\n  weird   chars\t\n***\n___\nmore\n";
        let f = BacklogFile::parse(input);
        assert_eq!(f.freeform(), Some("  weird   chars\t\n***\n___\nmore\n"));
        assert_eq!(f.serialize(f.parsed()), input);
    }

    // @spec FMT-RGN-003
    #[test]
    fn file_serialize_with_new_parsed_swaps_only_parsed_region() {
        let f = BacklogFile::parse("---\nversion: 1\n---\nold parsed\n---\nfree\n");
        let out = f.serialize("new parsed\n");
        assert_eq!(out, "---\nversion: 1\n---\nnew parsed\n---\nfree\n");
    }

    // -- round-trip coverage across the four region combinations --

    #[test]
    fn round_trip_no_frontmatter_no_separator() {
        let input = "tasks only\n";
        let f = BacklogFile::parse(input);
        assert_eq!(f.serialize(f.parsed()), input);
    }

    #[test]
    fn round_trip_frontmatter_only_no_separator() {
        let input = "---\nversion: 1\n---\nbody\n";
        let f = BacklogFile::parse(input);
        assert_eq!(f.serialize(f.parsed()), input);
    }

    #[test]
    fn round_trip_no_frontmatter_with_separator() {
        let input = "parsed\n---\nfreeform\n";
        let f = BacklogFile::parse(input);
        assert_eq!(f.serialize(f.parsed()), input);
    }

    #[test]
    fn round_trip_frontmatter_with_separator() {
        let input = "---\nversion: 1\n---\nparsed\n---\nfreeform\n";
        let f = BacklogFile::parse(input);
        assert_eq!(f.serialize(f.parsed()), input);
    }

    #[test]
    fn empty_input_round_trips() {
        let f = BacklogFile::parse("");
        assert_eq!(f.parsed(), "");
        assert_eq!(f.freeform(), None);
        assert_eq!(f.serialize(f.parsed()), "");
    }

    // ===================================================================
    // parse_region tests — FMT-PARSE-001..005
    // ===================================================================

    // @spec FMT-PARSE-001
    #[test]
    fn parse_region_single_bullet_no_notes() {
        let r = parse_region("- task one\n");
        assert_eq!(r.preamble, "");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet, "- task one\n");
        assert_eq!(r.entries[0].notes, "");
    }

    // @spec FMT-PARSE-001
    #[test]
    fn parse_region_multiple_bullets_no_notes() {
        let r = parse_region("- first\n- second\n- third\n");
        assert_eq!(r.preamble, "");
        assert_eq!(r.entries.len(), 3);
        assert_eq!(r.entries[0].bullet, "- first\n");
        assert_eq!(r.entries[1].bullet, "- second\n");
        assert_eq!(r.entries[2].bullet, "- third\n");
        for e in &r.entries {
            assert_eq!(e.notes, "");
        }
    }

    // @spec FMT-PARSE-002
    #[test]
    fn parse_region_star_bullets_are_not_task_entries() {
        // Lines starting with `* ` or `+ ` must not be treated as bullets.
        let r = parse_region("* not a bullet\n+ also not a bullet\n");
        assert_eq!(r.preamble, "* not a bullet\n+ also not a bullet\n");
        assert_eq!(r.entries.len(), 0);
    }

    // @spec FMT-PARSE-002
    #[test]
    fn parse_region_star_after_bullet_is_note() {
        let r = parse_region("- task\n* sub-item treated as note\n");
        assert_eq!(r.preamble, "");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet, "- task\n");
        assert_eq!(r.entries[0].notes, "* sub-item treated as note\n");
    }

    // @spec FMT-PARSE-002
    #[test]
    fn parse_region_plus_after_bullet_is_note() {
        let r = parse_region("- task\n+ sub-item\n");
        assert_eq!(r.preamble, "");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet, "- task\n");
        assert_eq!(r.entries[0].notes, "+ sub-item\n");
    }

    // @spec FMT-PARSE-003
    #[test]
    fn parse_region_notes_follow_bullet_until_next_bullet() {
        let r = parse_region("- task one\n  note line one\n  note line two\n- task two\n");
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].bullet, "- task one\n");
        assert_eq!(r.entries[0].notes, "  note line one\n  note line two\n");
        assert_eq!(r.entries[1].bullet, "- task two\n");
        assert_eq!(r.entries[1].notes, "");
    }

    // @spec FMT-PARSE-003
    #[test]
    fn parse_region_trailing_blank_lines_before_next_bullet_stripped_from_notes() {
        let input = "- task one\n  note\n\n\n- task two\n";
        let r = parse_region(input);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].bullet, "- task one\n");
        // The two trailing blank lines preceding `- task two` must be stripped.
        assert_eq!(r.entries[0].notes, "  note\n");
        assert_eq!(r.entries[1].bullet, "- task two\n");
    }

    // @spec FMT-PARSE-003
    #[test]
    fn parse_region_blank_lines_between_notes_preserved_interior_blanks_kept() {
        // Interior blank lines (surrounded by non-blank note lines) are kept.
        let input = "- task\n  line1\n\n  line2\n";
        let r = parse_region(input);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].notes, "  line1\n\n  line2\n");
    }

    // @spec FMT-PARSE-003
    #[test]
    fn parse_region_notes_at_column_zero_between_two_bullets_attach_to_first() {
        // A paragraph at column 0 between two bullets is part of the first
        // bullet's notes — not preamble for the second.
        let input = "- task one\nparagraph at col 0\n- task two\n";
        let r = parse_region(input);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].notes, "paragraph at col 0\n");
        assert_eq!(r.entries[1].notes, "");
    }

    // @spec FMT-PARSE-004
    #[test]
    fn parse_region_preamble_before_first_bullet() {
        let r = parse_region("# Backlog\n\nSome preamble text.\n- task one\n");
        assert_eq!(r.preamble, "# Backlog\n\nSome preamble text.\n");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet, "- task one\n");
    }

    // @spec FMT-PARSE-004
    #[test]
    fn parse_region_empty_preamble_when_first_line_is_bullet() {
        let r = parse_region("- task\n");
        assert_eq!(r.preamble, "");
    }

    // @spec FMT-PARSE-004
    #[test]
    fn parse_region_no_bullets_whole_region_is_preamble() {
        let r = parse_region("# Title\nJust some text.\n");
        assert_eq!(r.preamble, "# Title\nJust some text.\n");
        assert_eq!(r.entries.len(), 0);
    }

    // @spec FMT-PARSE-004
    #[test]
    fn parse_region_empty_input_gives_empty_preamble_and_no_entries() {
        let r = parse_region("");
        assert_eq!(r.preamble, "");
        assert_eq!(r.entries.len(), 0);
    }

    // @spec FMT-PARSE-005
    #[test]
    fn parse_region_preamble_preserved_verbatim_in_output() {
        // The preamble slice must be byte-for-byte identical to the input prefix.
        let input = "# My Backlog\n\nSome text here.\n- foo\n";
        let r = parse_region(input);
        // The preamble should be the exact leading substring.
        assert!(input.starts_with(r.preamble));
        assert_eq!(r.preamble, "# My Backlog\n\nSome text here.\n");
    }

    // @spec FMT-PARSE-001
    #[test]
    fn parse_region_hyphen_without_space_is_not_a_bullet() {
        // `"--"` and `"-text"` are not bullet lines.
        let r = parse_region("-not a bullet\n--also not\n");
        assert_eq!(r.preamble, "-not a bullet\n--also not\n");
        assert_eq!(r.entries.len(), 0);
    }

    // @spec FMT-PARSE-001
    #[test]
    fn parse_region_bullet_with_id_markers_parsed_as_bullet() {
        // Real-world bullet with markers is still a bullet line.
        let r = parse_region("- [foo-7k2] [in-progress] [by:jared] Title text here\n");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(
            r.entries[0].bullet,
            "- [foo-7k2] [in-progress] [by:jared] Title text here\n"
        );
    }

    // @spec FMT-PARSE-003
    #[test]
    fn parse_region_last_entry_notes_captured_to_end_of_region() {
        let input = "- task\n  note1\n  note2\n";
        let r = parse_region(input);
        assert_eq!(r.entries[0].notes, "  note1\n  note2\n");
    }

    // @spec FMT-PARSE-003
    #[test]
    fn parse_region_last_entry_trailing_blank_stripped() {
        let input = "- task\n  note\n\n";
        let r = parse_region(input);
        assert_eq!(r.entries[0].notes, "  note\n");
    }

    // @spec FMT-PARSE-004, FMT-PARSE-005
    #[test]
    fn parse_region_preamble_only_no_trailing_newline() {
        // Input without final newline — parse should still work.
        let r = parse_region("preamble only");
        assert_eq!(r.preamble, "preamble only");
        assert_eq!(r.entries.len(), 0);
    }

    // Integration: full backlog.md parse
    // @spec FMT-PARSE-001, FMT-PARSE-004
    #[test]
    fn parse_region_integrated_with_backlog_file() {
        let input = "---\nversion: 1\n---\n# Backlog\n\n- [vat-f1w] First task\n- [vat-g5y] Second task\n  Notes for second.\n---\nfreeform\n";
        let f = BacklogFile::parse(input);
        let r = parse_region(f.parsed());
        assert_eq!(r.preamble, "# Backlog\n\n");
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].bullet, "- [vat-f1w] First task\n");
        assert_eq!(r.entries[0].notes, "");
        assert_eq!(r.entries[1].bullet, "- [vat-g5y] Second task\n");
        assert_eq!(r.entries[1].notes, "  Notes for second.\n");
    }
}
