pub mod cli;
pub mod client;
pub mod formatter;

use anyhow::Result;
use cli::{HqCommands, HqSubcommand, TapSubcommand, UserSubcommand};
use client::HqClient;
use hq_types::hq::rpc::HqRpcClient;

pub async fn handle_command(
    hq_addr: String,
    hq_admin_token: Option<String>,
    cmd: HqCommands,
) -> Result<()> {
    let client = HqClient::new(&hq_addr, hq_admin_token.as_deref())?;
    let rpc = client.inner();

    match cmd.command {
        HqSubcommand::User { command } => match command {
            UserSubcommand::List => {
                let users = rpc.list_users().await?;
                println!("{}", formatter::format_user_list(&users));
            }
            UserSubcommand::Get { id } => {
                let user = rpc.get_user(id).await?;
                if let Some(user) = user {
                    println!("{}", formatter::format_user_details(&user));
                } else {
                    println!("User not found");
                }
            }
            UserSubcommand::Admin { id } => {
                let user = rpc.get_user(id.clone()).await?;
                if let Some(mut user) = user {
                    if !user.permissions.iter().any(|p| p == "admin") {
                        user.permissions.push("admin".to_string());
                        let updated = rpc.update_user_permissions(id, user.permissions).await?;
                        println!("User {} is now admin", updated.username.0);
                    } else {
                        println!("User {} is already admin", user.username.0);
                    }
                } else {
                    println!("User not found");
                }
            }
            UserSubcommand::Revoke { id } => {
                let user = rpc.get_user(id.clone()).await?;
                if let Some(mut user) = user {
                    if user.permissions.iter().any(|p| p == "admin") {
                        user.permissions.retain(|p| p != "admin");
                        let updated = rpc.update_user_permissions(id, user.permissions).await?;
                        println!("User {} is no longer admin", updated.username.0);
                    } else {
                        println!("User {} is not an admin", user.username.0);
                    }
                } else {
                    println!("User not found");
                }
            }
        },
        HqSubcommand::Tap { command } => match command {
            TapSubcommand::List { owner } => {
                let taps = rpc.list_taps(owner).await?;
                println!("{}", formatter::format_tap_list(&taps));
            }
            TapSubcommand::Get { id } => {
                let tap = rpc.get_tap(id).await?;
                if let Some(tap) = tap {
                    println!("{}", formatter::format_tap_details(&tap));
                } else {
                    println!("Tap not found");
                }
            }
            TapSubcommand::Delete { id } => {
                rpc.delete_tap(id.clone()).await?;
                println!("Tap {} deleted", id);
            }
        },
    }

    Ok(())
}
