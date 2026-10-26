-- Profile groups become a tree of records with ids, and a profile points at
-- its group by id instead of naming it.
--
-- A group from before keeps its name as its id. Another device that migrates
-- on its own then gives the same group the same id, and sync files and
-- backups written before this change, which key groups by name, still find
-- them.

DROP INDEX profiles_group_name_idx;
ALTER TABLE profiles DROP COLUMN group_name;

ALTER TABLE profile_groups RENAME TO legacy_profile_groups;

-- `parent_id` is not a foreign key: sync writes groups in any order and may
-- briefly leave a group whose parent is gone, which is repaired afterwards.
CREATE TABLE profile_groups (
    id          TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    parent_id   TEXT,
    name        TEXT NOT NULL CHECK(length(trim(name)) > 0),
    updated_at  INTEGER
);

CREATE INDEX profile_groups_parent_id_idx ON profile_groups(parent_id);

CREATE TRIGGER profile_groups_touch_insert AFTER INSERT ON profile_groups
BEGIN
    UPDATE profile_groups
    SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;

CREATE TRIGGER profile_groups_touch_update AFTER UPDATE OF parent_id, name ON profile_groups
WHEN OLD.parent_id IS NOT NEW.parent_id OR OLD.name IS NOT NEW.name
BEGIN
    UPDATE profile_groups
    SET updated_at = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    WHERE id = NEW.id;
END;

INSERT OR IGNORE INTO profile_groups (id, name)
SELECT trim(name), trim(name) FROM legacy_profile_groups WHERE length(trim(name)) > 0;

-- A group used to exist as soon as a profile named it.
INSERT OR IGNORE INTO profile_groups (id, name)
SELECT DISTINCT trim(json_extract(data, '$.group')), trim(json_extract(data, '$.group'))
FROM profiles
WHERE json_type(data, '$.group') = 'text' AND length(trim(json_extract(data, '$.group'))) > 0;

UPDATE profiles
SET data = CASE
    WHEN json_type(data, '$.group') = 'text' AND length(trim(json_extract(data, '$.group'))) > 0
        THEN json_set(json_remove(data, '$.group'), '$.group_id', trim(json_extract(data, '$.group')))
    ELSE json_remove(data, '$.group')
END
WHERE json_type(data, '$.group') IS NOT NULL;

DROP TABLE legacy_profile_groups;
