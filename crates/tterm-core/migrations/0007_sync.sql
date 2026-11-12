-- WebDAV sync.
--
-- Profiles and tunnels record when they last changed, so a record edited on
-- two devices between syncs resolves to the later edit. Triggers keep the
-- time for every write path; rows from before this migration have none.

ALTER TABLE profiles ADD COLUMN updated_at INTEGER;
ALTER TABLE tunnels ADD COLUMN updated_at INTEGER;

CREATE TRIGGER profiles_touch_insert AFTER INSERT ON profiles
BEGIN
    UPDATE profiles SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;

CREATE TRIGGER profiles_touch_update AFTER UPDATE OF position, data ON profiles
WHEN OLD.position IS NOT NEW.position OR OLD.data IS NOT NEW.data
BEGIN
    UPDATE profiles SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;

CREATE TRIGGER tunnels_touch_insert AFTER INSERT ON tunnels
BEGIN
    UPDATE tunnels SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;

CREATE TRIGGER tunnels_touch_update AFTER UPDATE OF position, data ON tunnels
WHEN OLD.position IS NOT NEW.position OR OLD.data IS NOT NEW.data
BEGIN
    UPDATE tunnels SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;

-- What this device and the server agreed on at the last sync (the merge
-- base), for the server, user and folder in `target`. The base holds saved
-- passwords, so it is encrypted with the data key like `secrets`.
CREATE TABLE sync_state (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    target           TEXT NOT NULL,
    base_nonce       BLOB NOT NULL,
    base_ciphertext  BLOB NOT NULL,
    remote_etag      TEXT,
    synced_at        INTEGER NOT NULL
);
