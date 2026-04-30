CREATE TABLE use_history (
    id BIGINT PRIMARY KEY,
    tap_id TEXT NOT NULL,
    user_id TEXT,
    discord_user_id TEXT,
    ars_length INT NOT NULL,
    trace_id TEXT UNIQUE,
    cache_hit BOOLEAN NOT NULL,
    success BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_use_history_tap_id ON use_history (tap_id, created_at DESC);
