mod backlog_file;
mod base32;
mod project_config;

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
            ConfigAction::Get { key } => cmd_config_get(key),
            ConfigAction::Set { key, value } => cmd_config_set(key, value),
        },
    }
}

fn cmd_init(_prefix: Option<String>) {
    eprintln!("vat init: not yet implemented");
    std::process::exit(1);
}

fn cmd_sync() {
    eprintln!("vat sync: not yet implemented");
    std::process::exit(1);
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

fn cmd_config_get(_key: String) {
    eprintln!("vat config get: not yet implemented");
    std::process::exit(1);
}

fn cmd_config_set(_key: String, _value: String) {
    eprintln!("vat config set: not yet implemented");
    std::process::exit(1);
}
