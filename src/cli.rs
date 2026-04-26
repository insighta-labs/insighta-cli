use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "insighta", about = "Insighta Labs CLI", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Authenticate with GitHub
    Login,
    /// Sign out and clear stored credentials
    Logout,
    /// Show the currently authenticated user
    Whoami,
    /// Profile commands
    Profiles {
        #[command(subcommand)]
        command: ProfileCommands,
    },
}

#[derive(Subcommand)]
pub enum ProfileCommands {
    /// List profiles with optional filters
    List {
        #[arg(long)]
        gender: Option<String>,
        #[arg(long)]
        country: Option<String>,
        #[arg(long, name = "age-group")]
        age_group: Option<String>,
        #[arg(long, name = "min-age")]
        min_age: Option<u8>,
        #[arg(long, name = "max-age")]
        max_age: Option<u8>,
        #[arg(long, name = "sort-by")]
        sort_by: Option<String>,
        #[arg(long)]
        order: Option<String>,
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "10")]
        limit: u32,
    },
    /// Get a single profile by ID
    Get { id: String },
    /// Search profiles using natural language
    Search {
        query: String,
        #[arg(long, default_value = "1")]
        page: u32,
        #[arg(long, default_value = "10")]
        limit: u32,
    },
    /// Create a new profile (admin only)
    Create {
        #[arg(long)]
        name: String,
    },
    /// Export profiles to CSV and save to current directory
    Export {
        #[arg(long, default_value = "csv")]
        format: String,
        #[arg(long)]
        gender: Option<String>,
        #[arg(long)]
        country: Option<String>,
    },
}
