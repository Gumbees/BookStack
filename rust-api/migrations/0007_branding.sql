-- Branding logos. Colors/name live in the settings table (key 'branding',
-- global row org_id NULL, per-org overrides). Logos need bytes, so they get
-- their own table: org_id NULL = the instance-wide default logo.

CREATE TABLE branding_logos (
    org_id     BIGINT      REFERENCES orgs (id) ON DELETE CASCADE,
    mime       TEXT        NOT NULL,
    data       BYTEA       NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX branding_logos_scope_uq ON branding_logos (coalesce(org_id, 0));
