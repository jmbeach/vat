// @spec FMT-PFX-001, FMT-PFX-002, FMT-PFX-003, FMT-PFX-004

#![allow(dead_code)]

use crate::base32::Base32Error;

/// A project-ID prefix is always exactly this many characters.
pub(crate) const PREFIX_LEN: usize = 3;

/// Validate a user-chosen project-ID prefix.
///
/// Unlike the auto-generated suffix — which uses the Crockford base32 validator
/// (`base32::validate`) to keep random IDs readable by excluding the ambiguous
/// glyphs `i`/`l`/`o`/`u` — the prefix is chosen *once* by a human and never
/// regenerated. The readability constraint is therefore the wrong tool: it
/// needlessly rejects natural choices like `lib`, `ui0`, `io`, or `sql`. This
/// validator instead accepts any 3-character ASCII *alphanumeric* prefix
/// (letters `a`–`z` case-insensitive, digits `0`–`9`).
///
/// It still rejects the `-` segment separator, whitespace, and the `[`/`]`
/// bracket characters, so a `[<prefix>-<suffix>]` ID token continues to parse
/// unambiguously (`is_ascii_alphanumeric` excludes all of these).
///
/// Errors reuse [`Base32Error`] so callers (`ProjectConfig`, tombstone, bullet)
/// keep a single error type and the existing exit-code classification and
/// caret-rendering paths apply unchanged.
// @spec FMT-PFX-001, FMT-PFX-002, FMT-PFX-003
pub(crate) fn validate(s: &str) -> Result<(), Base32Error> {
    let got = s.chars().count();
    if got != PREFIX_LEN {
        return Err(Base32Error::WrongLength {
            expected: PREFIX_LEN,
            got,
        });
    }
    for (pos, ch) in s.chars().enumerate() {
        if !ch.is_ascii_alphanumeric() {
            return Err(Base32Error::InvalidChar { ch, pos });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    use crate::base32::Base32Error;

    // @spec FMT-PFX-001
    #[test]
    fn accepts_lowercase_letters() {
        assert_eq!(validate("lib"), Ok(()));
        assert_eq!(validate("sql"), Ok(()));
    }

    // @spec FMT-PFX-001
    #[test]
    fn accepts_ambiguous_letters_rejected_by_crockford() {
        // 'i', 'l', 'o', 'u' are excluded from Crockford base32 but are fine
        // for a human-chosen prefix.
        for s in ["lib", "iou", "oil", "lou"] {
            assert_eq!(validate(s), Ok(()), "prefix {s:?} should be accepted");
        }
    }

    // @spec FMT-PFX-001
    #[test]
    fn accepts_digits_and_mixed_alphanumeric() {
        assert_eq!(validate("ui0"), Ok(()));
        assert_eq!(validate("123"), Ok(()));
        assert_eq!(validate("a1b"), Ok(()));
    }

    // @spec FMT-PFX-004
    #[test]
    fn accepts_uppercase_for_caller_to_normalize() {
        // The validator is case-insensitive; lowercasing is the caller's job.
        assert_eq!(validate("LIB"), Ok(()));
        assert_eq!(validate("Ui0"), Ok(()));
    }

    // @spec FMT-PFX-002
    #[test]
    fn rejects_wrong_length() {
        assert_eq!(
            validate("ab"),
            Err(Base32Error::WrongLength {
                expected: 3,
                got: 2,
            })
        );
        assert_eq!(
            validate("abcd"),
            Err(Base32Error::WrongLength {
                expected: 3,
                got: 4,
            })
        );
        assert_eq!(
            validate(""),
            Err(Base32Error::WrongLength {
                expected: 3,
                got: 0,
            })
        );
    }

    // @spec FMT-PFX-002
    #[test]
    fn reports_length_before_checking_chars() {
        // A too-long all-invalid string surfaces WrongLength, not InvalidChar.
        assert_eq!(
            validate("-- -"),
            Err(Base32Error::WrongLength {
                expected: 3,
                got: 4,
            })
        );
    }

    // @spec FMT-PFX-003
    #[test]
    fn rejects_separator_whitespace_and_brackets() {
        // These are exactly the characters that would break `[<prefix>-<suffix>]`
        // tokenization if they were allowed in a prefix.
        for (s, ch, pos) in [
            ("a-b", '-', 1),
            ("a b", ' ', 1),
            ("a\tb", '\t', 1),
            ("[ab", '[', 0),
            ("ab]", ']', 2),
        ] {
            assert_eq!(
                validate(s),
                Err(Base32Error::InvalidChar { ch, pos }),
                "prefix {s:?} should be rejected at {ch:?}"
            );
        }
    }

    // @spec FMT-PFX-003
    #[test]
    fn reports_zero_based_char_index_for_non_ascii() {
        let s = "a\u{1F600}b";
        assert_eq!(s.chars().count(), 3);
        assert_eq!(
            validate(s),
            Err(Base32Error::InvalidChar {
                ch: '\u{1F600}',
                pos: 1,
            })
        );
    }
}
