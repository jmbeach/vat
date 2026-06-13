mod backlog_file;
mod base32;
mod bullet;
mod cmd_completions;
mod cmd_config;
mod cmd_done;
mod cmd_init;
mod cmd_start;
mod errors;
mod file_io;
mod id_assignment;
mod item_file;
mod project_config;
mod readme_template;
mod sync;
mod tombstone;
mod user_config;

use std::io::{self, Write as _};

use clap::{Parser, Subcommand};

use crate::cmd_completions::SupportedShell;

#[derive(Parser)]
#[command(
    name = "vat",
    about = "Versioned Addressable Tasks — backlog in plain markdown"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create backlog/ and write initial files
    Init {
        /// 3-char Crockford base32 project prefix (prompted if omitted)
        prefix: Option<String>,
    },
    /// Assign IDs, extract notes, normalize markers
    Sync,
    /// Mark a task in-progress and claim it
    Start {
        /// Task ID (e.g. foo-7k2)
        id: String,
    },
    /// Add a blocked-by marker to a task
    Block {
        /// Task to block
        id: String,
        /// Task that is blocking it
        blocker_id: String,
    },
    /// Remove the blocked-by marker from a task
    Unblock {
        /// Task ID
        id: String,
    },
    /// Complete a task and remove it from the backlog
    Done {
        /// Task ID
        id: String,
    },
    /// Read or write config values
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Generate shell completions and write them to stdout
    // @spec CMD-COMP-003
    #[command(hide = true)]
    Completions {
        /// Shell to generate completions for
        shell: SupportedShell,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print a config value
    Get {
        /// Config key (user.name or project.id)
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key (user.name or project.id)
        key: String,
        /// Value to store
        value: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { prefix } => cmd_init(prefix),
        Commands::Sync => cmd_sync(),
        Commands::Start { id } => cmd_start(&id),
        Commands::Block { id, blocker_id } => cmd_block(id, blocker_id),
        Commands::Unblock { id } => cmd_unblock(id),
        Commands::Done { id } => cmd_done(&id),
        Commands::Config { action } => match action {
            ConfigAction::Get { key } => cmd_config_get(&key),
            ConfigAction::Set { key, value } => cmd_config_set(&key, &value),
        },
        // @spec CMD-COMP-001, CMD-COMP-002
        Commands::Completions { shell } => cmd_completions(shell),
    }
}

// @spec CMD-COMP-001, CMD-COMP-005
fn cmd_completions(shell: SupportedShell) {
    if let Err(e) = cmd_completions::run(shell) {
        // A consumer closing the pipe early (`vat completions bash | head`)
        // is normal, not an error.
        if e.kind() == io::ErrorKind::BrokenPipe {
            return;
        }
        eprintln!("error: failed to write completions: {e}");
        std::process::exit(2);
    }
}

// @spec CMD-INIT-002, CMD-INIT-003
fn cmd_init(prefix: Option<String>) {
    let prefix_str = prefix.unwrap_or_else(prompt_for_prefix);
    match cmd_init::init(std::path::Path::new("."), &prefix_str) {
        Ok(msg) => println!("{msg}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

fn prompt_for_prefix() -> String {
    print!("Project prefix (3 Crockford base32 chars): ");
    io::stdout().flush().ok();
    let mut input = String::new();
    // A closed stdin (non-interactive: CI pipeline, `vat init < /dev/null`)
    // yields Ok(0); surface that explicitly instead of silently falling
    // through to a confusing "invalid prefix" error on the empty string.
    match io::stdin().read_line(&mut input) {
        Ok(0) | Err(_) => {
            eprintln!("error: could not read prefix from stdin; try `vat init <prefix>`");
            std::process::exit(1);
        }
        Ok(_) => {}
    }
    input.trim().to_string()
}

fn cmd_sync() {
    let backlog_dir = std::path::Path::new("backlog");
    if let Err(e) = sync::run(backlog_dir) {
        eprintln!("vat sync: {e}");
        std::process::exit(1);
    }
}

// @spec CMD-START-001, CMD-START-002, CMD-START-003
fn cmd_start(id: &str) {
    let backlog_dir = std::path::Path::new("backlog");
    match cmd_start::run(id, backlog_dir) {
        Ok(msg) => println!("{msg}"),
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(classify_exit_code(&e));
        }
    }
}

fn cmd_block(_id: String, _blocker_id: String) {
    eprintln!("vat block: not yet implemented");
    std::process::exit(1);
}

fn cmd_unblock(_id: String) {
    eprintln!("vat unblock: not yet implemented");
    std::process::exit(1);
}

// @spec CMD-DONE-001, CMD-DONE-002, CMD-DONE-003, CMD-DONE-004, CMD-DONE-005
fn cmd_done(id: &str) {
    let backlog_dir = std::path::Path::new("backlog");
    match cmd_done::run(id, backlog_dir) {
        Ok(msg) => println!("{msg}"),
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(classify_exit_code(&e));
        }
    }
}

// @spec CMD-CFG-001, CMD-CFG-002
fn cmd_config_get(key: &str) {
    let backlog_dir = std::path::Path::new("backlog");
    match cmd_config::get(key, backlog_dir) {
        Ok(Some(value)) => println!("{value}"),
        Ok(None) => {}
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(classify_exit_code(&e));
        }
    }
}

// @spec CMD-CFG-003, CMD-CFG-004, CMD-CFG-005, CMD-CFG-006
fn cmd_config_set(key: &str, value: &str) {
    let backlog_dir = std::path::Path::new("backlog");
    if let Err(e) = cmd_config::set(key, value, backlog_dir) {
        eprintln!("error: {e:#}");
        std::process::exit(classify_exit_code(&e));
    }
}

// @spec CMD-EXIT-001, CMD-EXIT-002, CMD-EXIT-003
//
// Maps an anyhow error to an exit code by searching the cause chain for known
// typed error variants. IO failures and unexpected parse failures are internal
// (exit 2); everything else that reaches the top level is user-facing (exit 1).
fn classify_exit_code(e: &anyhow::Error) -> i32 {
    use backlog_file::UnsupportedVersion;
    use errors::UserError;
    use project_config::ConfigError;
    use tombstone::TombstoneError;
    use user_config::UserConfigError;

    for cause in e.chain() {
        // Matches are exhaustive on purpose: a new error variant must make an
        // explicit exit-code choice here rather than silently inheriting one.
        if let Some(ce) = cause.downcast_ref::<ConfigError>() {
            return match ce {
                ConfigError::Io(_) | ConfigError::Parse(_) => 2,
                ConfigError::MissingProject
                | ConfigError::MissingProjectId
                | ConfigError::ProjectIdNotString
                | ConfigError::InvalidProjectId(_)
                | ConfigError::NotFound(_) => 1,
            };
        }
        if let Some(ue) = cause.downcast_ref::<UserConfigError>() {
            return match ue {
                UserConfigError::Io(_) | UserConfigError::Parse(_) => 2,
                UserConfigError::UserNotATable
                | UserConfigError::UserNameNotString
                | UserConfigError::UserNameEmpty
                | UserConfigError::NotFound(_)
                | UserConfigError::NoHome => 1,
            };
        }
        if let Some(te) = cause.downcast_ref::<TombstoneError>() {
            return match te {
                // A genuine OS error is internal; the structural variants are all
                // user-addressable (corrupt tombstone line, or backlog/ missing or
                // shadowed by a regular file — "run `vat init`").
                TombstoneError::Io(_) => 2,
                TombstoneError::MalformedLine { .. }
                | TombstoneError::NoBacklogDir { .. }
                | TombstoneError::BacklogNotDirectory { .. } => 1,
            };
        }
        if cause.downcast_ref::<UnsupportedVersion>().is_some() {
            return 1;
        }
        if cause.downcast_ref::<UserError>().is_some() {
            return 1;
        }
    }
    2
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use super::classify_exit_code;
    use crate::backlog_file::{SUPPORTED_MAJOR, UnsupportedVersion};
    use crate::base32::Base32Error;
    use crate::errors::UserError;
    use crate::project_config::ConfigError;
    use crate::tombstone::TombstoneError;
    use crate::user_config::UserConfigError;

    fn anyhow(e: impl std::error::Error + Send + Sync + 'static) -> anyhow::Error {
        anyhow::Error::from(e)
    }

    fn io_err() -> io::Error {
        io::Error::new(io::ErrorKind::PermissionDenied, "denied")
    }

    // @spec CMD-EXIT-003
    #[test]
    fn config_io_error_is_internal() {
        let e = anyhow(ConfigError::Io(io_err()));
        assert_eq!(classify_exit_code(&e), 2);
    }

    // @spec CMD-EXIT-003
    #[test]
    fn config_parse_error_is_internal() {
        let e = anyhow(ConfigError::Parse("bad toml".to_owned()));
        assert_eq!(classify_exit_code(&e), 2);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn config_project_id_not_string_is_user() {
        let e = anyhow(ConfigError::ProjectIdNotString);
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn config_not_found_is_user() {
        let e = anyhow(ConfigError::NotFound(PathBuf::from("backlog/vat.toml")));
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn config_missing_project_is_user() {
        let e = anyhow(ConfigError::MissingProject);
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn config_invalid_project_id_is_user() {
        let e = anyhow(ConfigError::InvalidProjectId(Base32Error::WrongLength {
            expected: 3,
            got: 2,
        }));
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-003
    #[test]
    fn user_config_io_error_is_internal() {
        let e = anyhow(UserConfigError::Io(io_err()));
        assert_eq!(classify_exit_code(&e), 2);
    }

    // @spec CMD-EXIT-003
    #[test]
    fn user_config_parse_error_is_internal() {
        let e = anyhow(UserConfigError::Parse("bad toml".to_owned()));
        assert_eq!(classify_exit_code(&e), 2);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn user_config_user_not_a_table_is_user() {
        let e = anyhow(UserConfigError::UserNotATable);
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn user_config_name_not_string_is_user() {
        let e = anyhow(UserConfigError::UserNameNotString);
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn user_config_no_home_is_user() {
        let e = anyhow(UserConfigError::NoHome);
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn user_config_name_empty_is_user() {
        let e = anyhow(UserConfigError::UserNameEmpty);
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn unsupported_version_is_user() {
        let e = anyhow(UnsupportedVersion {
            found: SUPPORTED_MAJOR + 1,
            supported: SUPPORTED_MAJOR,
        });
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn user_error_is_user() {
        let e = anyhow(UserError("unknown config key: foo".to_owned()));
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-003
    #[test]
    fn unrecognized_error_defaults_to_internal() {
        let e = anyhow::anyhow!("something unexpected happened");
        assert_eq!(classify_exit_code(&e), 2);
    }

    // @spec CMD-EXIT-002, CMD-EXIT-003
    // context() wrapping must not hide the underlying typed variant.
    #[test]
    fn context_wrapped_io_error_is_internal() {
        let e = anyhow::Error::from(ConfigError::Io(io_err())).context("reading vat.toml");
        assert_eq!(classify_exit_code(&e), 2);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn context_wrapped_user_error_is_user() {
        let e = anyhow::Error::from(UserError("cannot change project.id".to_owned()))
            .context("validating");
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-003
    #[test]
    fn tombstone_io_error_is_internal() {
        let e = anyhow(TombstoneError::Io(io_err()));
        assert_eq!(classify_exit_code(&e), 2);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn tombstone_malformed_line_is_user() {
        let e = anyhow(TombstoneError::MalformedLine {
            line_no: 3,
            content: "not-an-id".to_owned(),
        });
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn tombstone_no_backlog_dir_is_user() {
        let e = anyhow(TombstoneError::NoBacklogDir {
            path: PathBuf::from("backlog"),
        });
        assert_eq!(classify_exit_code(&e), 1);
    }

    // @spec CMD-EXIT-002
    #[test]
    fn tombstone_backlog_not_directory_is_user() {
        let e = anyhow(TombstoneError::BacklogNotDirectory {
            path: PathBuf::from("backlog"),
        });
        assert_eq!(classify_exit_code(&e), 1);
    }
}
