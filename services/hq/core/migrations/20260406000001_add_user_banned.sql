-- Add banned column to users table
ALTER TABLE users ADD COLUMN banned BOOLEAN NOT NULL DEFAULT FALSE;
