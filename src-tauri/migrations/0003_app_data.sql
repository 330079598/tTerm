-- Data that used to live in JSON files in the config directory.
--
-- Profiles and tunnels keep their serialized model in `data` so fields can be
-- added to the Rust types without a migration; the columns the app queries on
-- are generated from it and cannot drift. Secrets never appear in `data`: the
-- secret fields of the models are skipped during serialization.

CREATE TABLE profiles (
    id          TEXT PRIMARY KEY NOT NULL,
    position    INTEGER NOT NULL,
    data        TEXT NOT NULL CHECK(json_valid(data)),
    name        TEXT GENERATED ALWAYS AS (json_extract(data, '$.name')) VIRTUAL,
    group_name  TEXT GENERATED ALWAYS AS (coalesce(json_extract(data, '$.group'), '')) VIRTUAL
);

CREATE INDEX profiles_position_idx ON profiles(position);
CREATE INDEX profiles_group_name_idx ON profiles(group_name);

-- Groups created explicitly, including empty ones. Groups that only appear on
-- profiles are not listed here.
CREATE TABLE profile_groups (
    name TEXT PRIMARY KEY NOT NULL CHECK(length(trim(name)) > 0)
);

-- A tunnel outlives the profile it points at so the user can re-point it,
-- which is why `profile_id` is not a foreign key.
CREATE TABLE tunnels (
    id          TEXT PRIMARY KEY NOT NULL,
    position    INTEGER NOT NULL,
    data        TEXT NOT NULL CHECK(json_valid(data)),
    profile_id  TEXT GENERATED ALWAYS AS (json_extract(data, '$.profileId')) VIRTUAL
);

CREATE INDEX tunnels_position_idx ON tunnels(position);
CREATE INDEX tunnels_profile_id_idx ON tunnels(profile_id);

-- Trusted host keys. Jump hosts are recorded under a synthetic profile name
-- without a profile id, and lookups fall back to the name for entries saved
-- before ids were recorded, so `profile_id` is optional and not a foreign key.
CREATE TABLE known_hosts (
    id            INTEGER PRIMARY KEY,
    profile_id    TEXT,
    profile_name  TEXT NOT NULL,
    host          TEXT NOT NULL,
    port          INTEGER NOT NULL CHECK(port BETWEEN 0 AND 65535),
    algorithm     TEXT NOT NULL,
    fingerprint   TEXT NOT NULL,
    trusted_at    INTEGER NOT NULL
);

CREATE INDEX known_hosts_endpoint_idx ON known_hosts(host, port);

CREATE TABLE sftp_last_directories (
    host       TEXT NOT NULL,
    port       INTEGER NOT NULL CHECK(port BETWEEN 0 AND 65535),
    username   TEXT NOT NULL,
    last_path  TEXT NOT NULL,
    PRIMARY KEY (host, port, username)
);

CREATE TABLE app_meta (
    key    TEXT PRIMARY KEY NOT NULL,
    value  TEXT NOT NULL
);
