-- What serial send bars sent, per connection: `connection_key` is
-- `profile:<id>` for a saved profile, else `port:<port>`. The `id` is derived
-- from the key, mode and text, so the same send on two devices is one record
-- when they sync, the later `sent_at` winning.

CREATE TABLE serial_send_history (
    id              TEXT PRIMARY KEY NOT NULL CHECK(length(id) > 0),
    connection_key  TEXT NOT NULL CHECK(length(connection_key) > 0),
    mode            TEXT NOT NULL CHECK(mode IN ('text', 'hex')),
    text            TEXT NOT NULL,
    sent_at         INTEGER NOT NULL
);

CREATE INDEX serial_send_history_connection_idx
    ON serial_send_history(connection_key, sent_at);

-- A profile's history goes with it, on every path that deletes profiles.
CREATE TRIGGER profiles_drop_serial_send_history AFTER DELETE ON profiles
BEGIN
    DELETE FROM serial_send_history WHERE connection_key = 'profile:' || OLD.id;
END;
