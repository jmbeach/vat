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

// ---------------------------------------------------------------------------
// Parsed region grammar (FMT-PARSE-001..005)
// ---------------------------------------------------------------------------

/// A single task entry in the parsed region: a bullet line plus all note lines
/// that follow it up to the next bullet or end of the parsed region.
pub(crate) struct TaskEntry<'a> {
    /// The bullet line, including its trailing `\n` (or without it if the file
    /// ends without a newline). Always starts with `"- "`.
    pub(crate) bullet_line: &'a str,
    /// All lines between this bullet and the next bullet (exclusive), verbatim.
    /// May be empty when the bullet is immediately followed by another bullet or
    /// by the end of the parsed region.
    pub(crate) notes: &'a str,
}

/// The structured view of a `backlog.md` parsed region: an optional preamble
/// followed by a sequence of task entries.
pub(crate) struct ParsedRegion<'a> {
    /// Content before the first bullet line; preserved verbatim on re-serialize.
    pub(crate) preamble: &'a str,
    /// Task entries in document order.
    pub(crate) entries: Vec<TaskEntry<'a>>,
}

// @spec FMT-PARSE-001, FMT-PARSE-002
fn is_bullet_line(line: &str) -> bool {
    line.starts_with("- ")
}

impl<'a> ParsedRegion<'a> {
    // @spec FMT-PARSE-001, FMT-PARSE-002, FMT-PARSE-003, FMT-PARSE-004
    pub(crate) fn parse(region: &'a str) -> Self {
        let mut offset = 0usize;
        for line in region.split_inclusive('\n') {
            if is_bullet_line(line) {
                break;
            }
            offset += line.len();
        }
        // `offset` is the byte start of the first bullet, or `region.len()` if
        // the loop exhausted all lines without finding one.
        let preamble = &region[..offset];
        let entries = parse_task_entries(&region[offset..]);
        ParsedRegion { preamble, entries }
    }

    // @spec FMT-PARSE-005
    pub(crate) fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str(self.preamble);
        for entry in &self.entries {
            out.push_str(entry.bullet_line);
            out.push_str(entry.notes);
        }
        out
    }

    /// Serialize the region with the entry at `entry_idx`'s bullet line replaced
    /// by `new_bullet_line`. Notes for every entry, and the preamble, are
    /// preserved verbatim.
    ///
    /// The shared single-bullet-replace emitter for every command that mutates
    /// exactly one bullet in place (`start`, `block`, `unblock`). `done`, which
    /// drops an entry and re-emits others, has its own region walk.
    // @spec FMT-PARSE-005
    pub(crate) fn serialize_with_replaced_bullet(
        &self,
        entry_idx: usize,
        new_bullet_line: &str,
    ) -> String {
        let mut out = String::new();
        out.push_str(self.preamble);
        for (i, entry) in self.entries.iter().enumerate() {
            if i == entry_idx {
                out.push_str(new_bullet_line);
            } else {
                out.push_str(entry.bullet_line);
            }
            out.push_str(entry.notes);
        }
        out
    }
}

fn parse_task_entries(rest: &str) -> Vec<TaskEntry<'_>> {
    let mut entries: Vec<TaskEntry<'_>> = Vec::new();
    let mut offset = 0usize;
    // `(bullet_start, notes_start)` for the entry currently being accumulated.
    // The two offsets are always set and cleared together, so a single Option
    // over the pair makes that invariant explicit.
    let mut in_flight: Option<(usize, usize)> = None;

    for line in rest.split_inclusive('\n') {
        if is_bullet_line(line) {
            if let Some((bs, ns)) = in_flight {
                entries.push(TaskEntry {
                    bullet_line: &rest[bs..ns],
                    notes: &rest[ns..offset],
                });
            }
            in_flight = Some((offset, offset + line.len()));
        }
        offset += line.len();
    }
    if let Some((bs, ns)) = in_flight {
        entries.push(TaskEntry {
            bullet_line: &rest[bs..ns],
            notes: &rest[ns..],
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::{
        BacklogFile, ParsedRegion, SUPPORTED_MAJOR, UnsupportedVersion, check_version,
        parse_frontmatter, split_body,
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
    // ParsedRegion unit tests (FMT-PARSE-001..005)
    // ===================================================================

    // @spec FMT-PARSE-004
    #[test]
    fn region_empty_input_gives_empty_preamble_and_no_entries() {
        let r = ParsedRegion::parse("");
        assert_eq!(r.preamble, "");
        assert_eq!(r.entries.len(), 0);
    }

    // @spec FMT-PARSE-004
    #[test]
    fn region_no_bullets_whole_region_is_preamble() {
        let input = "# Backlog\n\nSome preamble text.\n";
        let r = ParsedRegion::parse(input);
        assert_eq!(r.preamble, input);
        assert_eq!(r.entries.len(), 0);
    }

    // @spec FMT-PARSE-001
    #[test]
    fn region_single_bullet_no_preamble_no_notes() {
        let r = ParsedRegion::parse("- task one\n");
        assert_eq!(r.preamble, "");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet_line, "- task one\n");
        assert_eq!(r.entries[0].notes, "");
    }

    // @spec FMT-PARSE-001, FMT-PARSE-004
    #[test]
    fn region_preamble_then_single_bullet() {
        let r = ParsedRegion::parse("# Backlog\n\n- task one\n");
        assert_eq!(r.preamble, "# Backlog\n\n");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet_line, "- task one\n");
        assert_eq!(r.entries[0].notes, "");
    }

    // @spec FMT-PARSE-003
    #[test]
    fn region_bullet_with_indented_note_lines() {
        let r = ParsedRegion::parse("- task\n  note line\n  another note\n");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet_line, "- task\n");
        assert_eq!(r.entries[0].notes, "  note line\n  another note\n");
    }

    // @spec FMT-PARSE-003
    #[test]
    fn region_blank_line_between_bullets_belongs_to_first_bullet_notes() {
        let r = ParsedRegion::parse("- first\n\n- second\n");
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].bullet_line, "- first\n");
        assert_eq!(r.entries[0].notes, "\n");
        assert_eq!(r.entries[1].bullet_line, "- second\n");
        assert_eq!(r.entries[1].notes, "");
    }

    // @spec FMT-PARSE-003
    #[test]
    fn region_paragraph_at_col0_between_bullets_attaches_to_first() {
        let r = ParsedRegion::parse("- first\nsome text at col 0\n- second\n");
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].notes, "some text at col 0\n");
        assert_eq!(r.entries[1].notes, "");
    }

    // @spec FMT-PARSE-002
    #[test]
    fn region_star_line_at_start_is_preamble_not_bullet() {
        let r = ParsedRegion::parse("* not a task\n- real task\n");
        assert_eq!(r.preamble, "* not a task\n");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet_line, "- real task\n");
    }

    // @spec FMT-PARSE-002
    #[test]
    fn region_plus_line_at_start_is_preamble_not_bullet() {
        let r = ParsedRegion::parse("+ not a task\n- real task\n");
        assert_eq!(r.preamble, "+ not a task\n");
        assert_eq!(r.entries.len(), 1);
    }

    // @spec FMT-PARSE-002
    #[test]
    fn region_star_line_after_bullet_is_note_not_new_entry() {
        let r = ParsedRegion::parse("- task\n* not a bullet\n- second\n");
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].notes, "* not a bullet\n");
    }

    // @spec FMT-PARSE-002
    #[test]
    fn region_plus_line_after_bullet_is_note_not_new_entry() {
        let r = ParsedRegion::parse("- task\n+ not a bullet\n- second\n");
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].notes, "+ not a bullet\n");
    }

    // @spec FMT-PARSE-001
    #[test]
    fn region_indented_dash_space_is_not_a_bullet() {
        let r = ParsedRegion::parse("  - indented bullet\n- real\n");
        assert_eq!(r.preamble, "  - indented bullet\n");
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet_line, "- real\n");
    }

    // @spec FMT-PARSE-001
    #[test]
    fn region_bare_hyphen_without_space_is_not_a_bullet() {
        let r = ParsedRegion::parse("-not a bullet\n- real\n");
        assert_eq!(r.preamble, "-not a bullet\n");
        assert_eq!(r.entries.len(), 1);
    }

    // @spec FMT-PARSE-001
    #[test]
    fn region_bullet_without_trailing_newline_round_trips() {
        let input = "- task without newline";
        let r = ParsedRegion::parse(input);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].bullet_line, "- task without newline");
        assert_eq!(r.entries[0].notes, "");
        assert_eq!(r.serialize(), input);
    }

    // @spec FMT-PARSE-001
    #[test]
    fn region_multi_bullet_last_without_trailing_newline_round_trips() {
        // Only the final entry lacks a trailing newline; the earlier entry must
        // still flush correctly and the last entry's notes must be empty.
        let input = "- a\n- b";
        let r = ParsedRegion::parse(input);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].bullet_line, "- a\n");
        assert_eq!(r.entries[0].notes, "");
        assert_eq!(r.entries[1].bullet_line, "- b");
        assert_eq!(r.entries[1].notes, "");
        assert_eq!(r.serialize(), input);
    }

    // @spec FMT-PARSE-005
    #[test]
    fn region_serialize_round_trips_full_document() {
        let input = "# Backlog\n\nThis is preamble.\n\n- task one\n  note for one\n- task two\n\n";
        let r = ParsedRegion::parse(input);
        assert_eq!(r.serialize(), input);
    }

    // @spec FMT-PARSE-005
    #[test]
    fn region_serialize_preamble_only_round_trips() {
        let input = "# Backlog\n\nNo tasks yet.\n";
        let r = ParsedRegion::parse(input);
        assert_eq!(r.serialize(), input);
    }

    // @spec FMT-PARSE-005
    #[test]
    fn region_serialize_empty_round_trips() {
        let r = ParsedRegion::parse("");
        assert_eq!(r.serialize(), "");
    }

    // @spec FMT-PARSE-001, FMT-PARSE-003, FMT-PARSE-004, FMT-PARSE-005
    #[test]
    fn region_real_backlog_round_trips() {
        let input = concat!(
            "# VAT implementation backlog\n",
            "\n",
            "Tasks to bring VAT from spec to a working binary.\n",
            "\n",
            "- [vat-f1w] [in-progress] [by:claude-routine] Task A (see ./items/vat-f1w.md)\n",
            "- [vat-g5y] [blocked-by:vat-f1w] Task B\n",
            "- [vat-p7d] Task C\n",
        );
        let r = ParsedRegion::parse(input);
        assert_eq!(
            r.preamble,
            "# VAT implementation backlog\n\nTasks to bring VAT from spec to a working binary.\n\n"
        );
        assert_eq!(r.entries.len(), 3);
        assert_eq!(
            r.entries[0].bullet_line,
            "- [vat-f1w] [in-progress] [by:claude-routine] Task A (see ./items/vat-f1w.md)\n"
        );
        assert_eq!(
            r.entries[1].bullet_line,
            "- [vat-g5y] [blocked-by:vat-f1w] Task B\n"
        );
        assert_eq!(r.entries[2].bullet_line, "- [vat-p7d] Task C\n");
        assert_eq!(r.serialize(), input);
    }
}
