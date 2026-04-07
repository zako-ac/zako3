use clap::{Args, Subcommand};

#[derive(Args)]
pub struct HqCommands {
    #[command(subcommand)]
    pub command: HqSubcommand,
}

#[derive(Subcommand)]
pub enum HqSubcommand {
    /// User management commands
    User {
        #[command(subcommand)]
        command: UserSubcommand,
    },
    /// Tap management commands
    Tap {
        #[command(subcommand)]
        command: TapSubcommand,
    },
}

#[derive(Subcommand)]
pub enum UserSubcommand {
    /// List all users
    List,
    /// Get user details
    Get { id: String },
    /// Grant admin permission to a user
    Admin { id: String },
    /// Revoke admin permission from a user
    Revoke { id: String },
}

#[derive(Subcommand)]
pub enum TapSubcommand {
    /// List all taps
    List {
        /// Filter by owner ID
        #[arg(long)]
        owner: Option<String>,
    },
    /// Get tap details
    Get { id: String },
    /// Delete a tap
    Delete { id: String },
}
