use clap::{Args, Subcommand};

#[derive(Args)]
pub struct ConfigCommands {
    #[command(subcommand)]
    pub command: ConfigSubcommands,
}

#[derive(Subcommand)]
pub enum ConfigSubcommands {
    /// View current configuration
    View,
    /// Get current context
    CurrentContext,
    /// Set the current context
    UseContext {
        #[arg(help = "Name of the context to use")]
        name: String,
    },
    /// Set a context entry in kubeconfig
    SetContext {
        #[arg(help = "Name of the context")]
        name: String,
        #[arg(long, help = "Address of the Audio Engine gRPC server")]
        ae_addr: String,
        #[arg(long, help = "Default Guild ID for this context")]
        default_guild_id: Option<String>,
    },
    /// Delete a context from the config
    DeleteContext {
        #[arg(help = "Name of the context to delete")]
        name: String,
    },
    /// Set an alias
    SetAlias {
        #[arg(help = "The alias key")]
        key: String,
        #[arg(help = "The value to alias")]
        value: String,
    },
    /// Get an alias
    GetAlias {
        #[arg(help = "The alias key")]
        key: String,
    },
    /// Delete an alias
    DeleteAlias {
        #[arg(help = "The alias key")]
        key: String,
    },
    /// List all aliases
    GetAliases,
}
