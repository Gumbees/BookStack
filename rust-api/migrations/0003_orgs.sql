-- Multi-tenancy: organizations. Users belong to any number of orgs with a
-- per-org role; every content entity is owned by exactly one org. Existing
-- content migrates into a seeded "Default" org (id 1).

CREATE TABLE orgs (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       TEXT        NOT NULL,
    slug       TEXT        NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE org_members (
    org_id     BIGINT      NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    user_id    BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role       TEXT        NOT NULL DEFAULT 'viewer'
               CHECK (role IN ('admin', 'editor', 'viewer')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, user_id)
);
CREATE INDEX org_members_user_idx ON org_members (user_id);

INSERT INTO orgs (name, slug) VALUES ('Default', 'default');

-- Existing users become members of the default org, carrying over the role
-- they held when roles were instance-wide.
INSERT INTO org_members (org_id, user_id, role)
SELECT 1, id, role FROM users;

ALTER TABLE shelves   ADD COLUMN org_id BIGINT NOT NULL DEFAULT 1 REFERENCES orgs (id) ON DELETE CASCADE;
ALTER TABLE books     ADD COLUMN org_id BIGINT NOT NULL DEFAULT 1 REFERENCES orgs (id) ON DELETE CASCADE;
ALTER TABLE chapters  ADD COLUMN org_id BIGINT NOT NULL DEFAULT 1 REFERENCES orgs (id) ON DELETE CASCADE;
ALTER TABLE pages     ADD COLUMN org_id BIGINT NOT NULL DEFAULT 1 REFERENCES orgs (id) ON DELETE CASCADE;
ALTER TABLE deletions ADD COLUMN org_id BIGINT NOT NULL DEFAULT 1 REFERENCES orgs (id) ON DELETE CASCADE;
ALTER TABLE shelves   ALTER COLUMN org_id DROP DEFAULT;
ALTER TABLE books     ALTER COLUMN org_id DROP DEFAULT;
ALTER TABLE chapters  ALTER COLUMN org_id DROP DEFAULT;
ALTER TABLE pages     ALTER COLUMN org_id DROP DEFAULT;
ALTER TABLE deletions ALTER COLUMN org_id DROP DEFAULT;

CREATE INDEX shelves_org_idx   ON shelves (org_id);
CREATE INDEX books_org_idx     ON books (org_id);
CREATE INDEX chapters_org_idx  ON chapters (org_id);
CREATE INDEX pages_org_idx     ON pages (org_id);
CREATE INDEX deletions_org_idx ON deletions (org_id);

-- Slug uniqueness becomes per-org for top-level entities. Chapters/pages are
-- already scoped by book, which implies the org.
DROP INDEX shelves_slug_uq;
CREATE UNIQUE INDEX shelves_slug_uq ON shelves (org_id, slug) WHERE deleted_at IS NULL;
DROP INDEX books_slug_uq;
CREATE UNIQUE INDEX books_slug_uq ON books (org_id, slug) WHERE deleted_at IS NULL;

-- API tokens act within one org (MCP clients can't switch headers per call).
ALTER TABLE api_tokens ADD COLUMN org_id BIGINT REFERENCES orgs (id) ON DELETE CASCADE;
UPDATE api_tokens SET org_id = 1;
