// @spec FMT-B32-001, FMT-B32-002, FMT-B32-003, FMT-B32-004, FMT-B32-005, FMT-B32-006, FMT-B32-007

use rand::Rng;
use thiserror::Error;

#[allow(dead_code)]
const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

#[allow(dead_code)]
#[derive(Debug, Error, PartialEq)]
pub(crate) enum Base32Error {
    #[error("wrong length: expected {expected}, got {got}")]
    WrongLength { expected: usize, got: usize },
    #[error("invalid character {ch:?} at position {pos}")]
    InvalidChar { ch: char, pos: usize },
}

// @spec FMT-B32-001, FMT-B32-002, FMT-B32-003, FMT-B32-004, FMT-B32-005
#[allow(dead_code)]
pub(crate) fn validate(s: &str, expected_len: usize) -> Result<(), Base32Error> {
    let got = s.chars().count();
    if got != expected_len {
        return Err(Base32Error::WrongLength {
            expected: expected_len,
            got,
        });
    }
    for (pos, ch) in s.chars().enumerate() {
        let lower = ch.to_ascii_lowercase();
        let in_alphabet = u32::from(lower) < 128 && ALPHABET.contains(&(lower as u8));
        if !in_alphabet {
            return Err(Base32Error::InvalidChar { ch, pos });
        }
    }
    Ok(())
}

// @spec FMT-B32-006, FMT-B32-007
#[allow(dead_code)]
pub(crate) fn random(n: usize, rng: &mut impl rand::RngCore) -> String {
    (0..n)
        .map(|_| {
            let idx: u8 = rng.gen_range(0..32_u8);
            ALPHABET[idx as usize] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Base32Error, random, validate};

    // @spec FMT-B32-002
    #[test]
    fn validate_accepts_mixed_case() {
        // "0123456789AbCdEfGhJkMnPqRsTvWxYz" is the full alphabet in mixed case (len=32)
        let mixed = "0123456789AbCdEfGhJkMnPqRsTvWxYz";
        assert_eq!(validate(mixed, 32), Ok(()));
    }

    // @spec FMT-B32-001
    #[test]
    fn validate_accepts_every_alphabet_character() {
        // Uppercase canonical alphabet, length 32
        let upper = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        assert_eq!(validate(upper, 32), Ok(()));
        // Lowercase canonical alphabet, length 32
        let lower = "0123456789abcdefghjkmnpqrstvwxyz";
        assert_eq!(validate(lower, 32), Ok(()));
    }

    // @spec FMT-B32-005
    #[test]
    fn validate_rejects_ambiguous_chars_without_folding() {
        // I, L, O, U (upper and lower) must be rejected as invalid, not folded
        for ch in ['I', 'i', 'L', 'l', 'O', 'o', 'U', 'u'] {
            let s = format!("{ch}0000000");
            let result = validate(&s, 8);
            assert_eq!(
                result,
                Err(Base32Error::InvalidChar { ch, pos: 0 }),
                "expected InvalidChar for {ch:?}"
            );
        }
    }

    // @spec FMT-B32-004
    #[test]
    fn validate_reports_zero_based_char_index_for_invalid_char() {
        // "a😀b" has chars: 'a' at 0, '😀' at 1, 'b' at 2 — all length-3 string
        // '😀' is not in the alphabet; pos should be 1 (char index, not byte index)
        let s = "a\u{1F600}b";
        assert_eq!(s.chars().count(), 3);
        assert_eq!(
            validate(s, 3),
            Err(Base32Error::InvalidChar {
                ch: '\u{1F600}',
                pos: 1
            })
        );
    }

    // @spec FMT-B32-003
    #[test]
    fn validate_returns_wrong_length_before_checking_chars() {
        // String of wrong length with all-invalid characters must still return WrongLength
        let s = "!!@@##$$"; // 8 invalid chars
        assert_eq!(
            validate(s, 5),
            Err(Base32Error::WrongLength { expected: 5, got: 8 })
        );
    }

    // @spec FMT-B32-007
    #[test]
    fn random_with_seeded_rng_is_deterministic() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::from_seed([0u8; 32]);
        assert_eq!(random(3, &mut rng), "da2");
    }

    // @spec FMT-B32-006
    #[test]
    fn random_output_contains_only_canonical_lowercase_chars() {
        use rand::SeedableRng;
        let canonical_lower = "0123456789abcdefghjkmnpqrstvwxyz";
        let mut rng = rand::rngs::StdRng::from_seed([42u8; 32]);
        let output = random(100, &mut rng);
        assert_eq!(output.len(), 100);
        for ch in output.chars() {
            assert!(
                canonical_lower.contains(ch),
                "character {ch:?} is not in the canonical lowercase alphabet"
            );
        }
    }
}
