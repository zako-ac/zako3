//! Who is allowed to act on a guild's settings.
//!
//! Guild settings are "configurable by guild administrators" (`docs/settings.md`),
//! and the only thing that knows who holds Discord's `MANAGE_GUILD` in a guild is
//! Discord itself. The web client learns the answer from
//! `GET /api/v1/guilds/me` (`canManage`) and hides the editor behind it — which is
//! a hint to the user, not a permission check. This is the same answer, resolved
//! where the write actually happens.

use super::{GuildInfo, Service};
use crate::{CoreError, CoreResult};
use hq_types::hq::UserId;

/// Whether any of `guilds` grants management of `guild_id`.
///
/// Kept separate from the service call so the predicate can be tested without
/// Discord, a database or a resolver.
pub fn guilds_grant_manage(guilds: &[GuildInfo], guild_id: u64) -> bool {
    guilds.iter().any(|g| g.id == guild_id && g.can_manage)
}

impl Service {
    /// Whether `user_id` holds Discord's `MANAGE_GUILD` in `guild_id`.
    ///
    /// The user's guild list is read through their stored OAuth token (cached in
    /// Redis for five minutes); when the token is absent or rejected — expired
    /// tokens are the normal case — it falls back to the bot's member cache, which
    /// resolves `MANAGE_GUILD` from Discord's own member data. That is the same
    /// path `GET /api/v1/guilds/me` takes, so a guild the web UI offers for editing
    /// is a guild this accepts, and a guild nothing can vouch for is a `false`.
    pub async fn user_can_manage_guild(
        &self,
        user_id: &UserId,
        guild_id: &str,
    ) -> CoreResult<bool> {
        let guild_id: u64 = guild_id
            .parse()
            .map_err(|_| CoreError::InvalidInput(format!("invalid guild id: {guild_id}")))?;

        let user = self.auth.get_full_user(&user_id.to_string()).await?;
        let discord_id: u64 = user
            .discord_user_id
            .0
            .parse()
            .map_err(|_| CoreError::InvalidInput("invalid discord id".to_string()))?;

        let guilds = match user.oauth_access_token.as_deref() {
            Some(token) => match self
                .auth
                .fetch_discord_guilds_for_user(&user.discord_user_id.0, token)
                .await
            {
                Ok(guilds) => guilds,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        discord_id,
                        "Discord guild lookup failed, falling back to the bot member cache"
                    );
                    self.bot_guilds_for_user(discord_id)
                }
            },
            None => self.bot_guilds_for_user(discord_id),
        };

        Ok(guilds_grant_manage(&guilds, guild_id))
    }

    /// The guilds the bot's member cache knows this Discord user to be in.
    ///
    /// Empty when no resolver is installed (a process running without the bot),
    /// which is the safe direction: no resolver means no way to prove a guild
    /// permission, so nothing is granted.
    fn bot_guilds_for_user(&self, discord_id: u64) -> Vec<GuildInfo> {
        self.name_resolver_slot
            .get()
            .map(|resolver| resolver.guilds_for_user(discord_id))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::{GuildInfo, guilds_grant_manage};

    fn guild(id: u64, can_manage: bool) -> GuildInfo {
        GuildInfo {
            id,
            name: format!("guild {id}"),
            icon_url: None,
            can_manage,
        }
    }

    #[test]
    fn manage_is_granted_only_for_a_matching_guild_with_the_permission() {
        let guilds = vec![guild(1, true), guild(2, false)];

        assert!(guilds_grant_manage(&guilds, 1));
        assert!(!guilds_grant_manage(&guilds, 2));
    }

    #[test]
    fn a_guild_the_user_is_not_in_grants_nothing() {
        assert!(!guilds_grant_manage(&[guild(1, true)], 2));
        assert!(!guilds_grant_manage(&[], 1));
    }
}
