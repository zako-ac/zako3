CREATE TABLE guild_settings (
    guild_id    TEXT PRIMARY KEY,
    settings    JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE user_guild_settings (
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    guild_id    TEXT NOT NULL,
    settings    JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, guild_id)
);

-- Singleton row enforced by CHECK constraint
CREATE TABLE global_settings (
    id          INT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    settings    JSONB,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
