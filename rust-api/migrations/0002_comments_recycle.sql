-- Comments and recycle-bin (deletions) support, mirroring the tool surface of
-- bees-roadhouse/bookstack-mcp.

CREATE TABLE comments (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    page_id    BIGINT      NOT NULL REFERENCES pages (id) ON DELETE CASCADE,
    parent_id  BIGINT      REFERENCES comments (id) ON DELETE SET NULL,
    markdown   TEXT        NOT NULL DEFAULT '',
    html       TEXT        NOT NULL DEFAULT '',
    created_by BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    updated_by BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX comments_page_idx ON comments (page_id, id);

-- One row per soft-delete, powering list/restore/destroy recycle-bin tools.
-- Child entities soft-deleted in the same transaction share the parent's
-- deleted_at timestamp (transaction_timestamp()), which restore relies on.
CREATE TABLE deletions (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    entity_type TEXT        NOT NULL CHECK (entity_type IN ('shelf', 'book', 'chapter', 'page')),
    entity_id   BIGINT      NOT NULL,
    entity_name TEXT        NOT NULL,
    deleted_by  BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    deleted_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX deletions_entity_idx ON deletions (entity_type, entity_id);
