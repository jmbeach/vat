// @spec FMT-CFG-001, FMT-CFG-002, FMT-CFG-003

#![allow(dead_code)]

use thiserror::Error;
use toml::Value;

use crate::base32::{self, Base32Error};

const PROJECT_ID_LEN: usize = 3;

#[derive(Debug, Error, PartialEq)]
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
}

#[derive(Debug)]
pub(crate) struct ProjectConfig {
    document: Value,
    project_id: String,
}

impl ProjectConfig {
    // @spec FMT-CFG-001
    pub(crate) fn new(_project_id: &str) -> Result<Self, ConfigError> {
        unimplemented!()
    }

    pub(crate) fn project_id(&self) -> &str {
        unimplemented!()
    }

    // @spec FMT-CFG-001
    pub(crate) fn set_project_id(&mut self, _id: &str) -> Result<(), ConfigError> {
        unimplemented!()
    }

    // @spec FMT-CFG-003
    pub(crate) fn serialize(&self) -> String {
        unimplemented!()
    }
}

// @spec FMT-CFG-001, FMT-CFG-002, FMT-CFG-003
pub(crate) fn parse(_input: &str) -> Result<ProjectConfig, ConfigError> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::{ConfigError, ProjectConfig, parse};
    use crate::base32::Base32Error;

    // @spec FMT-CFG-001
    #[test]
    fn parse_accepts_valid_project_id() {
        let input = "[project]\nid = \"foo\"\n";
        let cfg = parse(input).expect("valid config should parse");
        assert_eq!(cfg.project_id(), "foo");
    }

    // @spec FMT-CFG-001
    #[test]
    fn parse_normalizes_uppercase_project_id_to_lowercase() {
        let input = "[project]\nid = \"FoO\"\n";
        let cfg = parse(input).expect("mixed-case base32 should parse");
        assert_eq!(cfg.project_id(), "foo");
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
        let input = "[project]\nid = \"fol\"\n";
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
        let input = "[project\nid = \"foo\"\n";
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
        let input = "[project]\nid = \"foo\"\n\n[user]\nname = \"jared\"\n";
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
            Some("foo")
        );
    }

    // @spec FMT-CFG-003
    #[test]
    fn round_trip_preserves_unknown_keys_in_project_table() {
        let input = "[project]\nid = \"foo\"\nextra = \"keep me\"\nnumber = 42\n";
        let cfg = parse(input).unwrap();
        let out = cfg.serialize();
        let reparsed: toml::Value = out.parse().unwrap();
        let project = reparsed.get("project").unwrap();
        assert_eq!(project.get("id").and_then(|v| v.as_str()), Some("foo"));
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
        let input = "[project]\nid = \"foo\"\n\n[user]\nname = \"jared\"\n";
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
        let input = "[project]\nid = \"foo\"\n";
        let mut cfg = parse(input).unwrap();
        assert!(cfg.set_project_id("bad-id").is_err());
        assert_eq!(cfg.project_id(), "foo");
    }
}
