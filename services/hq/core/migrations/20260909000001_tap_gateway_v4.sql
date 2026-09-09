-- Which protocol a tap speaks.
--
-- Taps are run by third parties and cannot be migrated in lockstep, so both the
-- protofish3 taphub path and the WebSocket gateway run side by side and each
-- tap is switched over individually. Defaults to false: every existing tap
-- keeps using taphub until it is deliberately flipped.
ALTER TABLE taps ADD COLUMN gateway_v4 BOOLEAN NOT NULL DEFAULT FALSE;
