use poise::serenity_prelude as serenity;
use serenity::{Context, EventHandler, async_trait, model::voice::VoiceState};
use zako3_states::VoiceStateService;

pub struct VoiceStateHandler {
    pub voice_state_service: VoiceStateService,
}

#[async_trait]
impl EventHandler for VoiceStateHandler {
    async fn cache_ready(&self, ctx: Context, guilds: Vec<serenity::GuildId>) {
        for guild_id in guilds {
            let voice_states: Vec<_> = {
                let guild = ctx.cache.guild(guild_id);
                match guild {
                    None => continue,
                    Some(g) => g
                        .voice_states
                        .iter()
                        .filter_map(|(user_id, vs)| {
                            let channel_id = vs.channel_id?;
                            let channel_name = g
                                .channels
                                .get(&channel_id)
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| channel_id.get().to_string());
                            Some((
                                user_id.to_string(),
                                guild_id.get(),
                                channel_id.get(),
                                g.name.clone(),
                                channel_name,
                            ))
                        })
                        .collect(),
                }
            };

            for (discord_user_id, gid, cid, guild_name, channel_name) in voice_states {
                let _ = self
                    .voice_state_service
                    .set_user_channel(&discord_user_id, gid, cid, guild_name, channel_name)
                    .await;
            }
        }
    }

    async fn voice_state_update(&self, ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
        let discord_user_id = new.user_id.to_string();

        let guild_id_typed = match new.guild_id {
            Some(g) => g,
            None => return,
        };
        let guild_id = guild_id_typed.get();

        match new.channel_id {
            Some(ch) => {
                // Extract names while holding cache refs, then drop before .await
                let (guild_name, channel_name) = {
                    let guild = ctx.cache.guild(guild_id_typed);
                    let gn = guild
                        .as_ref()
                        .map(|g| g.name.clone())
                        .unwrap_or_else(|| guild_id.to_string());
                    let cn = guild
                        .as_ref()
                        .and_then(|g| g.channels.get(&ch))
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| ch.get().to_string());
                    (gn, cn)
                };
                let _ = self
                    .voice_state_service
                    .set_user_channel(
                        &discord_user_id,
                        guild_id,
                        ch.get(),
                        guild_name,
                        channel_name,
                    )
                    .await;
            }
            None => {
                let _ = self
                    .voice_state_service
                    .remove_user_from_guild(&discord_user_id, guild_id)
                    .await;
            }
        }
    }
}
