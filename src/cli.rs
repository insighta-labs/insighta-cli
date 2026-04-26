use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "insighta", about = "Insighta Labs CLI", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Login,
    Logout,
    Whoami,
}
