-- Custom themes and edited preset themes, which the web view used to keep in
-- localStorage. `data` is the theme as the web view defines it; its `id` is
-- the row's. `updated_at` lets sync resolve a theme edited on two devices.

CREATE TABLE custom_themes (
    id          TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    position    INTEGER NOT NULL,
    data        TEXT NOT NULL CHECK(json_valid(data)),
    updated_at  INTEGER
);

CREATE INDEX custom_themes_position_idx ON custom_themes(position);

CREATE TRIGGER custom_themes_touch_insert AFTER INSERT ON custom_themes
BEGIN
    UPDATE custom_themes SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;

CREATE TRIGGER custom_themes_touch_update AFTER UPDATE OF position, data ON custom_themes
WHEN OLD.position IS NOT NEW.position OR OLD.data IS NOT NEW.data
BEGIN
    UPDATE custom_themes SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;
