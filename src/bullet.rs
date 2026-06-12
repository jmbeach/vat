// @spec FMT-MARK-001, FMT-MARK-002, FMT-MARK-003, FMT-MARK-004, FMT-MARK-005, FMT-MARK-006, FMT-MARK-007, FMT-PARSE-006, FMT-WS-002

#![allow(dead_code)]

use crate::base32;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum BulletError {
    #[error("bullet has no title after markers")]
    EmptyTitle,
}

/// A parsed bullet line with markers and title extracted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bullet {
    pub(crate) id: Option<String>,
    pub(crate) in_progress: bool,
    pub(crate) by: Option<String>,
    pub(crate) blocked_by: Option<String>,
    pub(crate) title: String,
}

impl Bullet {
    // @spec FMT-MARK-001, FMT-MARK-002, FMT-MARK-003, FMT-MARK-006, FMT-MARK-007, FMT-PARSE-006
    //
    // Greedy front-loaded marker parser. Strips "- " prefix and trailing
    // newline then matches markers left-to-right: as soon as a token does not
    // match any known pattern the rest becomes the title. FMT-MARK-007: a
    // second [blocked-by:...] is silently skipped (not treated as unknown)
    // so parsing continues past it and only the first is kept.
    pub(crate) fn parse(bullet_line: &str) -> Result<Self, BulletError> {
        let body = bullet_line.strip_prefix("- ").unwrap_or(bullet_line);
        let body = body.trim_end_matches('\n').trim_end_matches('\r');

        let mut rest = body;
        let mut id: Option<String> = None;
        let mut in_progress = false;
        let mut by: Option<String> = None;
        let mut blocked_by: Option<String> = None;

        loop {
            rest = rest.trim_start_matches(' ');

            if !rest.starts_with('[') {
                break;
            }

            // FMT-MARK-001: [<3-char-base32>-<3-char-base32>]
            if id.is_none() {
                if let Some((mid, after)) = try_id(rest) {
                    id = Some(mid);
                    rest = after;
                    continue;
                }
            }

            // [in-progress] literal
            if !in_progress {
                if let Some(after) = rest.strip_prefix("[in-progress]") {
                    in_progress = true;
                    rest = after;
                    continue;
                }
            }

            // FMT-MARK-002: [by:<name>]
            if by.is_none() {
                if let Some((mby, after)) = try_by(rest) {
                    by = Some(mby);
                    rest = after;
                    continue;
                }
            }

            // FMT-MARK-003 + FMT-MARK-007: [blocked-by:<id>]
            if let Some((mblocker, after)) = try_blocked_by(rest) {
                if blocked_by.is_none() {
                    blocked_by = Some(mblocker);
                }
                rest = after;
                continue;
            }

            // FMT-MARK-006: unrecognized [...] token — title starts here
            break;
        }

        // FMT-WS-002: strip trailing whitespace from title
        let title = rest.trim_end().to_string();

        if title.is_empty() {
            return Err(BulletError::EmptyTitle);
        }

        Ok(Bullet {
            id,
            in_progress,
            by,
            blocked_by,
            title,
        })
    }

    // @spec FMT-MARK-004, FMT-MARK-005, FMT-WS-002
    pub(crate) fn serialize(&self) -> String {
        let mut tokens: Vec<String> = Vec::new();
        if let Some(id) = &self.id {
            tokens.push(format!("[{id}]"));
        }
        if self.in_progress {
            tokens.push("[in-progress]".to_string());
        }
        if let Some(by) = &self.by {
            tokens.push(format!("[by:{by}]"));
        }
        if let Some(blocked_by) = &self.blocked_by {
            tokens.push(format!("[blocked-by:{blocked_by}]"));
        }
        tokens.push(self.title.trim_end().to_string());
        format!("- {}\n", tokens.join(" "))
    }
}

// FMT-MARK-001
fn try_id(s: &str) -> Option<(String, &str)> {
    debug_assert!(s.starts_with('['));
    let close = s[1..].find(']')? + 1;
    let inner = &s[1..close];
    if inner.len() != 7 || inner.as_bytes().get(3) != Some(&b'-') {
        return None;
    }
    let prefix = &inner[..3];
    let suffix = &inner[4..];
    base32::validate(prefix, 3).ok()?;
    base32::validate(suffix, 3).ok()?;
    Some((inner.to_lowercase(), &s[close + 1..]))
}

// FMT-MARK-002
fn try_by(s: &str) -> Option<(String, &str)> {
    let after_prefix = s.strip_prefix("[by:")?;
    let close = after_prefix.find(']')?;
    let name = &after_prefix[..close];
    if name.is_empty() {
        return None;
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-')
    {
        return None;
    }
    Some((name.to_string(), &after_prefix[close + 1..]))
}

// FMT-MARK-003
fn try_blocked_by(s: &str) -> Option<(String, &str)> {
    let after_prefix = s.strip_prefix("[blocked-by:")?;
    let close = after_prefix.find(']')?;
    let id_str = &after_prefix[..close];
    if id_str.len() != 7 || id_str.as_bytes().get(3) != Some(&b'-') {
        return None;
    }
    let prefix = &id_str[..3];
    let suffix = &id_str[4..];
    base32::validate(prefix, 3).ok()?;
    base32::validate(suffix, 3).ok()?;
    Some((id_str.to_lowercase(), &after_prefix[close + 1..]))
}

#[cfg(test)]
mod tests {
    use super::{Bullet, BulletError};

    fn b(
        id: Option<&str>,
        in_progress: bool,
        by: Option<&str>,
        blocked_by: Option<&str>,
        title: &str,
    ) -> Bullet {
        Bullet {
            id: id.map(str::to_string),
            in_progress,
            by: by.map(str::to_string),
            blocked_by: blocked_by.map(str::to_string),
            title: title.to_string(),
        }
    }

    // ===================================================================
    // FMT-MARK-001 — id marker
    // ===================================================================

    // @spec FMT-MARK-001
    #[test]
    fn parse_extracts_valid_id_marker() {
        let bullet = Bullet::parse("- [vat-g5y] My task\n").unwrap();
        assert_eq!(bullet.id, Some("vat-g5y".to_string()));
        assert_eq!(bullet.title, "My task");
    }

    // @spec FMT-MARK-001
    #[test]
    fn parse_id_marker_normalizes_to_lowercase() {
        let bullet = Bullet::parse("- [VAT-G5Y] My task\n").unwrap();
        assert_eq!(bullet.id, Some("vat-g5y".to_string()));
    }

    // @spec FMT-MARK-001
    #[test]
    fn parse_id_wrong_length_not_recognized() {
        // "abcde-fg" — prefix is 5 chars, not 3
        let bullet = Bullet::parse("- [abcde-fg] title\n").unwrap();
        assert_eq!(bullet.id, None);
        assert_eq!(bullet.title, "[abcde-fg] title");
    }

    // @spec FMT-MARK-001
    #[test]
    fn parse_id_with_ambiguous_base32_chars_not_recognized() {
        // 'o' and 'i' are excluded from Crockford base32
        let bullet = Bullet::parse("- [oii-abc] title\n").unwrap();
        assert_eq!(bullet.id, None);
        assert_eq!(bullet.title, "[oii-abc] title");
    }

    // @spec FMT-MARK-001
    #[test]
    fn parse_id_without_dash_separator_not_recognized() {
        let bullet = Bullet::parse("- [vatg5yy] title\n").unwrap();
        assert_eq!(bullet.id, None);
        assert_eq!(bullet.title, "[vatg5yy] title");
    }

    // ===================================================================
    // FMT-MARK-002 — [by:<name>]
    // ===================================================================

    // @spec FMT-MARK-002
    #[test]
    fn parse_extracts_by_marker() {
        let bullet = Bullet::parse("- [by:jared] My task\n").unwrap();
        assert_eq!(bullet.by, Some("jared".to_string()));
        assert_eq!(bullet.title, "My task");
    }

    // @spec FMT-MARK-002
    #[test]
    fn parse_by_accepts_alphanumeric_dot_underscore_hyphen() {
        let bullet = Bullet::parse("- [by:john.doe_2-dev] title\n").unwrap();
        assert_eq!(bullet.by, Some("john.doe_2-dev".to_string()));
    }

    // @spec FMT-MARK-002
    #[test]
    fn parse_by_with_space_in_name_not_recognized() {
        let bullet = Bullet::parse("- [by:hello world] title\n").unwrap();
        assert_eq!(bullet.by, None);
        assert_eq!(bullet.title, "[by:hello world] title");
    }

    // @spec FMT-MARK-002
    #[test]
    fn parse_by_with_empty_name_not_recognized() {
        let bullet = Bullet::parse("- [by:] title\n").unwrap();
        assert_eq!(bullet.by, None);
        assert_eq!(bullet.title, "[by:] title");
    }

    // ===================================================================
    // FMT-MARK-003 — [blocked-by:<id>]
    // ===================================================================

    // @spec FMT-MARK-003
    #[test]
    fn parse_extracts_blocked_by_marker() {
        let bullet = Bullet::parse("- [blocked-by:vat-f1w] title\n").unwrap();
        assert_eq!(bullet.blocked_by, Some("vat-f1w".to_string()));
    }

    // @spec FMT-MARK-003
    #[test]
    fn parse_blocked_by_normalizes_id_to_lowercase() {
        let bullet = Bullet::parse("- [blocked-by:VAT-F1W] title\n").unwrap();
        assert_eq!(bullet.blocked_by, Some("vat-f1w".to_string()));
    }

    // @spec FMT-MARK-003
    #[test]
    fn parse_blocked_by_with_invalid_id_not_recognized() {
        // "invalid-id" — prefix is 7 chars, not 3
        let bullet = Bullet::parse("- [blocked-by:invalid-id] title\n").unwrap();
        assert_eq!(bullet.blocked_by, None);
        assert_eq!(bullet.title, "[blocked-by:invalid-id] title");
    }

    // ===================================================================
    // FMT-MARK-004 — canonical serialization order
    // ===================================================================

    // @spec FMT-MARK-004
    #[test]
    fn serialize_emits_markers_in_canonical_order() {
        let bullet = b(
            Some("vat-g5y"),
            true,
            Some("jared"),
            Some("vat-f1w"),
            "My task",
        );
        assert_eq!(
            bullet.serialize(),
            "- [vat-g5y] [in-progress] [by:jared] [blocked-by:vat-f1w] My task\n"
        );
    }

    // @spec FMT-MARK-004
    #[test]
    fn parse_out_of_order_markers_serialize_normalizes_to_canonical_order() {
        // Input has in-progress before id
        let bullet = Bullet::parse("- [in-progress] [vat-g5y] My task\n").unwrap();
        assert_eq!(bullet.in_progress, true);
        assert_eq!(bullet.id, Some("vat-g5y".to_string()));
        assert_eq!(
            bullet.serialize(),
            "- [vat-g5y] [in-progress] My task\n"
        );
    }

    // @spec FMT-MARK-004
    #[test]
    fn serialize_omits_absent_markers() {
        let bullet = b(Some("vat-g5y"), false, None, None, "title");
        assert_eq!(bullet.serialize(), "- [vat-g5y] title\n");
    }

    // ===================================================================
    // FMT-MARK-005 — single space between markers
    // ===================================================================

    // @spec FMT-MARK-005
    #[test]
    fn serialize_single_space_between_adjacent_markers() {
        let bullet = b(Some("vat-g5y"), true, None, None, "title");
        let s = bullet.serialize();
        assert_eq!(s, "- [vat-g5y] [in-progress] title\n");
        assert!(
            s.contains("] ["),
            "should have exactly one space between adjacent markers"
        );
        assert!(!s.contains("]  ["), "must not have double space between markers");
    }

    // @spec FMT-MARK-005
    #[test]
    fn parse_double_spaces_between_markers_normalized_on_serialize() {
        let bullet = Bullet::parse("- [vat-g5y]  [in-progress]  title\n").unwrap();
        assert_eq!(bullet.serialize(), "- [vat-g5y] [in-progress] title\n");
    }

    // ===================================================================
    // FMT-MARK-006 — unknown [...] → title
    // ===================================================================

    // @spec FMT-MARK-006
    #[test]
    fn parse_unknown_marker_at_front_becomes_title() {
        let bullet = Bullet::parse("- [agent-ready] some task\n").unwrap();
        assert_eq!(bullet.id, None);
        assert_eq!(bullet.title, "[agent-ready] some task");
    }

    // @spec FMT-MARK-006
    #[test]
    fn parse_unknown_marker_after_known_stops_and_becomes_title() {
        let bullet = Bullet::parse("- [vat-g5y] [unknown] task title\n").unwrap();
        assert_eq!(bullet.id, Some("vat-g5y".to_string()));
        assert_eq!(bullet.title, "[unknown] task title");
    }

    // @spec FMT-MARK-006
    #[test]
    fn parse_todo_marker_is_not_known() {
        // [TODO] — 4-char inner, not a valid id format, not a known literal
        let bullet = Bullet::parse("- [TODO] do something\n").unwrap();
        assert_eq!(bullet.id, None);
        assert_eq!(bullet.title, "[TODO] do something");
    }

    // ===================================================================
    // FMT-MARK-007 — only first [blocked-by:...] preserved
    // ===================================================================

    // @spec FMT-MARK-007
    #[test]
    fn parse_multiple_blocked_by_keeps_only_first() {
        let bullet =
            Bullet::parse("- [vat-g5y] [blocked-by:vat-f1w] [blocked-by:vat-h8x] title\n")
                .unwrap();
        assert_eq!(bullet.blocked_by, Some("vat-f1w".to_string()));
    }

    // @spec FMT-MARK-007
    #[test]
    fn parse_multiple_blocked_by_continues_parsing_past_second() {
        // After the second [blocked-by:...] there is still title text; make
        // sure the parser doesn't stop at the second occurrence.
        let bullet =
            Bullet::parse("- [blocked-by:vat-f1w] [blocked-by:vat-h8x] My task\n").unwrap();
        assert_eq!(bullet.blocked_by, Some("vat-f1w".to_string()));
        assert_eq!(bullet.title, "My task");
    }

    // ===================================================================
    // FMT-PARSE-006 — empty title
    // ===================================================================

    // @spec FMT-PARSE-006
    #[test]
    fn parse_bullet_with_no_title_returns_empty_title_error() {
        assert_eq!(
            Bullet::parse("- [vat-g5y]\n"),
            Err(BulletError::EmptyTitle)
        );
    }

    // @spec FMT-PARSE-006
    #[test]
    fn parse_bullet_with_whitespace_only_title_returns_empty_title_error() {
        assert_eq!(
            Bullet::parse("- [vat-g5y]   \n"),
            Err(BulletError::EmptyTitle)
        );
    }

    // @spec FMT-PARSE-006
    #[test]
    fn parse_bare_bullet_no_markers_no_title_returns_empty_title_error() {
        assert_eq!(Bullet::parse("- \n"), Err(BulletError::EmptyTitle));
    }

    // @spec FMT-PARSE-006
    #[test]
    fn parse_empty_body_after_prefix_returns_empty_title_error() {
        assert_eq!(Bullet::parse("- "), Err(BulletError::EmptyTitle));
    }

    // ===================================================================
    // FMT-WS-002 — trailing whitespace stripped on serialize
    // ===================================================================

    // @spec FMT-WS-002
    #[test]
    fn serialize_strips_trailing_whitespace_from_title() {
        let bullet = Bullet {
            id: None,
            in_progress: false,
            by: None,
            blocked_by: None,
            title: "My task   ".to_string(),
        };
        assert_eq!(bullet.serialize(), "- My task\n");
    }

    // @spec FMT-WS-002
    #[test]
    fn serialize_strips_trailing_whitespace_when_markers_present() {
        let bullet = b(Some("vat-g5y"), false, None, None, "title  ");
        assert_eq!(bullet.serialize(), "- [vat-g5y] title\n");
    }

    // ===================================================================
    // Round-trip: tokenize → serialize on canonical-form bullets
    // ===================================================================

    // @spec FMT-MARK-001, FMT-MARK-004, FMT-MARK-005
    #[test]
    fn round_trip_full_canonical_bullet() {
        let line =
            "- [vat-g5y] [in-progress] [by:jared] [blocked-by:vat-f1w] My task\n";
        let bullet = Bullet::parse(line).unwrap();
        assert_eq!(bullet.serialize(), line);
    }

    // @spec FMT-MARK-001
    #[test]
    fn round_trip_id_only() {
        let line = "- [vat-g5y] My task\n";
        assert_eq!(Bullet::parse(line).unwrap().serialize(), line);
    }

    // @spec FMT-MARK-004
    #[test]
    fn round_trip_no_markers() {
        let line = "- My plain task\n";
        assert_eq!(Bullet::parse(line).unwrap().serialize(), line);
    }

    // @spec FMT-MARK-001, FMT-MARK-006
    #[test]
    fn round_trip_real_backlog_bullet_with_unknown_marker() {
        // [agent-ready] is unknown to VAT and becomes part of the title.
        let line =
            "- [vat-g5y] [agent-ready] Bullet line tokenizer (markers + title)\n";
        let bullet = Bullet::parse(line).unwrap();
        assert_eq!(bullet.id, Some("vat-g5y".to_string()));
        assert_eq!(
            bullet.title,
            "[agent-ready] Bullet line tokenizer (markers + title)"
        );
        assert_eq!(bullet.serialize(), line);
    }

    // @spec FMT-MARK-001, FMT-MARK-002
    #[test]
    fn round_trip_id_and_by_only() {
        let line = "- [vat-g5y] [by:alice] Do the thing\n";
        assert_eq!(Bullet::parse(line).unwrap().serialize(), line);
    }

    // @spec FMT-MARK-003, FMT-MARK-007
    #[test]
    fn round_trip_blocked_only() {
        let line = "- [blocked-by:vat-f1w] Waiting task\n";
        assert_eq!(Bullet::parse(line).unwrap().serialize(), line);
    }

    // ===================================================================
    // Misc
    // ===================================================================

    #[test]
    fn parse_without_bullet_prefix_uses_full_line_as_body() {
        let bullet = Bullet::parse("[vat-g5y] My task\n").unwrap();
        assert_eq!(bullet.id, Some("vat-g5y".to_string()));
        assert_eq!(bullet.title, "My task");
    }

    #[test]
    fn parse_title_only_no_markers() {
        let bullet = Bullet::parse("- My task without any markers\n").unwrap();
        assert_eq!(bullet.id, None);
        assert_eq!(bullet.in_progress, false);
        assert_eq!(bullet.by, None);
        assert_eq!(bullet.blocked_by, None);
        assert_eq!(bullet.title, "My task without any markers");
    }

    #[test]
    fn parse_in_progress_alone() {
        let bullet = Bullet::parse("- [in-progress] A task\n").unwrap();
        assert_eq!(bullet.id, None);
        assert_eq!(bullet.in_progress, true);
        assert_eq!(bullet.title, "A task");
    }

    #[test]
    fn parse_all_markers_in_canonical_order() {
        let line =
            "- [vat-g5y] [in-progress] [by:alice] [blocked-by:vat-f1w] Do the thing\n";
        let bullet = Bullet::parse(line).unwrap();
        assert_eq!(bullet.id, Some("vat-g5y".to_string()));
        assert_eq!(bullet.in_progress, true);
        assert_eq!(bullet.by, Some("alice".to_string()));
        assert_eq!(bullet.blocked_by, Some("vat-f1w".to_string()));
        assert_eq!(bullet.title, "Do the thing");
    }

    #[test]
    fn serialize_title_only() {
        let bullet = b(None, false, None, None, "some task");
        assert_eq!(bullet.serialize(), "- some task\n");
    }
}
