-- Add index on actor_id for performance
CREATE INDEX IF NOT EXISTS idx_audit_logs_actor_id ON audit_logs(actor_id);

-- Make actor_id nullable to support System actions
ALTER TABLE audit_logs ALTER COLUMN actor_id DROP NOT NULL;
