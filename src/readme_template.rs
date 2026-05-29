// @spec CMD-INIT-006

// Temporary: `vat init` (CMD-INIT-005) is the only consumer and is still a
// stub, so nothing outside tests references this module yet.
#![allow(dead_code)]

/// The `backlog/README.md` template, baked into the binary at compile time.
/// `vat init` renders it once via [`render`] and writes the result; no later
/// command reads or rewrites it (CMD-INIT-007). The literal `{prefix}` token
/// marks where the project's ID prefix is substituted.
pub(crate) const BACKLOG_README_TEMPLATE: &str = include_str!("templates/README.md.tmpl");

/// Render the baked template for a project, substituting every `{prefix}`
/// placeholder with `prefix`. Substitution only — validating `prefix` as a
/// 3-char Crockford base32 string is `vat init`'s job (CMD-INIT-004).
// @spec CMD-INIT-006
pub(crate) fn render(prefix: &str) -> String {
    BACKLOG_README_TEMPLATE.replace("{prefix}", prefix)
}

#[cfg(test)]
mod tests {
    use super::{BACKLOG_README_TEMPLATE, render};

    #[test]
    fn baked_template_is_non_empty() {
        assert!(!BACKLOG_README_TEMPLATE.is_empty());
    }

    // @spec CMD-INIT-006
    #[test]
    fn render_substitutes_every_prefix_placeholder() {
        let out = render("foo");
        assert!(
            !out.contains("{prefix}"),
            "no placeholder should survive rendering: {out:?}"
        );
        assert!(
            out.contains("foo-7k2"),
            "the prefix should appear in the rendered ID examples"
        );
    }

    // @spec CMD-INIT-006
    #[test]
    fn describes_what_vat_is() {
        let out = render("foo");
        assert!(out.contains("VAT"));
        assert!(
            out.to_lowercase().contains("versioned addressable tasks"),
            "README should spell out what VAT stands for"
        );
    }

    // @spec CMD-INIT-006
    #[test]
    fn documents_how_to_obtain_vat() {
        let out = render("foo");
        assert!(
            out.contains("cargo install"),
            "README should say how to obtain vat"
        );
    }

    // @spec CMD-INIT-006
    #[test]
    fn documents_purpose_of_each_backlog_file() {
        let out = render("foo");
        for file in ["backlog.md", "vat.toml", ".used-ids", "items/"] {
            assert!(out.contains(file), "README should document `{file}`");
        }
    }

    // @spec CMD-INIT-006
    #[test]
    fn documents_the_basic_workflow() {
        let out = render("foo");
        for cmd in ["vat sync", "vat start", "vat done"] {
            assert!(out.contains(cmd), "README should describe `{cmd}`");
        }
    }
}
