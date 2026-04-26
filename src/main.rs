use clap::Parser;
use cli::{Cli, Commands};

mod auth;
mod cli;
mod client;
mod credentials;
mod error;
mod output;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Login => auth::login().await,
        Commands::Logout => auth::logout().await,
        Commands::Whoami => auth::whoami(),
    };

    if let Err(e) = result {
        output::print_error(&e.to_string());
        std::process::exit(1);
    }
}
