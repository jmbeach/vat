// @spec FMT-WS-001

// Temporary: the IO layer has no callers yet; remove once a command wires it in.
#![allow(dead_code)]

use std::fs;
use std::io;
use std::path::Path;

// @spec FMT-WS-001
pub(crate) fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let raw = fs::read_to_string(path)?;
    Ok(normalize_line_endings(&raw))
}

pub(crate) fn write(path: impl AsRef<Path>, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}

fn normalize_line_endings(input: &str) -> String {
    if !input.contains('\r') {
        return input.to_string();
    }
    // CR and LF are ASCII, so byte-level scanning is safe across UTF-8 sequences.
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\r' {
            out.push(b'\n');
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i += 2;
            } else {
                i += 1;
            }
        } else {
            out.push(b);
            i += 1;
        }
    }
    // Safe: input was valid UTF-8, and we only ever replaced CR/CRLF with LF
    // (all ASCII), which preserves UTF-8 validity.
    String::from_utf8(out).expect("CR/LF substitution preserves UTF-8 validity")
}

#[cfg(test)]
mod tests {
    use super::{normalize_line_endings, read_to_string, write};
    use std::io::Write as _;

    // @spec FMT-WS-001
    #[test]
    fn normalizes_crlf_to_lf() {
        assert_eq!(normalize_line_endings("a\r\nb\r\nc"), "a\nb\nc");
    }

    // @spec FMT-WS-001
    #[test]
    fn normalizes_bare_cr_to_lf() {
        assert_eq!(normalize_line_endings("a\rb\rc"), "a\nb\nc");
    }

    // @spec FMT-WS-001
    #[test]
    fn normalizes_trailing_lone_cr() {
        assert_eq!(normalize_line_endings("a\nb\r"), "a\nb\n");
    }

    // @spec FMT-WS-001
    #[test]
    fn normalizes_mixed_crlf_lf_and_bare_cr() {
        assert_eq!(normalize_line_endings("a\r\nb\nc\rd\r\n"), "a\nb\nc\nd\n");
    }

    // @spec FMT-WS-001
    #[test]
    fn passes_lf_only_input_through_unchanged() {
        let input = "a\nb\nc\n";
        assert_eq!(normalize_line_endings(input), input);
    }

    // @spec FMT-WS-001
    #[test]
    fn empty_input_is_empty_output() {
        assert_eq!(normalize_line_endings(""), "");
    }

    // @spec FMT-WS-001
    #[test]
    fn preserves_multibyte_unicode_around_line_endings() {
        assert_eq!(normalize_line_endings("héllo\r\nwörld\r"), "héllo\nwörld\n");
    }

    // @spec FMT-WS-001
    #[test]
    fn collapses_consecutive_crlf() {
        assert_eq!(normalize_line_endings("\r\n\r\n"), "\n\n");
    }

    // @spec FMT-WS-001
    #[test]
    fn collapses_consecutive_bare_cr() {
        assert_eq!(normalize_line_endings("\r\r\r"), "\n\n\n");
    }

    // @spec FMT-WS-001
    #[test]
    fn read_to_string_normalizes_crlf_from_disk() {
        let dir = tempdir();
        let path = dir.path().join("crlf.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"alpha\r\nbeta\r\n").unwrap();
        drop(f);
        let got = read_to_string(&path).unwrap();
        assert_eq!(got, "alpha\nbeta\n");
    }

    // @spec FMT-WS-001
    #[test]
    fn read_to_string_normalizes_bare_cr_from_disk() {
        let dir = tempdir();
        let path = dir.path().join("cr.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"alpha\rbeta\r").unwrap();
        drop(f);
        let got = read_to_string(&path).unwrap();
        assert_eq!(got, "alpha\nbeta\n");
    }

    #[test]
    fn write_emits_bytes_verbatim() {
        let dir = tempdir();
        let path = dir.path().join("out.txt");
        write(&path, "alpha\nbeta\n").unwrap();
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(raw, b"alpha\nbeta\n");
    }

    #[test]
    fn write_preserves_crlf_verbatim() {
        let dir = tempdir();
        let path = dir.path().join("crlf_write.txt");
        write(&path, "alpha\r\nbeta\r\n").unwrap();
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(raw, b"alpha\r\nbeta\r\n");
    }

    #[test]
    fn write_propagates_missing_parent_dir_error() {
        let dir = tempdir();
        let path = dir.path().join("missing").join("out.txt");
        let err = write(&path, "data\n").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn read_after_write_round_trips_lf_content() {
        let dir = tempdir();
        let path = dir.path().join("rt.txt");
        let content = "one\ntwo\nthree\n";
        write(&path, content).unwrap();
        let got = read_to_string(&path).unwrap();
        assert_eq!(got, content);
    }

    #[test]
    fn read_to_string_propagates_missing_file_error() {
        let dir = tempdir();
        let path = dir.path().join("does-not-exist.txt");
        let err = read_to_string(&path).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    struct TempDir {
        path: std::path::PathBuf,
    }

    impl TempDir {
        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn tempdir() -> TempDir {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("vat-file-io-{pid}-{n}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir { path }
    }
}
