// @spec FMT-FM-001, FMT-FM-003, FMT-FM-004

#![allow(dead_code)]

use serde_yaml::{Mapping, Value};

pub(crate) struct Frontmatter {
    present: bool,
    raw_body: String,
    parsed: Mapping,
}

pub(crate) struct ParseResult<'a> {
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

    pub(crate) fn serialize(&self) -> String {
        if !self.present {
            return String::new();
        }
        format!("---\n{}---\n", self.raw_body)
    }
}

// @spec FMT-FM-001, FMT-FM-003, FMT-FM-004
//
// Pre-condition: `input` is LF-normalized per FMT-WS-001. The opening and
// closing delimiters are matched as `"---\n"`, so a CRLF-terminated input
// silently parses as no-frontmatter — which would let a `version: N` file
// from a Windows editor skip the FMT-FM-002 version check. Callers must
// normalize line endings upstream.
pub(crate) fn parse(input: &str) -> ParseResult<'_> {
    if let Some(after_open) = input.strip_prefix("---\n")
        && let Some(raw_body_end) = find_closing_delimiter(after_open)
    {
        let raw_body = &after_open[..raw_body_end];
        let body_start = (input.len() - after_open.len()) + raw_body_end + "---\n".len();
        return ParseResult {
            frontmatter: Frontmatter {
                present: true,
                raw_body: raw_body.to_string(),
                parsed: parse_yaml_mapping(raw_body),
            },
            body: &input[body_start..],
        };
    }
    ParseResult {
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
    use super::parse;

    // @spec FMT-FM-001
    #[test]
    fn detects_frontmatter_when_file_begins_with_delimiter() {
        let input = "---\nversion: 1\n---\nbody\n";
        let r = parse(input);
        assert!(r.frontmatter.present());
        assert_eq!(r.body, "body\n");
    }

    // @spec FMT-FM-001
    #[test]
    fn no_frontmatter_when_file_does_not_begin_with_delimiter() {
        let input = "hello\n---\nversion: 1\n---\n";
        let r = parse(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn unclosed_frontmatter_is_treated_as_no_frontmatter() {
        let input = "---\nversion: 1\nno closing delimiter\n";
        let r = parse(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn empty_frontmatter_is_recognized() {
        let input = "---\n---\nbody\n";
        let r = parse(input);
        assert!(r.frontmatter.present());
        assert_eq!(r.body, "body\n");
    }

    // @spec FMT-FM-001
    #[test]
    fn delimiter_with_trailing_whitespace_is_not_recognized() {
        let input = "--- \nversion: 1\n---\n";
        let r = parse(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn line_starting_with_three_dashes_but_not_solely_is_not_a_closing_delimiter() {
        let input = "---\nversion: 1\n---foo\n---\nbody\n";
        let r = parse(input);
        assert!(r.frontmatter.present());
        assert_eq!(r.body, "body\n");
    }

    // @spec FMT-FM-001
    #[test]
    fn closing_delimiter_without_trailing_newline_is_not_recognized() {
        let input = "---\nversion: 1\n---";
        let r = parse(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn empty_frontmatter_without_trailing_newline_is_not_recognized() {
        let input = "---\n---";
        let r = parse(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn opening_delimiter_alone_at_eof_is_no_frontmatter() {
        let input = "---\n";
        let r = parse(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, input);
    }

    // @spec FMT-FM-001
    #[test]
    fn line_scan_wins_over_yaml_quoted_multiline() {
        // The double-quoted YAML scalar would semantically span three lines,
        // but the parser's line-based scan terminates frontmatter at the first
        // unindented `---` regardless.
        let input = "---\nweird: \"line1\n---\nline3\"\n---\nbody\n";
        let r = parse(input);
        assert!(r.frontmatter.present());
        assert_eq!(r.body, "line3\"\n---\nbody\n");
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    // @spec FMT-FM-003
    #[test]
    fn missing_version_key_defaults_to_1() {
        let input = "---\nother: foo\n---\n";
        let r = parse(input);
        assert_eq!(r.frontmatter.version(), 1);
    }

    // @spec FMT-FM-003
    #[test]
    fn absent_frontmatter_defaults_to_version_1() {
        let input = "just body\n";
        let r = parse(input);
        assert_eq!(r.frontmatter.version(), 1);
    }

    // @spec FMT-FM-003
    #[test]
    fn empty_frontmatter_defaults_to_version_1() {
        let input = "---\n---\nbody\n";
        let r = parse(input);
        assert_eq!(r.frontmatter.version(), 1);
    }

    // @spec FMT-FM-003
    #[test]
    fn non_integer_version_value_defaults_to_1() {
        let input = "---\nversion: \"two\"\n---\n";
        let r = parse(input);
        assert_eq!(r.frontmatter.version(), 1);
    }

    #[test]
    fn version_key_is_exposed_as_u64() {
        let input = "---\nversion: 2\n---\n";
        let r = parse(input);
        assert_eq!(r.frontmatter.version(), 2);
    }

    // @spec FMT-FM-004
    #[test]
    fn serialize_preserves_unknown_keys_verbatim() {
        let input = "---\nversion: 1\ncustom: hello\nother: value\n---\nbody\n";
        let r = parse(input);
        assert_eq!(
            r.frontmatter.serialize(),
            "---\nversion: 1\ncustom: hello\nother: value\n---\n"
        );
    }

    // @spec FMT-FM-004
    #[test]
    fn round_trip_preserves_comments_and_blank_lines() {
        let input = "---\n# leading comment\nversion: 1\n\nbar: 42\n---\nbody\n";
        let r = parse(input);
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    // @spec FMT-FM-004
    #[test]
    fn round_trip_preserves_nested_yaml_verbatim() {
        let input = "---\nnested:\n  key: value\n  list:\n    - one\n    - two\n---\nbody\n";
        let r = parse(input);
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    // @spec FMT-FM-004
    #[test]
    fn round_trip_preserves_multibyte_unicode_in_keys_and_values() {
        let input = "---\nкл: значение\nemoji: \u{1F31F}\n---\nbody\n";
        let r = parse(input);
        assert!(r.frontmatter.present());
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    // @spec FMT-FM-004
    #[test]
    fn serialize_when_absent_returns_empty_string() {
        let input = "no frontmatter here\n";
        let r = parse(input);
        assert_eq!(r.frontmatter.serialize(), "");
    }

    #[test]
    fn malformed_yaml_still_round_trips_with_default_version() {
        let input = "---\nkey: value\n  bad: indent under scalar\n---\nbody\n";
        let r = parse(input);
        assert_eq!(r.frontmatter.version(), 1);
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    #[test]
    fn non_mapping_yaml_round_trips_with_default_version() {
        let input = "---\n- list\n- items\n---\nbody\n";
        let r = parse(input);
        assert_eq!(r.frontmatter.version(), 1);
        let recombined = format!("{}{}", r.frontmatter.serialize(), r.body);
        assert_eq!(recombined, input);
    }

    #[test]
    fn empty_input_is_no_frontmatter() {
        let input = "";
        let r = parse(input);
        assert!(!r.frontmatter.present());
        assert_eq!(r.body, "");
    }
}
