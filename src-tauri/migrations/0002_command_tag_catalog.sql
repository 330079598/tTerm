CREATE TABLE command_tag_catalog (
    normalized_tag TEXT PRIMARY KEY CHECK(length(normalized_tag) BETWEEN 1 AND 64),
    tag            TEXT NOT NULL CHECK(length(tag) BETWEEN 1 AND 64)
);

INSERT OR IGNORE INTO command_tag_catalog (normalized_tag, tag)
SELECT normalized_tag, MIN(tag) FROM command_tags GROUP BY normalized_tag;
