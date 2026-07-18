-- Operational features: BookStack-instance imports and encrypted backups.

CREATE TABLE import_jobs (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    org_id     BIGINT      NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    source_url TEXT        NOT NULL,
    status     TEXT        NOT NULL DEFAULT 'running'
               CHECK (status IN ('running', 'completed', 'failed')),
    -- Live counters + error samples, updated as the job progresses.
    progress   JSONB       NOT NULL DEFAULT '{}',
    created_by BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE backups (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- 'global' = whole instance JSON, 'org' = one org JSON, 'sql' = pg_dump.
    scope      TEXT        NOT NULL CHECK (scope IN ('global', 'org', 'sql')),
    org_id     BIGINT      REFERENCES orgs (id) ON DELETE CASCADE,
    target     TEXT        NOT NULL CHECK (target IN ('object_storage', 'filesystem')),
    location   TEXT        NOT NULL,
    size_bytes BIGINT      NOT NULL DEFAULT 0,
    encrypted  BOOLEAN     NOT NULL DEFAULT true,
    status     TEXT        NOT NULL DEFAULT 'running'
               CHECK (status IN ('running', 'completed', 'failed')),
    error      TEXT,
    created_by BIGINT      REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
