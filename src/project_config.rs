// @spec FMT-CFG-001, FMT-CFG-002, FMT-CFG-003

#![allow(dead_code)]

use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml::Value;

use crate::base32::{Base32Error, validate};

const PROJECT_ID_LEN: usize = 3;

#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    #[error("malformed vat.toml: {0}")]
    Parse(String),
    #[error("vat.toml is missing the [project] table; run `vat init`")]
    MissingProject,
    #[error("vat.toml [project] table has no `id` key; run `vat init`")]
    MissingProjectId,
    #[error("vat.toml [project].id must be a string; run `vat init`")]
    ProjectIdNotString,
    #[error("vat.toml [project].id is invalid: {0}; run `vat init`")]
    InvalidProjectId(#[from] Base32Error),
    #[error("vat.toml not found at {0}; run `vat init`")]
    NotFound(PathBuf),
    #[error("vat.toml I/O error: {0}")]
    Io(io::Error),
}

// `io::Error` is not `PartialEq`, so we can't derive it. Match unit variants
// by discriminant, payload variants by value, and `Io` by `ErrorKind` — enough
// for `assert_eq!`-style assertions in tests, matching `UserConfigError`.
impl PartialEq for ConfigError {
    fn eq(&self, other: &Self) -> bool {
        use ConfigError::{
            InvalidProjectId, Io, MissingProject, MissingProjectId, NotFound, Parse,
            ProjectIdNotString,
        };
        match (self, other) {
            (Parse(a), Parse(b)) => a == b,
            (MissingProject, MissingProject)
            | (MissingProjectId, MissingProjectId)
            | (ProjectIdNotString, ProjectIdNotString) => true,
            (InvalidProjectId(a), InvalidProjectId(b)) => a == b,
            (NotFound(a), NotFound(b)) => a == b,
            (Io(a), Io(b)) => a.kind() == b.kind(),
            _ => false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ProjectConfig {
    document: Value,
    project_id: String,
}

impl ProjectConfig {
    // @spec FMT-CFG-001
    pub(crate) fn new(project_id: &str) -> Result<Self, ConfigError> {
        let normalized = validate_and_normalize(project_id)?;
        let mut document = Value::Table(toml::value::Table::new());
        write_project_id(&mut document, &normalized);
        Ok(Self {
            document,
            project_id: normalized,
        })
    }

    pub(crate) fn project_id(&self) -> &str {
        &self.project_id
    }

    // @spec FMT-CFG-001
    pub(crate) fn set_project_id(&mut self, id: &str) -> Result<(), ConfigError> {
        let normalized = validate_and_normalize(id)?;
        write_project_id(&mut self.document, &normalized);
        self.project_id = normalized;
        Ok(())
    }

    // @spec FMT-CFG-003
    pub(crate) fn serialize(&self) -> String {
        toml::to_string(&self.document)
            .expect("serializing toml::Value with string keys cannot fail")
    }

    pub(crate) fn save(&self, path: &Path) -> Result<(), ConfigError> {
        std::fs::write(path, self.serialize()).map_err(ConfigError::Io)
    }
}

pub(crate) fn load(path: &Path) -> Result<ProjectConfig, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(contents) => parse(&contents),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(ConfigError::NotFound(path.to_path_buf()))
        }
        Err(e) => Err(ConfigError::Io(e)),
    }
}

// @spec FMT-CFG-001, FMT-CFG-002, FMT-CFG-003
pub(crate) fn parse(input: &str) -> Result<ProjectConfig, ConfigError> {
    let document: Value = input
        .parse::<Value>()
        .map_err(|e| ConfigError::Parse(e.to_string()))?;

    let project = document
        .get("project")
        .ok_or(ConfigError::MissingProject)?
        .as_table()
        .ok_or(ConfigError::MissingProject)?;

    let id_value = project.get("id").ok_or(ConfigError::MissingProjectId)?;
    let id_str = id_value.as_str().ok_or(ConfigError::ProjectIdNotString)?;
    let normalized = validate_and_normalize(id_str)?;

    Ok(ProjectConfig {
        document,
        project_id: normalized,
    })
}

fn validate_and_normalize(id: &str) -> Result<String, ConfigError> {
    validate(id, PROJECT_ID_LEN)?;
    Ok(id.to_ascii_lowercase())
}

fn write_project_id(document: &mut Value, id: &str) {
    let Value::Table(root) = document else {
        unreachable!("ProjectConfig.document is always a Table");
    };
    let project_entry = root
        .entry("project".to_string())
        .or_insert_with(|| Value::Table(toml::value::Table::new()));
    // If someone hand-edited [project] to a non-table value, replace it.
    if !project_entry.is_table() {
        *project_entry = Value::Table(toml::value::Table::new());
    }
    let Value::Table(project_table) = project_entry else {
        unreachable!("just-ensured Table");
    };
    project_table.insert("id".to_string(), Value::String(id.to_string()));
}

#[cfg(test)]
mod tests {
    use super::{ConfigError, ProjectConfig, parse};
    use crate::base32::Base32Error;

    // @spec FMT-CFG-001
    #[test]
    fn parse_accepts_valid_project_id() {
        let input = "[project]\nid = \"bar\"\n";
        let cfg = parse(input).expect("valid config should parse");
        assert_eq!(cfg.project_id(), "bar");
    }

    // @spec FMT-CFG-001
    #[test]
    fn parse_normalizes_uppercase_project_id_to_lowercase() {
        let input = "[project]\nid = \"BaR\"\n";
        let cfg = parse(input).expect("mixed-case base32 should parse");
        assert_eq!(cfg.project_id(), "bar");
    }

    // @spec FMT-CFG-002
    #[test]
    fn parse_rejects_missing_project_table() {
        let input = "[other]\nkey = \"value\"\n";
        assert_eq!(parse(input).err(), Some(ConfigError::MissingProject));
    }

    // @spec FMT-CFG-002
    #[test]
    fn parse_rejects_empty_input() {
        assert_eq!(parse("").err(), Some(ConfigError::MissingProject));
    }

    // @spec FMT-CFG-002
    #[test]
    fn parse_rejects_missing_project_id_key() {
        let input = "[project]\n";
        assert_eq!(parse(input).err(), Some(ConfigError::MissingProjectId));
    }

    // @spec FMT-CFG-002
    #[test]
    fn parse_rejects_non_string_project_id() {
        let input = "[project]\nid = 42\n";
        assert_eq!(parse(input).err(), Some(ConfigError::ProjectIdNotString));
    }

    // @spec FMT-CFG-001, FMT-CFG-002
    #[test]
    fn parse_rejects_wrong_length_project_id() {
        let input = "[project]\nid = \"fooo\"\n";
        assert_eq!(
            parse(input).err(),
            Some(ConfigError::InvalidProjectId(Base32Error::WrongLength {
                expected: 3,
                got: 4,
            }))
        );
    }

    // @spec FMT-CFG-001, FMT-CFG-002
    #[test]
    fn parse_rejects_invalid_char_in_project_id() {
        let input = "[project]\nid = \"abl\"\n";
        assert_eq!(
            parse(input).err(),
            Some(ConfigError::InvalidProjectId(Base32Error::InvalidChar {
                ch: 'l',
                pos: 2,
            }))
        );
    }

    // @spec FMT-CFG-002
    #[test]
    fn parse_rejects_malformed_toml() {
        let input = "[project\nid = \"bar\"\n";
        match parse(input) {
            Err(ConfigError::Parse(_)) => {}
            other => panic!("expected ConfigError::Parse, got {other:?}"),
        }
    }

    // @spec FMT-CFG-002
    #[test]
    fn error_messages_point_users_at_vat_init() {
        for err in [
            ConfigError::MissingProject,
            ConfigError::MissingProjectId,
            ConfigError::ProjectIdNotString,
            ConfigError::InvalidProjectId(Base32Error::WrongLength {
                expected: 3,
                got: 4,
            }),
            ConfigError::NotFound(std::path::PathBuf::from("backlog/vat.toml")),
        ] {
            let msg = err.to_string();
            assert!(
                msg.contains("vat init"),
                "error message {msg:?} should mention `vat init`"
            );
        }
    }

    // @spec FMT-CFG-003
    #[test]
    fn round_trip_preserves_unknown_top_level_sections() {
        let input = "[project]\nid = \"bar\"\n\n[user]\nname = \"jared\"\n";
        let cfg = parse(input).unwrap();
        let out = cfg.serialize();
        let reparsed: toml::Value = out.parse().unwrap();
        assert_eq!(
            reparsed
                .get("user")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str()),
            Some("jared")
        );
        assert_eq!(
            reparsed
                .get("project")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str()),
            Some("bar")
        );
    }

    // @spec FMT-CFG-003
    #[test]
    fn round_trip_preserves_unknown_keys_in_project_table() {
        let input = "[project]\nid = \"bar\"\nextra = \"keep me\"\nnumber = 42\n";
        let cfg = parse(input).unwrap();
        let out = cfg.serialize();
        let reparsed: toml::Value = out.parse().unwrap();
        let project = reparsed.get("project").unwrap();
        assert_eq!(project.get("id").and_then(|v| v.as_str()), Some("bar"));
        assert_eq!(
            project.get("extra").and_then(|v| v.as_str()),
            Some("keep me")
        );
        assert_eq!(
            project.get("number").and_then(toml::Value::as_integer),
            Some(42)
        );
    }

    // @spec FMT-CFG-001
    #[test]
    fn new_creates_config_with_normalized_id() {
        let cfg = ProjectConfig::new("BaR").unwrap();
        assert_eq!(cfg.project_id(), "bar");
        let serialized = cfg.serialize();
        let reparsed = parse(&serialized).unwrap();
        assert_eq!(reparsed.project_id(), "bar");
    }

    // @spec FMT-CFG-001
    #[test]
    fn new_rejects_invalid_project_id() {
        assert!(matches!(
            ProjectConfig::new("ab"),
            Err(ConfigError::InvalidProjectId(Base32Error::WrongLength {
                expected: 3,
                got: 2,
            }))
        ));
    }

    // @spec FMT-CFG-001, FMT-CFG-003
    #[test]
    fn set_project_id_updates_value_and_preserves_unknown_sections() {
        let input = "[project]\nid = \"bar\"\n\n[user]\nname = \"jared\"\n";
        let mut cfg = parse(input).unwrap();
        cfg.set_project_id("BaZ").unwrap();
        assert_eq!(cfg.project_id(), "baz");
        let serialized = cfg.serialize();
        let reparsed = parse(&serialized).unwrap();
        assert_eq!(reparsed.project_id(), "baz");
        let raw: toml::Value = serialized.parse().unwrap();
        assert_eq!(
            raw.get("user")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str()),
            Some("jared")
        );
    }

    // @spec FMT-CFG-001
    #[test]
    fn set_project_id_rejects_invalid_id_without_mutating() {
        let input = "[project]\nid = \"bar\"\n";
        let mut cfg = parse(input).unwrap();
        assert!(cfg.set_project_id("bad-id").is_err());
        assert_eq!(cfg.project_id(), "bar");
    }
}
