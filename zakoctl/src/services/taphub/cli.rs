use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args)]
pub struct TaphubCommands {
    #[command(subcommand)]
    pub command: TaphubSubcommand,
}

#[derive(Subcommand)]
pub enum TaphubSubcommand {
    /// Request audio directly from taphub and play it live or save to a WAV file.
    Request {
        /// taphub transport addr, e.g. `127.0.0.1:4000`
        #[arg(long, env = "ZAKO_TAPHUB_ADDR")]
        server_addr: String,
        /// TLS SNI / server name presented to taphub
        #[arg(long, default_value = "localhost")]
        server_name: String,
        /// Root CA certificate PEM (must validate taphub's transport cert)
        #[arg(long)]
        cert_file: PathBuf,
        /// Tap ID to route the request to
        #[arg(long)]
        tap_id: String,
        /// Discord user ID to send with the request
        #[arg(long, default_value = "0")]
        discord_user_id: String,
        /// Audio request string (e.g. `yt:sine`, `yt:https://...`)
        #[arg(long)]
        ars: String,
        /// Output WAV file path. Mutually exclusive with `--play`.
        #[arg(long, conflicts_with = "play")]
        output: Option<PathBuf>,
        /// Play live via the default audio output device. Mutually exclusive with `--output`.
        #[arg(long, conflicts_with = "output")]
        play: bool,
    },
}
