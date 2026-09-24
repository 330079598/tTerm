-- Profiles can store a dedicated sudo password under `<profile id>:sudo`.
-- Link those keys to their profile like jump host keys, so they are deleted
-- with it and a password saved before its profile is linked on insert.

DROP TRIGGER IF EXISTS profiles_link_secrets;

CREATE TRIGGER profiles_link_secrets AFTER INSERT ON profiles
BEGIN
    UPDATE secrets SET profile_id = NEW.id
    WHERE profile_id IS NULL
      AND (key = NEW.id
           OR substr(key, 1, length(NEW.id) + 6) = NEW.id || ':jump:'
           OR key = NEW.id || ':sudo');
END;
