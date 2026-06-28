//! Playback control tools (mirrors `handlers::playback`). All require an authenticated user.

use crate::mcp::auth::require_user;
use crate::mcp::support::{json_ok, map_core, mk_tool, parse_args, run};
use hq_core::Service;
use hq_types::hq::UserId;
use hq_types::hq::playback::{EditQueueDto, PauseTrackDto, ResumeTrackDto, SkipDto, StopTrackDto};
use mcpkit::server::capability::tools::ToolService;
use mcpkit::types::ToolOutput;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize)]
struct UndoArgs {
    action_id: String,
}

/// Resolve the caller's Discord id (mirrors `handlers::playback::get_discord_id`).
async fn discord_id(svc: &Arc<Service>, uid: &UserId) -> Result<String, ToolOutput> {
    let user = svc
        .auth
        .get_user(&uid.to_string())
        .await
        .map_err(map_core)?;
    Ok(user.discord_id)
}

fn parse_id(s: &str, what: &str) -> Result<u64, ToolOutput> {
    s.parse::<u64>()
        .map_err(|_| ToolOutput::error(format!("invalid {what}")))
}

pub fn register(tools: &mut ToolService, service: &Arc<Service>) {
    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_playback_state",
            "Get current playback state for the user's guilds.",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let did = discord_id(&svc, &uid).await?;
                let states = svc
                    .playback
                    .get_state_for_user(&did)
                    .await
                    .map_err(map_core)?;
                json_ok(&states)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "stop_track",
            "Stop a track. Body is a StopTrackDto.",
            json!({"type": "object"}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let did = discord_id(&svc, &uid).await?;
                let p: StopTrackDto = parse_args(args)?;
                let action = svc
                    .playback
                    .stop_track(
                        parse_id(&p.guild_id, "guild_id")?,
                        parse_id(&p.channel_id, "channel_id")?,
                        &p.track_id,
                        &did,
                    )
                    .await
                    .map_err(map_core)?;
                json_ok(&action)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "pause_track",
            "Pause a track. Body is a PauseTrackDto.",
            json!({"type": "object"}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let did = discord_id(&svc, &uid).await?;
                let p: PauseTrackDto = parse_args(args)?;
                let action = svc
                    .playback
                    .pause_track(
                        parse_id(&p.guild_id, "guild_id")?,
                        parse_id(&p.channel_id, "channel_id")?,
                        &p.track_id,
                        &did,
                    )
                    .await
                    .map_err(map_core)?;
                json_ok(&action)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "resume_track",
            "Resume a track. Body is a ResumeTrackDto.",
            json!({"type": "object"}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let did = discord_id(&svc, &uid).await?;
                let p: ResumeTrackDto = parse_args(args)?;
                let action = svc
                    .playback
                    .resume_track(
                        parse_id(&p.guild_id, "guild_id")?,
                        parse_id(&p.channel_id, "channel_id")?,
                        &p.track_id,
                        &did,
                    )
                    .await
                    .map_err(map_core)?;
                json_ok(&action)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "skip_music",
            "Skip the current music track. Body is a SkipDto.",
            json!({"type": "object"}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let did = discord_id(&svc, &uid).await?;
                let p: SkipDto = parse_args(args)?;
                let action = svc
                    .playback
                    .skip_music(
                        parse_id(&p.guild_id, "guild_id")?,
                        parse_id(&p.channel_id, "channel_id")?,
                        &did,
                    )
                    .await
                    .map_err(map_core)?;
                json_ok(&action)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "edit_queue",
            "Edit the playback queue. Body is an EditQueueDto.",
            json!({"type": "object"}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let did = discord_id(&svc, &uid).await?;
                let p: EditQueueDto = parse_args(args)?;
                let action = svc.playback.edit_queue(p, &did).await.map_err(map_core)?;
                json_ok(&action)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "undo_playback_action",
            "Undo a previous playback action by id.",
            json!({"type": "object", "properties": {"action_id": {"type": "string"}}, "required": ["action_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let did = discord_id(&svc, &uid).await?;
                let UndoArgs { action_id } = parse_args(args)?;
                let action = svc.playback.undo_action(&action_id, &did).await.map_err(map_core)?;
                json_ok(&action)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_playback_history",
            "Get recent playback action history for the user's guilds.",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let did = discord_id(&svc, &uid).await?;
                let history = svc.playback.get_history(&did, 50).await.map_err(map_core)?;
                json_ok(&history)
            })
        },
    );
}
