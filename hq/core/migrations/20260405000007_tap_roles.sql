-- Add new jsonb roles array column
ALTER TABLE taps ADD COLUMN roles JSONB NOT NULL DEFAULT '[]'::jsonb;

-- Migrate existing role to roles array if it exists
UPDATE taps SET roles = jsonb_build_array(role) WHERE role IS NOT NULL;

-- Drop the old role column
ALTER TABLE taps DROP COLUMN role;