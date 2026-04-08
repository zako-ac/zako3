CREATE TABLE playback_actions (
    id TEXT PRIMARY KEY,
    action_type TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    actor_discord_user_id TEXT NOT NULL,
    track_snapshot JSONB NOT NULL,
    queue_snapshot JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    undone_at TIMESTAMPTZ,
    undone_by_discord_user_id TEXT
);

CREATE INDEX idx_playback_actions_guild_id ON playback_actions(guild_id);
CREATE INDEX idx_playback_actions_actor ON playback_actions(actor_discord_user_id);
CREATE INDEX idx_playback_actions_created_at ON playback_actions(created_at DESC);
