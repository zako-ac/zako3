-- Create Users Table
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    discord_user_id TEXT NOT NULL UNIQUE,
    username TEXT NOT NULL,
    avatar_url TEXT,
    email TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create Taps Table
CREATE TABLE IF NOT EXISTS taps (
    id UUID PRIMARY KEY,
    owner_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    description TEXT,
    occupation TEXT NOT NULL DEFAULT 'base',
    permission JSONB NOT NULL DEFAULT '"owner_only"',
    role TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create Index on owner_id for faster lookups
CREATE INDEX IF NOT EXISTS idx_taps_owner_id ON taps(owner_id);
