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
        Commands::Init(args) => agit::cli::commands::init::execute(args),
        Commands::Record(args) => agit::cli::commands::record::execute(args),
        Commands::Status(args) => agit::cli::commands::status::execute(args),
        Commands::Log(args) => agit::cli::commands::log::execute(args),
        Commands::Show(args) => agit::cli::commands::show::execute(args),
        Commands::Commit(args) => agit::cli::commands::commit::execute(args),
        Commands::Add(args) => agit::cli::commands::add::execute(args),
        Commands::Push(args) => agit::cli::commands::push::execute(args),
        Commands::Pull(args) => agit::cli::commands::pull::execute(args),
        Commands::Migrate(args) => agit::cli::commands::migrate::execute(args),
        Commands::Serve(args) => agit::cli::commands::serve::execute(args),
        Commands::Search(args) => agit::cli::commands::search::execute(args),
        Commands::Reset(args) => agit::cli::commands::reset::execute(args),
        Commands::Sync(args) => agit::cli::commands::sync::execute(args),
        Commands::Hooks(args) => agit::cli::commands::hooks::execute(args),
    }
}
