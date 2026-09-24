-- Saved passwords, encrypted with one data key (envelope encryption).
--
-- Each secret is AES-256-GCM ciphertext under the data key. The data key is
-- never stored in the clear: `secret_key_wraps` holds it encrypted by a key
-- kept in the OS credential store ('system') and/or by a key derived from the
-- user's master password ('password'). Changing the master password or the
-- storage mode only re-wraps the data key.

CREATE TABLE secret_key_wraps (
    kind         TEXT PRIMARY KEY NOT NULL CHECK(kind IN ('system', 'password')),
    -- Argon2id salt and cost for the 'password' wrap; NULL for 'system'.
    kdf          TEXT CHECK(kdf IS NULL OR json_valid(kdf)),
    nonce        BLOB NOT NULL,
    wrapped_key  BLOB NOT NULL,
    created_at   INTEGER NOT NULL
);

-- `key` is the secret's lookup name: a profile id, `<profile id>:jump:...`
-- for a jump host, or a profile name for sessions saved before profiles had
-- ids. Secrets of a profile go with it; name-keyed ones have no profile_id.
CREATE TABLE secrets (
    key          TEXT PRIMARY KEY NOT NULL CHECK(length(key) > 0),
    profile_id   TEXT REFERENCES profiles(id) ON DELETE CASCADE,
    nonce        BLOB NOT NULL,
    ciphertext   BLOB NOT NULL,
    updated_at   INTEGER NOT NULL
);

CREATE INDEX secrets_profile_id_idx ON secrets(profile_id);

-- A password can be saved before its profile (a new profile is connected
-- before it is saved); link it once the profile appears.
CREATE TRIGGER profiles_link_secrets AFTER INSERT ON profiles
BEGIN
    UPDATE secrets SET profile_id = NEW.id
    WHERE profile_id IS NULL
      AND (key = NEW.id OR substr(key, 1, length(NEW.id) + 6) = NEW.id || ':jump:');
END;
