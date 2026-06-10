mod backlog_file;
mod base32;
mod cmd_config;
mod cmd_init;
mod file_io;
mod item_file;
mod project_config;
mod readme_template;
mod sync;
mod tombstone;
mod user_config;

use std::io::{self, Write as _};

use clap::{Parser, Subcommand};

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
        Commands::Start { id } => cmd_start(id),
        Commands::Block { id, blocker_id } => cmd_block(id, blocker_id),
        Commands::Unblock { id } => cmd_unblock(id),
        Commands::Done { id } => cmd_done(id),
        Commands::Config { action } => match action {
            ConfigAction::Get { key } => cmd_config_get(&key),
            ConfigAction::Set { key, value } => cmd_config_set(&key, &value),
        },
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
    eprintln!(
        "warning: vat sync is partial — ID assignment not yet wired (vat-s9g); notes are extracted but IDs are not assigned"
    );
    let backlog_dir = std::path::Path::new("backlog");
    if let Err(e) = sync::run(backlog_dir) {
        eprintln!("vat sync: {e}");
        std::process::exit(1);
    }
}

fn cmd_start(_id: String) {
    eprintln!("vat start: not yet implemented");
    std::process::exit(1);
}

fn cmd_block(_id: String, _blocker_id: String) {
    eprintln!("vat block: not yet implemented");
    std::process::exit(1);
}

fn cmd_unblock(_id: String) {
    eprintln!("vat unblock: not yet implemented");
    std::process::exit(1);
}

fn cmd_done(_id: String) {
    eprintln!("vat done: not yet implemented");
    std::process::exit(1);
}

// @spec CMD-CFG-001, CMD-CFG-002
fn cmd_config_get(key: &str) {
    let backlog_dir = std::path::Path::new("backlog");
    match cmd_config::get(key, backlog_dir) {
        Ok(Some(value)) => println!("{value}"),
        Ok(None) => {}
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    }
}

// @spec CMD-CFG-003, CMD-CFG-004, CMD-CFG-005, CMD-CFG-006
fn cmd_config_set(key: &str, value: &str) {
    let backlog_dir = std::path::Path::new("backlog");
    if let Err(e) = cmd_config::set(key, value, backlog_dir) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
