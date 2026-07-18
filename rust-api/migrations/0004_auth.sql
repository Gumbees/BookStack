-- Settings (global + per-org overrides), external OAuth/OIDC login
-- providers, and the MCP OAuth authorization server's own storage.

-- Key/value settings. org_id NULL = global. Org values override global
-- (inheritance is resolved in code).
CREATE TABLE settings (
    org_id     BIGINT      REFERENCES orgs (id) ON DELETE CASCADE,
    key        TEXT        NOT NULL,
    value      JSONB       NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX settings_scope_key_uq ON settings (coalesce(org_id, 0), key);

-- External OAuth2/OIDC login providers ("Sign in with …").
-- org_id NULL = global provider (available to every org unless the org opts
-- out of inheritance); org-scoped rows are managed by org admins when the
-- global `auth.orgs_can_manage` checkbox allows it.
CREATE TABLE auth_providers (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    org_id        BIGINT      REFERENCES orgs (id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    client_id     TEXT        NOT NULL,
    client_secret TEXT        NOT NULL DEFAULT '',
    authorize_url TEXT        NOT NULL,
    token_url     TEXT        NOT NULL,
    userinfo_url  TEXT        NOT NULL,
    scopes        TEXT        NOT NULL DEFAULT 'openid profile email',
    enabled       BOOLEAN     NOT NULL DEFAULT true,
    auto_register BOOLEAN     NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX auth_providers_org_idx ON auth_providers (org_id);

-- MCP OAuth authorization server: dynamically registered clients,
-- short-lived authorization codes (PKCE), and rotating refresh tokens.
CREATE TABLE oauth_clients (
    client_id     TEXT        PRIMARY KEY,
    client_secret TEXT,
    name          TEXT        NOT NULL DEFAULT '',
    redirect_uris JSONB       NOT NULL DEFAULT '[]',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE oauth_codes (
    code                  TEXT        PRIMARY KEY,
    client_id             TEXT        NOT NULL REFERENCES oauth_clients (client_id) ON DELETE CASCADE,
    user_id               BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    org_id                BIGINT      REFERENCES orgs (id) ON DELETE CASCADE,
    redirect_uri          TEXT        NOT NULL,
    code_challenge        TEXT        NOT NULL,
    code_challenge_method TEXT        NOT NULL DEFAULT 'S256',
    scope                 TEXT        NOT NULL DEFAULT '',
    expires_at            TIMESTAMPTZ NOT NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE oauth_refresh_tokens (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    token_hash TEXT        NOT NULL UNIQUE,
    client_id  TEXT        NOT NULL REFERENCES oauth_clients (client_id) ON DELETE CASCADE,
    user_id    BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    org_id     BIGINT      REFERENCES orgs (id) ON DELETE CASCADE,
    scope      TEXT        NOT NULL DEFAULT '',
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
