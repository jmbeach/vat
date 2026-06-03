// @spec CMD-CFG-001, CMD-CFG-002, CMD-CFG-003, CMD-CFG-004, CMD-CFG-005, CMD-CFG-006

use std::path::Path;

use anyhow::{Context, bail};

use crate::file_io;
use crate::project_config::{self, ConfigError};
use crate::tombstone;
use crate::user_config::{self, UserConfig, UserConfigError};

/// Returns the value for `key`, or `None` when the key is valid but unset.
///
/// Returns `Err` for parse failures or unknown keys.
// @spec CMD-CFG-001, CMD-CFG-002
pub(crate) fn get(key: &str, backlog_dir: &Path) -> anyhow::Result<Option<String>> {
    let user_cfg_path = user_config::config_path()?;
    get_impl(key, backlog_dir, &user_cfg_path)
}

/// Sets `key` to `value`.
// @spec CMD-CFG-003, CMD-CFG-004, CMD-CFG-005, CMD-CFG-006
pub(crate) fn set(key: &str, value: &str, backlog_dir: &Path) -> anyhow::Result<()> {
    let user_cfg_path = user_config::config_path()?;
    set_impl(key, value, backlog_dir, &user_cfg_path)
}

// Testable inner implementation with explicit user config path.
fn get_impl(key: &str, backlog_dir: &Path, user_cfg_path: &Path) -> anyhow::Result<Option<String>> {
    match key {
        "user.name" => match user_config::load(user_cfg_path) {
            Ok(cfg) => Ok(cfg.user_name().map(ToOwned::to_owned)),
            Err(UserConfigError::NotFound(_)) => Ok(None),
            Err(e) => Err(anyhow::Error::from(e).context("reading user config")),
        },
        "project.id" => {
            let vat_toml = backlog_dir.join("vat.toml");
            match project_config::load(&vat_toml) {
                Ok(cfg) => Ok(Some(cfg.project_id().to_owned())),
                Err(ConfigError::NotFound(_)) => Ok(None),
                Err(e) => Err(anyhow::Error::from(e).context("reading vat.toml")),
            }
        }
        _ => bail!("unknown config key: {key}"),
    }
}

// Testable inner implementation with explicit user config path.
fn set_impl(
    key: &str,
    value: &str,
    backlog_dir: &Path,
    user_cfg_path: &Path,
) -> anyhow::Result<()> {
    match key {
        "user.name" => {
            let mut cfg = match user_config::load(user_cfg_path) {
                Ok(cfg) => cfg,
                Err(UserConfigError::NotFound(_)) => UserConfig::empty(),
                Err(e) => return Err(anyhow::Error::from(e).context("reading user config")),
            };
            cfg.set_user_name(value).context("invalid user.name")?;
            cfg.save(user_cfg_path).context("saving user config")?;
            Ok(())
        }
        "project.id" => {
            let vat_toml = backlog_dir.join("vat.toml");
            let mut cfg = project_config::load(&vat_toml).context("reading vat.toml")?;
            let old_prefix = cfg.project_id().to_owned();
            // CMD-CFG-005: refuse if the old prefix already has IDs in use;
            // changing it would orphan every existing ID. Skip the guard when
            // the normalised new value equals the old one (idempotent set).
            if old_prefix != value.to_ascii_lowercase()
                && ids_exist_with_prefix(&old_prefix, backlog_dir)
                    .context("checking for existing IDs")?
            {
                bail!(
                    "cannot change project.id: existing IDs use prefix '{old_prefix}'; \
                     edit vat.toml directly if you need to rename the project prefix"
                );
            }
            cfg.set_project_id(value).context("invalid project.id")?;
            cfg.save(&vat_toml).context("saving vat.toml")?;
            Ok(())
        }
        _ => bail!("unknown config key: {key}"),
    }
}

/// Returns `true` when any entry in `.used-ids` or `backlog.md` starts with
/// `<prefix>-`. Used to guard `vat config set project.id`.
fn ids_exist_with_prefix(prefix: &str, backlog_dir: &Path) -> anyhow::Result<bool> {
    let tombstone_path = backlog_dir.join(".used-ids");
    let used = tombstone::read(&tombstone_path)?;
    let prefix_dash = format!("{prefix}-");
    if used.iter().any(|id| id.starts_with(&prefix_dash)) {
        return Ok(true);
    }

    let backlog_path = backlog_dir.join("backlog.md");
    match file_io::read_to_string(&backlog_path) {
        Ok(content) => {
            // IDs always appear as `[<prefix>-<suffix>]` on bullet lines.
            let needle = format!("[{prefix}-");
            Ok(content.contains(&needle))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use super::{get_impl, ids_exist_with_prefix, set_impl};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_backlog_dir(dir: &TempDir) -> PathBuf {
        let backlog = dir.path().join("backlog");
        fs::create_dir_all(&backlog).unwrap();
        backlog
    }

    fn write_vat_toml(backlog: &Path, prefix: &str) {
        fs::write(
            backlog.join("vat.toml"),
            format!("[project]\nid = \"{prefix}\"\n"),
        )
        .unwrap();
    }

    fn write_used_ids(backlog: &Path, ids: &[&str]) {
        let content = ids.iter().fold(String::new(), |mut s, id| {
            s.push_str(id);
            s.push('\n');
            s
        });
        fs::write(backlog.join(".used-ids"), content).unwrap();
    }

    fn write_backlog_md(backlog: &Path, content: &str) {
        fs::write(backlog.join("backlog.md"), content).unwrap();
    }

    fn make_user_config_file(dir: &TempDir, name: &str) -> PathBuf {
        let path = dir.path().join("user_config.toml");
        fs::write(&path, format!("[user]\nname = \"{name}\"\n")).unwrap();
        path
    }

    // -----------------------------------------------------------------------
    // CMD-CFG-001 — get user.name
    // -----------------------------------------------------------------------

    // @spec CMD-CFG-001
    #[test]
    fn get_user_name_returns_value_when_set() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let cfg_path = make_user_config_file(&dir, "alice");

        let result = get_impl("user.name", &backlog, &cfg_path).unwrap();
        assert_eq!(result, Some("alice".to_owned()));
    }

    // @spec CMD-CFG-001
    #[test]
    fn get_user_name_returns_none_when_config_absent() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let missing = dir.path().join("nonexistent.toml");

        let result = get_impl("user.name", &backlog, &missing).unwrap();
        assert_eq!(result, None);
    }

    // @spec CMD-CFG-001
    #[test]
    fn get_user_name_returns_none_when_name_key_absent() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let cfg_path = dir.path().join("user_config.toml");
        fs::write(&cfg_path, "[user]\n").unwrap();

        let result = get_impl("user.name", &backlog, &cfg_path).unwrap();
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // CMD-CFG-002 — get project.id
    // -----------------------------------------------------------------------

    // @spec CMD-CFG-002
    #[test]
    fn get_project_id_returns_value_from_vat_toml() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_vat_toml(&backlog, "bar");
        let cfg_path = dir.path().join("user_config.toml");

        let result = get_impl("project.id", &backlog, &cfg_path).unwrap();
        assert_eq!(result, Some("bar".to_owned()));
    }

    // @spec CMD-CFG-002
    #[test]
    fn get_project_id_returns_none_when_vat_toml_absent() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let cfg_path = dir.path().join("user_config.toml");

        let result = get_impl("project.id", &backlog, &cfg_path).unwrap();
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // CMD-CFG-003 — set user.name
    // -----------------------------------------------------------------------

    // @spec CMD-CFG-003
    #[test]
    fn set_user_name_creates_config_when_absent() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let cfg_path = dir.path().join("user_config.toml");

        set_impl("user.name", "bob", &backlog, &cfg_path).unwrap();

        let content = fs::read_to_string(&cfg_path).unwrap();
        assert!(content.contains("name = \"bob\""));
    }

    // @spec CMD-CFG-003
    #[test]
    fn set_user_name_updates_existing_config() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let cfg_path = make_user_config_file(&dir, "old");

        set_impl("user.name", "new", &backlog, &cfg_path).unwrap();

        let content = fs::read_to_string(&cfg_path).unwrap();
        assert!(content.contains("name = \"new\""));
        assert!(!content.contains("old"));
    }

    // @spec CMD-CFG-003
    #[test]
    fn set_user_name_preserves_unknown_keys_in_config() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let cfg_path = dir.path().join("user_config.toml");
        fs::write(
            &cfg_path,
            "[user]\nname = \"old\"\n\n[other]\nkey = \"kept\"\n",
        )
        .unwrap();

        set_impl("user.name", "new", &backlog, &cfg_path).unwrap();

        let content = fs::read_to_string(&cfg_path).unwrap();
        assert!(content.contains("\"new\""));
        assert!(content.contains("key = \"kept\""));
    }

    // -----------------------------------------------------------------------
    // CMD-CFG-004 — set project.id validates format
    // -----------------------------------------------------------------------

    // @spec CMD-CFG-004
    #[test]
    fn set_project_id_validates_wrong_length() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_vat_toml(&backlog, "bar");
        let cfg_path = dir.path().join("user_config.toml");

        let err = set_impl("project.id", "ab", &backlog, &cfg_path).unwrap_err();
        // Error chain should mention the validation failure.
        let msg = format!("{err:#}");
        assert!(
            msg.contains("WrongLength") || msg.contains("invalid") || msg.contains("project.id"),
            "unexpected error message: {msg}"
        );
    }

    // @spec CMD-CFG-004
    #[test]
    fn set_project_id_validates_invalid_char() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_vat_toml(&backlog, "bar");
        let cfg_path = dir.path().join("user_config.toml");

        let err = set_impl("project.id", "abl", &backlog, &cfg_path).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("InvalidChar") || msg.contains("invalid") || msg.contains("abl"),
            "unexpected error message: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // CMD-CFG-005 — set project.id refuses when old prefix has IDs
    // -----------------------------------------------------------------------

    // @spec CMD-CFG-005
    #[test]
    fn set_project_id_refuses_when_tombstone_has_old_prefix() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_vat_toml(&backlog, "bar");
        write_used_ids(&backlog, &["bar-abc", "bar-def"]);
        let cfg_path = dir.path().join("user_config.toml");

        // Attempt to change from "bar" to "baz" — refused because "bar" IDs exist.
        let err = set_impl("project.id", "baz", &backlog, &cfg_path).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("bar"),
            "error should name the old prefix: {msg}"
        );
    }

    // @spec CMD-CFG-005
    #[test]
    fn set_project_id_refuses_when_backlog_md_has_old_prefix() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_vat_toml(&backlog, "bar");
        write_backlog_md(&backlog, "- [bar-abc] Some task\n");
        let cfg_path = dir.path().join("user_config.toml");

        // Attempt to change from "bar" to "baz" — refused because backlog has "bar" IDs.
        let err = set_impl("project.id", "baz", &backlog, &cfg_path).unwrap_err();
        assert!(format!("{err:#}").contains("bar"));
    }

    // @spec CMD-CFG-005
    #[test]
    fn set_project_id_succeeds_when_no_ids_exist() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_vat_toml(&backlog, "bar");
        let cfg_path = dir.path().join("user_config.toml");

        // No IDs anywhere — changing prefix is allowed.
        set_impl("project.id", "baz", &backlog, &cfg_path).unwrap();

        let content = fs::read_to_string(backlog.join("vat.toml")).unwrap();
        assert!(content.contains("id = \"baz\""));
    }

    // @spec CMD-CFG-005
    #[test]
    fn set_project_id_allows_same_value_even_when_ids_exist() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_vat_toml(&backlog, "bar");
        write_used_ids(&backlog, &["bar-abc"]);
        let cfg_path = dir.path().join("user_config.toml");

        // Idempotent — setting the same prefix must succeed even with IDs.
        set_impl("project.id", "bar", &backlog, &cfg_path).unwrap();
    }

    // @spec CMD-CFG-005
    #[test]
    fn set_project_id_allows_same_value_uppercase_idempotent() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_vat_toml(&backlog, "bar");
        write_used_ids(&backlog, &["bar-abc"]);
        let cfg_path = dir.path().join("user_config.toml");

        // "BAR" normalises to "bar" == old prefix → must not refuse.
        set_impl("project.id", "BAR", &backlog, &cfg_path).unwrap();
    }

    // -----------------------------------------------------------------------
    // CMD-CFG-006 — unknown key error
    // -----------------------------------------------------------------------

    // @spec CMD-CFG-006
    #[test]
    fn get_unknown_key_returns_error_with_key_name() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let cfg_path = dir.path().join("user_config.toml");

        let err = get_impl("user.email", &backlog, &cfg_path).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unknown config key"),
            "expected 'unknown config key' in: {msg}"
        );
        assert!(msg.contains("user.email"));
    }

    // @spec CMD-CFG-006
    #[test]
    fn set_unknown_key_returns_error_with_key_name() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        let cfg_path = dir.path().join("user_config.toml");

        let err = set_impl("user.email", "x@y.com", &backlog, &cfg_path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown config key"));
        assert!(msg.contains("user.email"));
    }

    // -----------------------------------------------------------------------
    // ids_exist_with_prefix helper
    // -----------------------------------------------------------------------

    #[test]
    fn ids_exist_checks_tombstone() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_used_ids(&backlog, &["bar-abc"]);
        assert!(ids_exist_with_prefix("bar", &backlog).unwrap());
        assert!(!ids_exist_with_prefix("baz", &backlog).unwrap());
    }

    #[test]
    fn ids_exist_checks_backlog_md() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        write_backlog_md(&backlog, "- [bar-abc] Task\n");
        assert!(ids_exist_with_prefix("bar", &backlog).unwrap());
        assert!(!ids_exist_with_prefix("baz", &backlog).unwrap());
    }

    #[test]
    fn ids_exist_returns_false_when_no_files() {
        let dir = TempDir::new().unwrap();
        let backlog = make_backlog_dir(&dir);
        assert!(!ids_exist_with_prefix("bar", &backlog).unwrap());
    }
}
