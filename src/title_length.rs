//! The `vat sync` title-length rule (SYNC-LEN-*): what text of a new bullet
//! counts toward `[sync].max_title_length`, and the default limit.

/// Limit applied when `vat.toml` sets no `[sync].max_title_length` (FMT-CFG-004).
pub(crate) const DEFAULT_MAX_TITLE_LENGTH: usize = 120;

/// The text of a raw bullet line that counts toward the title-length limit.
///
/// Drops the leading `-` bullet marker, every `[...]` span (recognized marker
/// or not), and a `(see ./items/<anything>.md)` pointer followed only by
/// whitespace, then trims. Deliberately independent of the marker parser so
/// the binary and the skill's prose procedure count identically.
// @spec SYNC-LEN-001
pub(crate) fn measured_title(bullet_line: &str) -> String {
    let body = bullet_line.trim_start();
    let body = body.strip_prefix('-').unwrap_or(body);

    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(open) = rest.find('[') {
        match rest[open..].find(']') {
            Some(close) => {
                out.push_str(&rest[..open]);
                rest = &rest[open + close + 1..];
            }
            // An unmatched `[` is ordinary text.
            None => break,
        }
    }
    out.push_str(rest);

    let trimmed = out.trim_end();
    let trimmed = strip_pointer(trimmed).unwrap_or(trimmed);
    trimmed.trim().to_owned()
}

/// `s` without a trailing `(see ./items/<anything>.md)`, if it ends with one.
fn strip_pointer(s: &str) -> Option<&str> {
    let without_close = s.strip_suffix(".md)")?;
    let open = without_close.rfind("(see ./items/")?;
    Some(&s[..open])
}

#[cfg(test)]
mod tests {
    use super::measured_title;

    // @spec SYNC-LEN-001
    #[test]
    fn measured_title_drops_bullet_marker_and_trims() {
        assert_eq!(measured_title("- Fix sync"), "Fix sync");
        assert_eq!(measured_title("-   Fix sync   "), "Fix sync");
    }

    // @spec SYNC-LEN-001
    #[test]
    fn measured_title_drops_every_bracketed_span() {
        // Recognized markers and unknown tags alike are removed.
        assert_eq!(
            measured_title("- [vat-7k2] [in-progress] [by:jared] [blocked-by:vat-9hf] Fix sync"),
            "Fix sync"
        );
        assert_eq!(measured_title("- [TODO] [P1] Fix sync"), "Fix sync");
        assert_eq!(measured_title("- [ ] Fix sync"), "Fix sync");
    }

    // @spec SYNC-LEN-001
    #[test]
    fn measured_title_drops_mid_line_tags_keeping_interior_whitespace() {
        assert_eq!(measured_title("- Fix [P1] sync"), "Fix  sync");
    }

    // @spec SYNC-LEN-001
    #[test]
    fn measured_title_keeps_unmatched_open_bracket() {
        assert_eq!(measured_title("- Fix [sync"), "Fix [sync");
    }

    // @spec SYNC-LEN-001
    #[test]
    fn measured_title_keeps_link_target_of_markdown_link() {
        assert_eq!(
            measured_title("- See [docs](https://example.com/a)"),
            "See (https://example.com/a)"
        );
    }

    // @spec SYNC-LEN-001
    #[test]
    fn measured_title_drops_pointer_suffix_for_any_id() {
        assert_eq!(
            measured_title("- Fix sync (see ./items/vat-7k2.md)"),
            "Fix sync"
        );
        // The pointer need not match the bullet's own id (a new bullet has none).
        assert_eq!(
            measured_title("- [TODO] [vat-7k2] Fix sync (see ./items/vat-7k2.md)"),
            "Fix sync"
        );
    }

    // @spec SYNC-LEN-001
    #[test]
    fn measured_title_drops_pointer_followed_only_by_whitespace() {
        assert_eq!(
            measured_title("- Fix sync (see ./items/vat-7k2.md)   "),
            "Fix sync"
        );
    }

    // @spec SYNC-LEN-001
    #[test]
    fn measured_title_keeps_pointer_text_that_is_not_at_the_end() {
        assert_eq!(
            measured_title("- Fix (see ./items/vat-7k2.md) later"),
            "Fix (see ./items/vat-7k2.md) later"
        );
    }

    // @spec SYNC-LEN-001
    #[test]
    fn measured_title_of_tag_only_bullet_is_empty() {
        assert_eq!(measured_title("- [TODO]"), "");
    }
}
