-- Change UUID columns to BIGINT in a way that handles the conversion

-- 1. Users table
-- We need to drop references first
ALTER TABLE taps DROP CONSTRAINT IF EXISTS taps_owner_id_fkey;
ALTER TABLE audit_logs DROP CONSTRAINT IF EXISTS audit_logs_actor_id_fkey;
ALTER TABLE notifications DROP CONSTRAINT IF EXISTS notifications_user_id_fkey;

ALTER TABLE users ALTER COLUMN id TYPE BIGINT USING (('x' || left(replace(id::text, '-', ''), 16))::bit(64)::bigint);

-- 2. Taps table
ALTER TABLE api_keys DROP CONSTRAINT IF EXISTS api_keys_tap_id_fkey;
ALTER TABLE audit_logs DROP CONSTRAINT IF EXISTS audit_logs_tap_id_fkey;

ALTER TABLE taps ALTER COLUMN id TYPE BIGINT USING (('x' || left(replace(id::text, '-', ''), 16))::bit(64)::bigint);
ALTER TABLE taps ALTER COLUMN owner_id TYPE BIGINT USING (('x' || left(replace(owner_id::text, '-', ''), 16))::bit(64)::bigint);

-- 3. Api keys table
ALTER TABLE api_keys ALTER COLUMN id TYPE BIGINT USING (('x' || left(replace(id::text, '-', ''), 16))::bit(64)::bigint);
ALTER TABLE api_keys ALTER COLUMN tap_id TYPE BIGINT USING (('x' || left(replace(tap_id::text, '-', ''), 16))::bit(64)::bigint);
ALTER TABLE api_keys ALTER COLUMN id DROP DEFAULT;

-- 4. Audit logs table
ALTER TABLE audit_logs ALTER COLUMN id TYPE BIGINT USING (('x' || left(replace(id::text, '-', ''), 16))::bit(64)::bigint);
ALTER TABLE audit_logs ALTER COLUMN tap_id TYPE BIGINT USING (('x' || left(replace(tap_id::text, '-', ''), 16))::bit(64)::bigint);
ALTER TABLE audit_logs ALTER COLUMN actor_id TYPE BIGINT USING (('x' || left(replace(actor_id::text, '-', ''), 16))::bit(64)::bigint);
ALTER TABLE audit_logs ALTER COLUMN id DROP DEFAULT;

-- 5. Notifications table
ALTER TABLE notifications ALTER COLUMN id TYPE BIGINT USING (('x' || left(replace(id::text, '-', ''), 16))::bit(64)::bigint);
ALTER TABLE notifications ALTER COLUMN user_id TYPE BIGINT USING (('x' || left(replace(user_id::text, '-', ''), 16))::bit(64)::bigint);

-- Re-add constraints
ALTER TABLE taps ADD CONSTRAINT taps_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE audit_logs ADD CONSTRAINT audit_logs_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE notifications ADD CONSTRAINT notifications_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE api_keys ADD CONSTRAINT api_keys_tap_id_fkey FOREIGN KEY (tap_id) REFERENCES taps(id) ON DELETE CASCADE;
ALTER TABLE audit_logs ADD CONSTRAINT audit_logs_tap_id_fkey FOREIGN KEY (tap_id) REFERENCES taps(id) ON DELETE CASCADE;
