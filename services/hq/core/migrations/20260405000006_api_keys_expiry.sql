-- Add expires_at to api_keys table and remove scopes
ALTER TABLE api_keys ADD COLUMN expires_at TIMESTAMPTZ;
ALTER TABLE api_keys DROP COLUMN scopes;
