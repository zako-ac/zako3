use axum::{Json, extract::{State, Path}, http::StatusCode};
use hq_core::Service;
use hq_types::hq::guild::GuildSummaryDto;
use std::collections::HashMap;
use std::sync::Arc;

use crate::middleware::auth::{AuthUser, AdminUser};

fn map_error(e: hq_core::CoreError) -> (StatusCode, String) {
    match e {
        hq_core::CoreError::NotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
        hq_core::CoreError::InvalidInput(_) => (StatusCode::BAD_REQUEST, e.to_string()),
        hq_core::CoreError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, e.to_string()),
        hq_core::CoreError::Forbidden(_) => (StatusCode::FORBIDDEN, e.to_string()),
        hq_core::CoreError::Conflict(_) => (StatusCode::CONFLICT, e.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/guilds/me",
    responses(
        (status = 200, description = "Guilds where user is a member and bot is present", body = Vec<GuildSummaryDto>)
    ),
    security(("bearer_auth" = []))
)]
#[tracing::instrument(skip(service), fields(user_id = %user_id), name = "handler.get_my_guilds")]
pub async fn get_my_guilds(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Vec<GuildSummaryDto>>, (StatusCode, String)> {
    tracing::info!("Fetching user's guilds");

    // Get full user object (which includes oauth_access_token)
    let db_user = service
        .auth
        .get_full_user(&user_id.to_string())
        .await
        .map_err(map_error)?;

    let discord_id_str = db_user.discord_user_id.0.clone();
    let discord_id: u64 = discord_id_str.parse().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid discord id".into(),
        )
    })?;

    tracing::debug!(discord_id = %discord_id_str, has_oauth_token = db_user.oauth_access_token.is_some(), "User loaded from database");

    // Get voice channel info (used to enrich guild data)
    let voice_channels = service
        .voice_state
        .get_user_channels(&discord_id_str)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let voice_map: HashMap<u64, _> = voice_channels
        .into_iter()
        .map(|loc| (loc.guild_id, loc))
        .collect();

    // Try to fetch guilds from Discord API using OAuth token
    let guild_infos = if let Some(token) = &db_user.oauth_access_token {
        tracing::info!("User has OAuth token, attempting Discord API call");
        match service.auth.fetch_discord_guilds_for_user(&discord_id_str, token).await {
            Ok(guilds) => {
                tracing::info!(
                    count = guilds.len(),
                    "Successfully fetched {} guilds from Discord API",
                    guilds.len()
                );
                guilds
            }
            Err(e) => {
                // Fall back to bot cache if API call fails
                tracing::warn!(error = %e, "Discord API call failed, falling back to bot cache");
                let resolver = service.name_resolver_slot.get();
                let cache_guilds = resolver
                    .map(|r| r.guilds_for_user(discord_id))
                    .unwrap_or_default();
                tracing::info!(
                    count = cache_guilds.len(),
                    "Fetched {} guilds from bot cache (fallback)",
                    cache_guilds.len()
                );
                cache_guilds
            }
        }
    } else {
        // No OAuth token, fall back to bot cache
        tracing::info!("No OAuth token available, using bot cache");
        let resolver = service.name_resolver_slot.get();
        let cache_guilds = resolver
            .map(|r| r.guilds_for_user(discord_id))
            .unwrap_or_default();
        tracing::info!(
            count = cache_guilds.len(),
            "Fetched {} guilds from bot cache",
            cache_guilds.len()
        );
        cache_guilds
    };

    tracing::debug!(total_guilds = guild_infos.len(), "Intersecting with bot's guild list");

    // Get bot's guild list and intersect with user's guilds
    let resolver = service.name_resolver_slot.get();
    let bot_guild_ids: std::collections::HashSet<u64> = resolver
        .map(|r| r.bot_guilds().into_iter().collect())
        .unwrap_or_default();

    tracing::info!(
        user_guilds = guild_infos.len(),
        bot_guilds = bot_guild_ids.len(),
        "Guild lists before intersection"
    );

    // Filter to only mutual guilds (where bot is present)
    let mutual_guild_infos: Vec<_> = guild_infos
        .into_iter()
        .filter(|g| {
            let is_mutual = bot_guild_ids.contains(&g.id);
            if !is_mutual {
                tracing::debug!(
                    guild_id = g.id,
                    guild_name = %g.name,
                    "Excluding guild: bot not present"
                );
            }
            is_mutual
        })
        .collect();

    tracing::info!(
        mutual_guilds = mutual_guild_infos.len(),
        "Found {} mutual guilds (user + bot)",
        mutual_guild_infos.len()
    );

    let dtos = mutual_guild_infos
        .into_iter()
        .map(|g| {
            let vc = voice_map.get(&g.id);
            GuildSummaryDto {
                guild_id: g.id.to_string(),
                guild_name: g.name,
                guild_icon_url: g.icon_url,
                active_channel_id: vc.map(|v| v.channel_id.to_string()),
                active_channel_name: vc.map(|v| v.channel_name.clone()),
                can_manage: g.can_manage,
            }
        })
        .collect::<Vec<_>>();

    tracing::info!(
        total_guilds = dtos.len(),
        "Returning {} guilds to client",
        dtos.len()
    );
    Ok(Json(dtos))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{id}/guilds",
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "Guilds where user is a member and bot is present", body = Vec<GuildSummaryDto>)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_user_guilds(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path(id): Path<String>,
) -> Result<Json<Vec<GuildSummaryDto>>, (StatusCode, String)> {
    // Get target user's discord ID
    let db_user = service
        .auth
        .get_full_user(&id)
        .await
        .map_err(map_error)?;

    let discord_id_str = db_user.discord_user_id.0.clone();
    let discord_id: u64 = discord_id_str.parse().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid discord id".into(),
        )
    })?;

    // Use bot cache resolver (no OAuth token needed for admin view)
    let resolver = service.name_resolver_slot.get();
    let guild_infos = resolver
        .map(|r| r.guilds_for_user(discord_id))
        .unwrap_or_default();

    // Get bot's guild list and intersect with user's guilds
    let bot_guild_ids: std::collections::HashSet<u64> = resolver
        .map(|r| r.bot_guilds().into_iter().collect())
        .unwrap_or_default();

    // Filter to only mutual guilds (where bot is present)
    let mutual_guild_infos: Vec<_> = guild_infos
        .into_iter()
        .filter(|g| bot_guild_ids.contains(&g.id))
        .collect();

    let dtos = mutual_guild_infos
        .into_iter()
        .map(|g| GuildSummaryDto {
            guild_id: g.id.to_string(),
            guild_name: g.name,
            guild_icon_url: g.icon_url,
            active_channel_id: None,
            active_channel_name: None,
            can_manage: g.can_manage,
        })
        .collect::<Vec<_>>();

    Ok(Json(dtos))
}
