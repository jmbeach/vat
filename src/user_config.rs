// @spec FMT-USR-001, FMT-USR-002, FMT-USR-003, FMT-USR-004, FMT-USR-005, FMT-USR-006

#![allow(dead_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml::Value;

#[derive(Debug, Error)]
pub(crate) enum UserConfigError {
    #[error("malformed user config: {0}")]
    Parse(String),
    #[error("[user] must be a table")]
    UserNotATable,
    #[error("[user].name must be a string")]
    UserNameNotString,
    #[error("[user].name must not be empty or whitespace-only")]
    UserNameEmpty,
    #[error("user config file not found: {0}")]
    NotFound(PathBuf),
    #[error(
        "cannot determine user config path: neither XDG_CONFIG_HOME (absolute) nor HOME is set"
    )]
    NoHome,
    #[error("user config I/O error: {0}")]
    Io(#[from] io::Error),
}

// `io::Error` is not `PartialEq`, so this can't derive it the way `ConfigError`
// does. Unit variants compare by discriminant, payload variants by value, and
// `Io` by `ErrorKind` — enough for `assert_eq!`-style assertions in tests.
impl PartialEq for UserConfigError {
    fn eq(&self, other: &Self) -> bool {
        use UserConfigError::{
            Io, NoHome, NotFound, Parse, UserNameEmpty, UserNameNotString, UserNotATable,
        };
        match (self, other) {
            (Parse(a), Parse(b)) => a == b,
            (NotFound(a), NotFound(b)) => a == b,
            (Io(a), Io(b)) => a.kind() == b.kind(),
            (UserNotATable, UserNotATable)
            | (UserNameNotString, UserNameNotString)
            | (UserNameEmpty, UserNameEmpty)
            | (NoHome, NoHome) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct UserConfig {
    document: Value,
}

impl UserConfig {
    // @spec FMT-USR-002
    pub(crate) fn empty() -> Self {
        Self {
            document: Value::Table(toml::value::Table::new()),
        }
    }

    // @spec FMT-USR-002
    pub(crate) fn user_name(&self) -> Option<&str> {
        self.document.get("user")?.get("name")?.as_str()
    }

    // @spec FMT-USR-004
    pub(crate) fn set_user_name(&mut self, name: &str) -> Result<(), UserConfigError> {
        // Reject whitespace-only as well as empty: a blank name round-trips but
        // would render a malformed `[by:<name>]` marker downstream.
        if name.trim().is_empty() {
            return Err(UserConfigError::UserNameEmpty);
        }
        write_user_name(&mut self.document, name);
        Ok(())
    }

    // @spec FMT-USR-005
    pub(crate) fn serialize(&self) -> String {
        toml::to_string(&self.document)
            .expect("serializing toml::Value with string keys cannot fail")
    }

    // @spec FMT-USR-005
    pub(crate) fn save(&self, path: &Path) -> Result<(), UserConfigError> {
        // A bare filename has parent `Some("")`; `create_dir_all("")` is a no-op
        // on POSIX but errors on Windows, so skip the empty-parent case.
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        // Direct write, no temp-file-and-rename: see "Direct write rationale" in
        // docs/llds/backlog-format.md — git, not the local filesystem, is the
        // durability boundary for this small, infrequently-written file.
        fs::write(path, self.serialize())?;
        Ok(())
    }
}

// @spec FMT-USR-002, FMT-USR-003, FMT-USR-004, FMT-USR-006
pub(crate) fn parse(input: &str) -> Result<UserConfig, UserConfigError> {
    let document: Value = input
        .parse::<Value>()
        .map_err(|e| UserConfigError::Parse(e.to_string()))?;

    // `[user]` is optional, but if present it must be a table. A hand-edited
    // scalar like `user = "jared"` would otherwise pass silently and have its
    // value dropped, since `Value::get("name")` returns `None` on a non-table.
    if let Some(user_value) = document.get("user") {
        let user_table = user_value
            .as_table()
            .ok_or(UserConfigError::UserNotATable)?;
        // `name` is optional, but if present it must be a non-empty,
        // non-whitespace-only string.
        if let Some(name_value) = user_table.get("name") {
            let name = name_value
                .as_str()
                .ok_or(UserConfigError::UserNameNotString)?;
            if name.trim().is_empty() {
                return Err(UserConfigError::UserNameEmpty);
            }
        }
    }

    Ok(UserConfig { document })
}

// @spec FMT-USR-001
// Thin wrapper that reads the process environment. The resolution logic lives in
// `config_path_from_env`, which takes the env values as arguments — that is the
// testable seam, so tests avoid `std::env::set_var` (which races under the
// parallel test runner). This wrapper is intentionally left untested.
pub(crate) fn config_path() -> Result<PathBuf, UserConfigError> {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let home = std::env::var("HOME").ok();
    config_path_from_env(xdg.as_deref(), home.as_deref())
}

// @spec FMT-USR-001
pub(crate) fn config_path_from_env(
    xdg: Option<&str>,
    home: Option<&str>,
) -> Result<PathBuf, UserConfigError> {
    // An absolute, non-empty XDG_CONFIG_HOME wins outright. Empty or relative
    // values are ignored per the XDG Base Directory spec.
    if let Some(xdg) = xdg
        && !xdg.is_empty()
        && Path::new(xdg).is_absolute()
    {
        return Ok(Path::new(xdg).join("vat").join("config.toml"));
    }

    if let Some(home) = home
        && !home.is_empty()
    {
        return Ok(Path::new(home)
            .join(".config")
            .join("vat")
            .join("config.toml"));
    }

    Err(UserConfigError::NoHome)
}

// @spec FMT-USR-002, FMT-USR-003, FMT-USR-004
pub(crate) fn load(path: &Path) -> Result<UserConfig, UserConfigError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(UserConfigError::NotFound(path.to_path_buf()));
        }
        Err(e) => return Err(UserConfigError::Io(e)),
    };
    parse(&contents)
}

fn write_user_name(document: &mut Value, name: &str) {
    let Value::Table(root) = document else {
        unreachable!("UserConfig.document is always a Table");
    };
    let user_entry = root
        .entry("user".to_string())
        .or_insert_with(|| Value::Table(toml::value::Table::new()));
    // If someone hand-edited [user] to a non-table value, replace it.
    if !user_entry.is_table() {
        *user_entry = Value::Table(toml::value::Table::new());
    }
    let Value::Table(user_table) = user_entry else {
        unreachable!("just-ensured Table");
    };
    user_table.insert("name".to_string(), Value::String(name.to_string()));
}

#[cfg(test)]
mod tests {
    use super::{UserConfig, UserConfigError, config_path_from_env, load, parse};
    use std::path::PathBuf;
    use tempfile::tempdir;

    // ---------------------------------------------------------------
    // FMT-USR-001 — path resolution
    // ---------------------------------------------------------------

    // @spec FMT-USR-001
    #[test]
    fn path_uses_xdg_when_set_absolute_and_nonempty() {
        let path = config_path_from_env(Some("/custom/xdg"), Some("/home/jared")).unwrap();
        assert_eq!(path, PathBuf::from("/custom/xdg/vat/config.toml"));
    }

    // @spec FMT-USR-001
    #[test]
    fn path_uses_xdg_even_when_home_unset() {
        let path = config_path_from_env(Some("/custom/xdg"), None).unwrap();
        assert_eq!(path, PathBuf::from("/custom/xdg/vat/config.toml"));
    }

    // @spec FMT-USR-001
    #[test]
    fn path_falls_back_to_home_when_xdg_unset() {
        let path = config_path_from_env(None, Some("/home/jared")).unwrap();
        assert_eq!(path, PathBuf::from("/home/jared/.config/vat/config.toml"));
    }

    // @spec FMT-USR-001
    #[test]
    fn path_falls_back_to_home_when_xdg_empty() {
        let path = config_path_from_env(Some(""), Some("/home/jared")).unwrap();
        assert_eq!(path, PathBuf::from("/home/jared/.config/vat/config.toml"));
    }

    // @spec FMT-USR-001
    #[test]
    fn path_falls_back_to_home_when_xdg_relative() {
        let path = config_path_from_env(Some("relative/path"), Some("/home/jared")).unwrap();
        assert_eq!(path, PathBuf::from("/home/jared/.config/vat/config.toml"));
    }

    // @spec FMT-USR-001
    #[test]
    fn path_errors_no_home_when_neither_xdg_nor_home_usable() {
        let err = config_path_from_env(None, None).unwrap_err();
        assert!(matches!(err, UserConfigError::NoHome));
    }

    // @spec FMT-USR-001
    #[test]
    fn path_errors_no_home_when_xdg_relative_and_home_empty() {
        let err = config_path_from_env(Some("rel"), Some("")).unwrap_err();
        assert!(matches!(err, UserConfigError::NoHome));
    }

    // ---------------------------------------------------------------
    // FMT-USR-002 — user.name optional
    // ---------------------------------------------------------------

    // @spec FMT-USR-002
    #[test]
    fn parse_empty_input_yields_no_user_name() {
        let cfg = parse("").expect("empty input is a valid empty config");
        assert_eq!(cfg.user_name(), None);
    }

    // @spec FMT-USR-002
    #[test]
    fn parse_without_user_table_yields_no_user_name() {
        let cfg = parse("[other]\nkey = \"value\"\n").unwrap();
        assert_eq!(cfg.user_name(), None);
    }

    // @spec FMT-USR-002
    #[test]
    fn parse_with_user_table_but_no_name_yields_no_user_name() {
        let cfg = parse("[user]\n").unwrap();
        assert_eq!(cfg.user_name(), None);
    }

    // @spec FMT-USR-002
    #[test]
    fn parse_with_user_name_returns_it() {
        let cfg = parse("[user]\nname = \"jared\"\n").unwrap();
        assert_eq!(cfg.user_name(), Some("jared"));
    }

    // @spec FMT-USR-002
    #[test]
    fn empty_constructor_yields_no_user_name() {
        let cfg = UserConfig::empty();
        assert_eq!(cfg.user_name(), None);
    }

    // ---------------------------------------------------------------
    // FMT-USR-003 — non-string user.name is an error
    // ---------------------------------------------------------------

    // @spec FMT-USR-003
    #[test]
    fn parse_rejects_integer_user_name() {
        assert_eq!(
            parse("[user]\nname = 42\n").unwrap_err(),
            UserConfigError::UserNameNotString
        );
    }

    // @spec FMT-USR-003
    #[test]
    fn parse_rejects_array_user_name() {
        assert_eq!(
            parse("[user]\nname = [\"a\", \"b\"]\n").unwrap_err(),
            UserConfigError::UserNameNotString
        );
    }

    // @spec FMT-USR-003
    #[test]
    fn parse_rejects_boolean_user_name() {
        assert_eq!(
            parse("[user]\nname = true\n").unwrap_err(),
            UserConfigError::UserNameNotString
        );
    }

    // `load` is read-only, so there is no "file unchanged" invariant to assert;
    // the meaningful guarantee is that it surfaces the parse-layer error.
    // @spec FMT-USR-003
    #[test]
    fn load_propagates_non_string_name_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[user]\nname = 42\n").unwrap();
        assert_eq!(load(&path).unwrap_err(), UserConfigError::UserNameNotString);
    }

    // ---------------------------------------------------------------
    // FMT-USR-006 — a non-table `[user]` is an error
    // ---------------------------------------------------------------

    // @spec FMT-USR-006
    #[test]
    fn parse_rejects_non_table_user() {
        // A bare scalar `user = "jared"` is not a `[user]` table; without the
        // type guard this would parse cleanly and silently lose the value.
        assert_eq!(
            parse("user = \"jared\"\n").unwrap_err(),
            UserConfigError::UserNotATable
        );
    }

    // @spec FMT-USR-006
    #[test]
    fn parse_rejects_array_of_tables_user() {
        assert_eq!(
            parse("[[user]]\nname = \"jared\"\n").unwrap_err(),
            UserConfigError::UserNotATable
        );
    }

    // ---------------------------------------------------------------
    // FMT-USR-004 — empty-string user.name is an error
    // ---------------------------------------------------------------

    // @spec FMT-USR-004
    #[test]
    fn parse_rejects_empty_string_user_name() {
        assert_eq!(
            parse("[user]\nname = \"\"\n").unwrap_err(),
            UserConfigError::UserNameEmpty
        );
    }

    // @spec FMT-USR-004
    #[test]
    fn parse_rejects_whitespace_only_user_name() {
        // A blank name round-trips cleanly but would render a malformed
        // `[by:<name>]` marker downstream, so it is rejected like the empty string.
        assert_eq!(
            parse("[user]\nname = \"   \"\n").unwrap_err(),
            UserConfigError::UserNameEmpty
        );
    }

    // @spec FMT-USR-004
    #[test]
    fn load_propagates_empty_name_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[user]\nname = \"\"\n").unwrap();
        assert_eq!(load(&path).unwrap_err(), UserConfigError::UserNameEmpty);
    }

    // @spec FMT-USR-004
    #[test]
    fn set_user_name_rejects_empty_string() {
        let mut cfg = UserConfig::empty();
        assert_eq!(
            cfg.set_user_name("").unwrap_err(),
            UserConfigError::UserNameEmpty
        );
        assert_eq!(cfg.user_name(), None);
    }

    // @spec FMT-USR-004
    #[test]
    fn set_user_name_rejects_whitespace_only_string() {
        let mut cfg = UserConfig::empty();
        assert_eq!(
            cfg.set_user_name("   ").unwrap_err(),
            UserConfigError::UserNameEmpty
        );
        assert_eq!(cfg.user_name(), None);
    }

    // @spec FMT-USR-004
    #[test]
    fn set_user_name_accepts_nonempty_string() {
        let mut cfg = UserConfig::empty();
        cfg.set_user_name("jared").unwrap();
        assert_eq!(cfg.user_name(), Some("jared"));
    }

    // ---------------------------------------------------------------
    // FMT-USR-005 — unknown sections and keys are preserved on write
    // ---------------------------------------------------------------

    // @spec FMT-USR-005
    #[test]
    fn round_trip_preserves_unknown_top_level_section() {
        let input = "[user]\nname = \"jared\"\n\n[other]\nkey = \"value\"\n";
        let cfg = parse(input).unwrap();
        let out = cfg.serialize();
        let reparsed: toml::Value = out.parse().unwrap();
        assert_eq!(
            reparsed
                .get("other")
                .and_then(|v| v.get("key"))
                .and_then(|v| v.as_str()),
            Some("value")
        );
        assert_eq!(
            reparsed
                .get("user")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str()),
            Some("jared")
        );
    }

    // @spec FMT-USR-005
    #[test]
    fn round_trip_preserves_unknown_keys_in_user_table() {
        let input = "[user]\nname = \"jared\"\nextra = \"keep me\"\nnumber = 42\n";
        let cfg = parse(input).unwrap();
        let out = cfg.serialize();
        let reparsed: toml::Value = out.parse().unwrap();
        let user = reparsed.get("user").unwrap();
        assert_eq!(user.get("name").and_then(|v| v.as_str()), Some("jared"));
        assert_eq!(user.get("extra").and_then(|v| v.as_str()), Some("keep me"));
        assert_eq!(
            user.get("number").and_then(toml::Value::as_integer),
            Some(42)
        );
    }

    // A future refactor of write_user_name that replaces the whole [user] table
    // would silently drop sibling keys; pin the parse + set + serialize path.
    // @spec FMT-USR-005
    #[test]
    fn set_user_name_preserves_sibling_keys_in_user_table() {
        let input = "[user]\nname = \"old\"\nextra = \"keep\"\n";
        let mut cfg = parse(input).unwrap();
        cfg.set_user_name("new").unwrap();
        let out = cfg.serialize();
        let reparsed: toml::Value = out.parse().unwrap();
        let user = reparsed.get("user").unwrap();
        assert_eq!(user.get("name").and_then(|v| v.as_str()), Some("new"));
        assert_eq!(user.get("extra").and_then(|v| v.as_str()), Some("keep"));
    }

    // With toml's preserve_order feature, sections keep their authored order
    // rather than being alphabetized ([other] would otherwise sort before [user]).
    // @spec FMT-USR-005
    #[test]
    fn serialize_preserves_section_order() {
        let input = "[user]\nname = \"jared\"\n\n[other]\nkey = \"value\"\n";
        let cfg = parse(input).unwrap();
        let out = cfg.serialize();
        let user_pos = out.find("[user]").expect("[user] present");
        let other_pos = out.find("[other]").expect("[other] present");
        assert!(
            user_pos < other_pos,
            "expected [user] before [other], got:\n{out}"
        );
    }

    // @spec FMT-USR-005
    #[test]
    fn set_user_name_preserves_unknown_sections() {
        let input = "[user]\nname = \"jared\"\n\n[other]\nkey = \"value\"\n";
        let mut cfg = parse(input).unwrap();
        cfg.set_user_name("alice").unwrap();
        let out = cfg.serialize();
        let reparsed: toml::Value = out.parse().unwrap();
        assert_eq!(
            reparsed
                .get("user")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str()),
            Some("alice")
        );
        assert_eq!(
            reparsed
                .get("other")
                .and_then(|v| v.get("key"))
                .and_then(|v| v.as_str()),
            Some("value")
        );
    }

    // ---------------------------------------------------------------
    // load / save — file I/O behavior from the LLD
    // ---------------------------------------------------------------

    // @spec FMT-USR-002
    #[test]
    fn load_missing_file_returns_not_found_with_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist.toml");
        // The error carries the resolved path so callers can tell the user where
        // the binary looked.
        assert_eq!(
            load(&path).unwrap_err(),
            UserConfigError::NotFound(path.clone())
        );
    }

    // @spec FMT-USR-002, FMT-USR-005
    #[test]
    fn load_then_save_round_trips_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[user]\nname = \"jared\"\n").unwrap();
        let cfg = load(&path).unwrap();
        assert_eq!(cfg.user_name(), Some("jared"));
        cfg.save(&path).unwrap();
        let on_disk = std::fs::read_to_string(&path).unwrap();
        // Independent oracle: assert on the raw TOML structure, not just our
        // own parse()/user_name() round-trip (which could mask a symmetric bug).
        let raw: toml::Value = on_disk.parse().unwrap();
        assert_eq!(
            raw.get("user")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str()),
            Some("jared")
        );
        let reparsed = parse(&on_disk).unwrap();
        assert_eq!(reparsed.user_name(), Some("jared"));
    }

    // From CMD-CFG-003 ("creating the file and parent directories if needed");
    // user_config::save is the implementation point.
    #[test]
    fn save_creates_missing_parent_directories() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b").join("c").join("config.toml");
        let mut cfg = UserConfig::empty();
        cfg.set_user_name("jared").unwrap();
        cfg.save(&nested).unwrap();
        assert!(nested.exists());
        let on_disk = std::fs::read_to_string(&nested).unwrap();
        let reparsed = parse(&on_disk).unwrap();
        assert_eq!(reparsed.user_name(), Some("jared"));
    }

    // No dedicated EARS: generic parse robustness (the `Parse` variant).
    #[test]
    fn parse_rejects_malformed_toml() {
        let err = parse("[user\nname = \"x\"\n").unwrap_err();
        assert!(matches!(err, UserConfigError::Parse(_)));
    }
}
