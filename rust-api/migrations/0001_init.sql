-- BookStack-rs initial PostgreSQL schema.
-- Mirrors the core BookStack entity model (shelves > books > chapters > pages)
-- with native Postgres full-text search and CRDT collaboration state.

CREATE TABLE users (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name          TEXT        NOT NULL,
    email         TEXT        NOT NULL,
    password_hash TEXT        NOT NULL,
    role          TEXT        NOT NULL DEFAULT 'viewer'
                  CHECK (role IN ('admin', 'editor', 'viewer')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX users_email_uq ON users (lower(email));

CREATE TABLE api_tokens (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id      BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    token_id     TEXT        NOT NULL UNIQUE,
    secret_hash  TEXT        NOT NULL,
    expires_at   TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE shelves (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name        TEXT        NOT NULL,
    slug        TEXT        NOT NULL,
    description TEXT        NOT NULL DEFAULT '',
    created_by  BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    updated_by  BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ,
    search_vector TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(name, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(description, '')), 'C')
    ) STORED
);
CREATE UNIQUE INDEX shelves_slug_uq ON shelves (slug) WHERE deleted_at IS NULL;
CREATE INDEX shelves_search_idx ON shelves USING GIN (search_vector);

CREATE TABLE books (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name        TEXT        NOT NULL,
    slug        TEXT        NOT NULL,
    description TEXT        NOT NULL DEFAULT '',
    created_by  BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    updated_by  BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ,
    search_vector TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(name, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(description, '')), 'C')
    ) STORED
);
CREATE UNIQUE INDEX books_slug_uq ON books (slug) WHERE deleted_at IS NULL;
CREATE INDEX books_search_idx ON books USING GIN (search_vector);

CREATE TABLE shelf_books (
    shelf_id BIGINT NOT NULL REFERENCES shelves (id) ON DELETE CASCADE,
    book_id  BIGINT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    "order"  INT    NOT NULL DEFAULT 0,
    PRIMARY KEY (shelf_id, book_id)
);

CREATE TABLE chapters (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    book_id     BIGINT      NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    slug        TEXT        NOT NULL,
    description TEXT        NOT NULL DEFAULT '',
    priority    INT         NOT NULL DEFAULT 0,
    created_by  BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    updated_by  BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ,
    search_vector TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(name, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(description, '')), 'C')
    ) STORED
);
CREATE UNIQUE INDEX chapters_slug_uq ON chapters (book_id, slug) WHERE deleted_at IS NULL;
CREATE INDEX chapters_search_idx ON chapters USING GIN (search_vector);
CREATE INDEX chapters_book_idx ON chapters (book_id);

CREATE TABLE pages (
    id             BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    book_id        BIGINT      NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    chapter_id     BIGINT      REFERENCES chapters (id) ON DELETE SET NULL,
    name           TEXT        NOT NULL,
    slug           TEXT        NOT NULL,
    markdown       TEXT        NOT NULL DEFAULT '',
    html           TEXT        NOT NULL DEFAULT '',
    priority       INT         NOT NULL DEFAULT 0,
    draft          BOOLEAN     NOT NULL DEFAULT false,
    revision_count INT         NOT NULL DEFAULT 0,
    -- Persisted Yjs (yrs) CRDT document state for realtime collaboration.
    ydoc_state     BYTEA,
    created_by     BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    updated_by     BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at     TIMESTAMPTZ,
    search_vector TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(name, '')), 'A') ||
        setweight(to_tsvector('english', left(coalesce(markdown, ''), 400000)), 'C')
    ) STORED
);
CREATE UNIQUE INDEX pages_slug_uq ON pages (book_id, slug) WHERE deleted_at IS NULL;
CREATE INDEX pages_search_idx ON pages USING GIN (search_vector);
CREATE INDEX pages_book_idx ON pages (book_id);
CREATE INDEX pages_chapter_idx ON pages (chapter_id);

CREATE TABLE page_revisions (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    page_id         BIGINT      NOT NULL REFERENCES pages (id) ON DELETE CASCADE,
    revision_number INT         NOT NULL,
    name            TEXT        NOT NULL,
    markdown        TEXT        NOT NULL DEFAULT '',
    summary         TEXT        NOT NULL DEFAULT '',
    created_by      BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX page_revisions_page_idx ON page_revisions (page_id, revision_number DESC);

CREATE TABLE tags (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    entity_type TEXT   NOT NULL CHECK (entity_type IN ('shelf', 'book', 'chapter', 'page')),
    entity_id   BIGINT NOT NULL,
    name        TEXT   NOT NULL,
    value       TEXT   NOT NULL DEFAULT '',
    "order"     INT    NOT NULL DEFAULT 0
);
CREATE INDEX tags_entity_idx ON tags (entity_type, entity_id);
CREATE INDEX tags_name_idx ON tags (lower(name));
