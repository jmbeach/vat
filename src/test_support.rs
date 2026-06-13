//! Shared helpers for the command unit-test modules (`cmd_start`, `cmd_block`,
//! `cmd_unblock`, `cmd_done`). Compiled only under `cfg(test)`. Command-specific
//! helpers (e.g. `write_user_config`, `write_item`) stay local to their module.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// Frontmatter header prefixing a minimal valid `backlog.md` (`version: 1`).
pub(crate) const HEADER: &str = "---\nversion: 1\n---\n\n";

/// Create a `backlog/` directory under `dir` and return its path.
pub(crate) fn make_backlog_dir(dir: &TempDir) -> PathBuf {
    let backlog = dir.path().join("backlog");
    std::fs::create_dir_all(&backlog).unwrap();
    backlog
}

/// Write `content` to `<backlog>/backlog.md`.
pub(crate) fn write_backlog(backlog: &Path, content: &str) {
    std::fs::write(backlog.join("backlog.md"), content).unwrap();
}

/// Read `<backlog>/backlog.md` back as a string.
pub(crate) fn read_backlog(backlog: &Path) -> String {
    std::fs::read_to_string(backlog.join("backlog.md")).unwrap()
}
