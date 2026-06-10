// @spec CMD-INIT-001, CMD-INIT-002, CMD-INIT-004, CMD-INIT-005, CMD-INIT-006, CMD-INIT-007

use std::fs;
use std::io;
use std::path::Path;

use thiserror::Error;

use crate::file_io;
use crate::project_config::{ConfigError, ProjectConfig};
use crate::readme_template;

#[derive(Debug, Error)]
pub(crate) enum InitError {
    #[error("backlog/ already exists; vat is initialized")]
    AlreadyInitialized,
    #[error("{0}")]
    InvalidPrefix(#[from] ConfigError),
    #[error("IO error initializing backlog/: {0}")]
    Io(#[from] io::Error),
}

/// Initialize a new backlog under `project_root/backlog/`.
///
/// `prefix` is the already-resolved 3-char Crockford base32 prefix (either
/// from the CLI arg or from the interactive prompt in the caller). Validation
/// is delegated to `ProjectConfig::new`.
// @spec CMD-INIT-001, CMD-INIT-002, CMD-INIT-004, CMD-INIT-005, CMD-INIT-006
pub(crate) fn init(project_root: &Path, prefix: &str) -> Result<String, InitError> {
    let backlog_dir = project_root.join("backlog");

    // CMD-INIT-001: abort if backlog/ already exists.
    if backlog_dir.exists() {
        return Err(InitError::AlreadyInitialized);
    }

    // CMD-INIT-004: validate prefix via ProjectConfig (3 chars, Crockford base32).
    let config = ProjectConfig::new(prefix)?;
    let normalized = config.project_id().to_string();

    // CMD-INIT-005: create directory and write all required files.
    // If any write fails after the directory is created, remove the
    // partially-populated backlog/ so the CMD-INIT-001 AlreadyInitialized
    // guard doesn't permanently block a retry.
    fs::create_dir_all(&backlog_dir)?;
    if let Err(e) = write_backlog_files(&backlog_dir, &config, &normalized) {
        // Best-effort cleanup; ignore any secondary error so the original
        // write failure is what surfaces to the user.
        let _ = fs::remove_dir_all(&backlog_dir);
        return Err(e);
    }

    Ok(format!("initialized backlog/ with prefix {normalized}"))
}

/// Write the four backlog files into an already-created `backlog_dir`.
///
/// Split out from `init` so that a write failure partway through can be
/// cleaned up by the caller (removing the half-populated directory) rather
/// than leaving the user stuck behind the `AlreadyInitialized` guard.
// @spec CMD-INIT-005, CMD-INIT-006
fn write_backlog_files(
    backlog_dir: &Path,
    config: &ProjectConfig,
    normalized: &str,
) -> Result<(), InitError> {
    file_io::write(backlog_dir.join("vat.toml"), &config.serialize())?;
    file_io::write(backlog_dir.join("backlog.md"), "---\nversion: 1\n---\n")?;
    file_io::write(backlog_dir.join(".used-ids"), "")?;

    // CMD-INIT-006: render template and write README (written once; never read by any other command).
    file_io::write(
        backlog_dir.join("README.md"),
        &readme_template::render(normalized),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // @spec CMD-INIT-001
    #[test]
    fn init_fails_when_backlog_dir_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("backlog")).unwrap();
        let err = init(dir.path(), "vat").unwrap_err();
        assert!(
            matches!(err, InitError::AlreadyInitialized),
            "expected AlreadyInitialized, got {err:?}"
        );
        assert_eq!(
            err.to_string(),
            "backlog/ already exists; vat is initialized"
        );
    }

    // @spec CMD-INIT-001
    #[test]
    fn init_does_not_create_files_when_backlog_dir_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let backlog_dir = dir.path().join("backlog");
        fs::create_dir(&backlog_dir).unwrap();
        let _ = init(dir.path(), "vat");
        // Only the pre-existing dir should be present; no files created.
        assert!(
            fs::read_dir(&backlog_dir).unwrap().next().is_none(),
            "no files should be written when already initialized"
        );
    }

    // @spec CMD-INIT-002
    #[test]
    fn init_uses_supplied_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let msg = init(dir.path(), "vat").unwrap();
        assert_eq!(msg, "initialized backlog/ with prefix vat");
    }

    // @spec CMD-INIT-004
    #[test]
    fn init_rejects_prefix_that_is_too_short() {
        let dir = tempfile::tempdir().unwrap();
        let err = init(dir.path(), "fo").unwrap_err();
        assert!(
            matches!(err, InitError::InvalidPrefix(_)),
            "two-char prefix should be invalid"
        );
    }

    // @spec CMD-INIT-004
    #[test]
    fn init_rejects_prefix_that_is_too_long() {
        let dir = tempfile::tempdir().unwrap();
        let err = init(dir.path(), "fooo").unwrap_err();
        assert!(
            matches!(err, InitError::InvalidPrefix(_)),
            "four-char prefix should be invalid"
        );
    }

    // @spec CMD-INIT-004
    #[test]
    fn init_rejects_prefix_with_invalid_crockford_chars() {
        let dir = tempfile::tempdir().unwrap();
        // 'l', 'i', 'o', 'u' are excluded from the Crockford alphabet.
        let err = init(dir.path(), "lol").unwrap_err();
        assert!(matches!(err, InitError::InvalidPrefix(_)));
    }

    // @spec CMD-INIT-004
    #[test]
    fn init_rejects_empty_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let err = init(dir.path(), "").unwrap_err();
        assert!(matches!(err, InitError::InvalidPrefix(_)));
    }

    // @spec CMD-INIT-004
    #[test]
    fn init_normalizes_uppercase_prefix_to_lowercase() {
        let dir = tempfile::tempdir().unwrap();
        // "BAR" is a valid uppercase Crockford base32 prefix (B, A, R all valid).
        let msg = init(dir.path(), "BAR").unwrap();
        assert!(
            msg.contains("bar"),
            "prefix should be lowercased in success message"
        );
        let toml_raw = fs::read_to_string(dir.path().join("backlog/vat.toml")).unwrap();
        assert!(
            toml_raw.contains("bar"),
            "normalized prefix should appear in vat.toml"
        );
        assert!(
            !toml_raw.contains("BAR"),
            "uppercase prefix must not appear in vat.toml"
        );
    }

    // @spec CMD-INIT-005
    #[test]
    fn init_creates_backlog_directory() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), "vat").unwrap();
        assert!(dir.path().join("backlog").is_dir());
    }

    // @spec CMD-INIT-005
    #[test]
    fn init_creates_vat_toml_with_correct_project_id() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), "bar").unwrap();
        let toml_raw = fs::read_to_string(dir.path().join("backlog/vat.toml")).unwrap();
        let parsed: toml::Value = toml_raw.parse().unwrap();
        let id = parsed
            .get("project")
            .and_then(|p| p.get("id"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(id, "bar");
    }

    // @spec CMD-INIT-005
    #[test]
    fn init_creates_backlog_md_with_version_1_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), "vat").unwrap();
        let content = fs::read_to_string(dir.path().join("backlog/backlog.md")).unwrap();
        assert_eq!(content, "---\nversion: 1\n---\n");
    }

    // @spec CMD-INIT-005
    #[test]
    fn init_creates_empty_used_ids_file() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), "vat").unwrap();
        let content = fs::read_to_string(dir.path().join("backlog/.used-ids")).unwrap();
        assert_eq!(content, "");
    }

    // @spec CMD-INIT-005, CMD-INIT-006
    #[test]
    fn init_creates_readme_md() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), "vat").unwrap();
        let path = dir.path().join("backlog/README.md");
        assert!(path.exists(), "backlog/README.md should be created");
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty());
    }

    // @spec CMD-INIT-006
    #[test]
    fn init_readme_substitutes_prefix_in_template() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), "baz").unwrap();
        let content = fs::read_to_string(dir.path().join("backlog/README.md")).unwrap();
        assert!(
            content.contains("baz"),
            "README should contain the project prefix"
        );
        assert!(
            !content.contains("{prefix}"),
            "README should not contain the unsubstituted placeholder"
        );
    }

    // @spec CMD-INIT-004
    #[test]
    fn init_with_invalid_prefix_does_not_create_any_files() {
        let dir = tempfile::tempdir().unwrap();
        let _ = init(dir.path(), "bad!");
        assert!(
            !dir.path().join("backlog").exists(),
            "backlog/ must not be created when prefix is invalid"
        );
    }
}
