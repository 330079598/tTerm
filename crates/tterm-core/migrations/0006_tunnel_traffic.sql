-- Traffic a tunnel has carried across every run, since `since` (unix ms): the
-- first run counted or the last reset. Rows are machine-local and stay out of
-- backups. There is no foreign key to `tunnels` on purpose: restoring a backup
-- rewrites every rule, which a cascade would turn into wiping the counters.
-- Rows are removed with their rule, and orphans when the rules are replaced.

CREATE TABLE tunnel_traffic (
    tunnel_id   TEXT PRIMARY KEY NOT NULL,
    bytes_up    INTEGER NOT NULL DEFAULT 0,
    bytes_down  INTEGER NOT NULL DEFAULT 0,
    connections INTEGER NOT NULL DEFAULT 0,
    since       INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
