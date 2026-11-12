-- Proxy commands the user allowed to run on this device, by the SHA-256 of
-- the command. The table stays on the device: sync and backups leave it out,
-- so a command that arrives from elsewhere is shown to the user before it
-- first runs, and any change to a command asks again.

CREATE TABLE trusted_proxy_commands (
    command_sha256  TEXT PRIMARY KEY NOT NULL,
    trusted_at      INTEGER NOT NULL
);
