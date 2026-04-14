-- Add oauth_access_token column to users table for OAuth guild fetching
ALTER TABLE users ADD COLUMN oauth_access_token TEXT;
