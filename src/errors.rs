use thiserror::Error;

/// Wrapper for errors that are directly user-visible (wrong key, bad value, etc.).
/// Causes `classify_exit_code` in `main.rs` to return 1 rather than the
/// internal-error default of 2.
#[derive(Debug, Error)]
#[error("{0}")]
pub(crate) struct UserError(pub(crate) String);
