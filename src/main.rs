use clap::Parser;
use cli::{Cli, Commands};

mod auth;
mod cli;
mod client;
mod config;
mod credentials;
mod error;
mod output;
mod profiles;
mod tests;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Login => auth::login().await,
        Commands::Logout => auth::logout().await,
        Commands::Whoami => auth::whoami().await,
        Commands::Profiles { command } => profiles::handle(command).await,
    };

    if let Err(e) = result {
        output::print_error(&e.to_string());
        std::process::exit(1);
    }
}
