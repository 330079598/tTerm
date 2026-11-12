CREATE TABLE saved_commands (
    id                  TEXT PRIMARY KEY,
    name                TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 120),
    command_text        TEXT NOT NULL CHECK(length(command_text) BETWEEN 1 AND 65536),
    description         TEXT NOT NULL DEFAULT '',
    scope_type          TEXT NOT NULL DEFAULT 'global'
                        CHECK(scope_type IN ('global', 'profile')),
    scope_id            TEXT,
    shell_type          TEXT NOT NULL DEFAULT 'any',
    platform            TEXT NOT NULL DEFAULT 'any'
                        CHECK(platform IN ('any', 'windows', 'linux', 'macos')),
    is_favorite         INTEGER NOT NULL DEFAULT 0 CHECK(is_favorite IN (0, 1)),
    confirm_before_run  INTEGER NOT NULL DEFAULT 0 CHECK(confirm_before_run IN (0, 1)),
    sort_order          INTEGER NOT NULL DEFAULT 0,
    use_count           INTEGER NOT NULL DEFAULT 0 CHECK(use_count >= 0),
    last_used_at        INTEGER,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    CHECK (
        (scope_type = 'global' AND scope_id IS NULL) OR
        (scope_type = 'profile' AND scope_id IS NOT NULL)
    )
);

CREATE INDEX idx_saved_commands_scope
    ON saved_commands(scope_type, scope_id);

CREATE INDEX idx_saved_commands_favorite_order
    ON saved_commands(is_favorite DESC, sort_order, updated_at DESC);

CREATE TABLE command_tags (
    command_id     TEXT NOT NULL,
    tag            TEXT NOT NULL CHECK(length(tag) BETWEEN 1 AND 64),
    normalized_tag TEXT NOT NULL CHECK(length(normalized_tag) BETWEEN 1 AND 64),
    PRIMARY KEY(command_id, normalized_tag),
    FOREIGN KEY(command_id) REFERENCES saved_commands(id) ON DELETE CASCADE
);

CREATE INDEX idx_command_tags_normalized_tag
    ON command_tags(normalized_tag);

CREATE TABLE command_variables (
    command_id    TEXT NOT NULL,
    name          TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 64),
    label         TEXT NOT NULL CHECK(length(label) BETWEEN 1 AND 120),
    value_type    TEXT NOT NULL DEFAULT 'text'
                  CHECK(value_type IN ('text', 'number', 'choice', 'secret')),
    default_value TEXT,
    options_json  TEXT,
    is_required   INTEGER NOT NULL DEFAULT 1 CHECK(is_required IN (0, 1)),
    position      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(command_id, name),
    FOREIGN KEY(command_id) REFERENCES saved_commands(id) ON DELETE CASCADE,
    CHECK(value_type != 'secret' OR default_value IS NULL)
);

CREATE VIRTUAL TABLE saved_commands_fts USING fts5(
    command_id UNINDEXED,
    name,
    command_text,
    description,
    tags,
    tokenize = 'unicode61'
);
