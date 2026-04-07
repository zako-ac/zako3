-- Change UUID columns to TEXT in a way that handles the conversion

-- 1. Users table
-- We need to drop references first
ALTER TABLE taps DROP CONSTRAINT IF EXISTS taps_owner_id_fkey;
ALTER TABLE audit_logs DROP CONSTRAINT IF EXISTS audit_logs_actor_id_fkey;
ALTER TABLE notifications DROP CONSTRAINT IF EXISTS notifications_user_id_fkey;

ALTER TABLE users ALTER COLUMN id DROP DEFAULT;
ALTER TABLE users ALTER COLUMN id TYPE TEXT USING (id::text);

-- 2. Taps table
ALTER TABLE api_keys DROP CONSTRAINT IF EXISTS api_keys_tap_id_fkey;
ALTER TABLE audit_logs DROP CONSTRAINT IF EXISTS audit_logs_tap_id_fkey;

ALTER TABLE taps ALTER COLUMN id DROP DEFAULT;
ALTER TABLE taps ALTER COLUMN id TYPE TEXT USING (id::text);
ALTER TABLE taps ALTER COLUMN owner_id TYPE TEXT USING (owner_id::text);

-- 3. Api keys table
ALTER TABLE api_keys ALTER COLUMN id DROP DEFAULT;
ALTER TABLE api_keys ALTER COLUMN id TYPE TEXT USING (id::text);
ALTER TABLE api_keys ALTER COLUMN tap_id TYPE TEXT USING (tap_id::text);

-- 4. Audit logs table
ALTER TABLE audit_logs ALTER COLUMN id DROP DEFAULT;
ALTER TABLE audit_logs ALTER COLUMN id TYPE TEXT USING (id::text);
ALTER TABLE audit_logs ALTER COLUMN tap_id TYPE TEXT USING (tap_id::text);
ALTER TABLE audit_logs ALTER COLUMN actor_id TYPE TEXT USING (actor_id::text);

-- 5. Notifications table
ALTER TABLE notifications ALTER COLUMN id DROP DEFAULT;
ALTER TABLE notifications ALTER COLUMN id TYPE TEXT USING (id::text);
ALTER TABLE notifications ALTER COLUMN user_id TYPE TEXT USING (user_id::text);

-- Re-add constraints
ALTER TABLE taps ADD CONSTRAINT taps_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE audit_logs ADD CONSTRAINT audit_logs_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE notifications ADD CONSTRAINT notifications_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE api_keys ADD CONSTRAINT api_keys_tap_id_fkey FOREIGN KEY (tap_id) REFERENCES taps(id) ON DELETE CASCADE;
ALTER TABLE audit_logs ADD CONSTRAINT audit_logs_tap_id_fkey FOREIGN KEY (tap_id) REFERENCES taps(id) ON DELETE CASCADE;
