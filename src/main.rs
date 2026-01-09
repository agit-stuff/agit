//! AGIT CLI entry point.

use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use agit::cli::{Cli, Commands};
use agit::error::Result;

fn main() {
    // Initialize tracing - output to stderr to not interfere with stdio protocols
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::from_default_env())
        .init();

    if let Err(e) = run() {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init(args)) => agit::cli::commands::init::execute(args),
        Some(Commands::Record(args)) => agit::cli::commands::record::execute(args),
        Some(Commands::Status(args)) => agit::cli::commands::status::execute(args),
        Some(Commands::Log(args)) => agit::cli::commands::log::execute(args),
        Some(Commands::Show(args)) => agit::cli::commands::show::execute(args),
        Some(Commands::Commit(args)) => agit::cli::commands::commit::execute(args),
        Some(Commands::Server(args)) => agit::cli::commands::server::execute(args),
        None => {
            // No command provided - launch TUI
            agit::tui::run()
        }
    }
}
