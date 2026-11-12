-- Proxy servers that SSH connections reach their first host through. Hosts
-- refer to a server by id, so editing a server changes every host using it.
-- `data` holds the server without its password, which is in `secrets` under
-- `proxy:<id>`. `updated_at` lets sync resolve a server edited on two devices.

CREATE TABLE proxies (
    id          TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    position    INTEGER NOT NULL,
    data        TEXT NOT NULL CHECK(json_valid(data)),
    updated_at  INTEGER
);

CREATE INDEX proxies_position_idx ON proxies(position);

CREATE TRIGGER proxies_touch_insert AFTER INSERT ON proxies
BEGIN
    UPDATE proxies SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;

CREATE TRIGGER proxies_touch_update AFTER UPDATE OF position, data ON proxies
WHEN OLD.position IS NOT NEW.position OR OLD.data IS NOT NEW.data
BEGIN
    UPDATE proxies SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;
