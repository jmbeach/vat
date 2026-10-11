//! The `vat sync` title-length rule (SYNC-LEN-*): what text of a new bullet
//! counts toward `[sync].max_title_length`, and the default limit.

/// Limit applied when `vat.toml` sets no `[sync].max_title_length` (FMT-CFG-004).
pub(crate) const DEFAULT_MAX_TITLE_LENGTH: usize = 120;

/// The text of a raw bullet line that counts toward the title-length limit.
pub(crate) fn measured_title(bullet_line: &str) -> String {
    let _ = bullet_line;
    todo!("SYNC-LEN-001")
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
