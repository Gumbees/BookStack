-- Semantic search: embedded content chunks plus change-notification triggers
-- that keep the index fresh no matter which path mutates content (REST, MCP,
-- or the realtime collaboration engine's persistence loop).

CREATE TABLE embedding_chunks (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    org_id      BIGINT      NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    entity_type TEXT        NOT NULL CHECK (entity_type IN ('shelf', 'book', 'chapter', 'page')),
    entity_id   BIGINT      NOT NULL,
    chunk_index INT         NOT NULL,
    content     TEXT        NOT NULL,
    -- f32 little-endian vector bytes; similarity is computed in-process
    -- against an in-memory cache (pgvector is the scale-up path).
    embedding   BYTEA       NOT NULL,
    model       TEXT        NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (entity_type, entity_id, chunk_index)
);
CREATE INDEX embedding_chunks_org_idx ON embedding_chunks (org_id);
CREATE INDEX embedding_chunks_entity_idx ON embedding_chunks (entity_type, entity_id);

-- Notify the semantic indexer about content changes.
CREATE OR REPLACE FUNCTION bookstack_notify_content() RETURNS trigger AS $$
DECLARE
    rec RECORD;
    was_deleted BOOLEAN := false;
BEGIN
    IF TG_OP = 'DELETE' THEN
        rec := OLD;
        was_deleted := true;
    ELSE
        rec := NEW;
        was_deleted := (rec.deleted_at IS NOT NULL);
    END IF;
    PERFORM pg_notify(
        'bookstack_content',
        json_build_object(
            'entity_type', TG_ARGV[0],
            'entity_id', rec.id,
            'org_id', rec.org_id,
            'deleted', was_deleted
        )::text
    );
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER shelves_semantic_notify
    AFTER INSERT OR UPDATE OR DELETE ON shelves
    FOR EACH ROW EXECUTE FUNCTION bookstack_notify_content('shelf');
CREATE TRIGGER books_semantic_notify
    AFTER INSERT OR UPDATE OR DELETE ON books
    FOR EACH ROW EXECUTE FUNCTION bookstack_notify_content('book');
CREATE TRIGGER chapters_semantic_notify
    AFTER INSERT OR UPDATE OR DELETE ON chapters
    FOR EACH ROW EXECUTE FUNCTION bookstack_notify_content('chapter');
CREATE TRIGGER pages_semantic_notify
    AFTER INSERT OR UPDATE OR DELETE ON pages
    FOR EACH ROW EXECUTE FUNCTION bookstack_notify_content('page');
