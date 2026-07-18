# BookStack-rs — Rust API + realtime collaboration + built-in MCP

A ground-up rewrite of the BookStack backend as a Rust workspace, paired with a
Solid.js frontend (in [`../frontend`](../frontend)), backed by **PostgreSQL**,
with **realtime collaborative editing** (Yjs CRDTs), **multi-tenant
organizations**, **OAuth SSO** with global→org inheritance, a **built-in OAuth
authorization server** for MCP clients, **semantic + precision search**, and a
**built-in `bookstack-mcp` server** so AI agents can read and write the
knowledge base over the Model Context Protocol.

```
┌─────────────────────────────────────────────────────────────┐
│  bookstack-api (Axum)                                       │
│  ├── /api/…      REST API (JWT + BookStack-style API tokens)│
│  ├── /ws/pages/… y-websocket collab protocol  ──┐           │
│  ├── /mcp        MCP Streamable HTTP endpoint   │           │
│  └── /           serves the Solid.js SPA        │           │
│                                                 │           │
│  bookstack-mcp ──► bookstack-core ◄── bookstack-collab      │
│  (JSON-RPC tools)   (services, auth,   (yrs CRDT rooms,     │
│                      search, models)    sync + awareness)   │
│                          │                                  │
│                     PostgreSQL 14+                          │
│              (tsvector search, ydoc_state)                  │
└─────────────────────────────────────────────────────────────┘
```

## Crates

| Crate | Purpose |
| --- | --- |
| `bookstack-core` | Domain models, org-scoped PostgreSQL services (orgs/shelves/books/chapters/pages/revisions/tags/comments/recycle-bin/users), settings with global→org inheritance, auth providers, OAuth AS storage (PKCE codes, rotating refresh tokens), argon2 + JWT + API-token auth, markdown pipeline, Postgres full-text search |
| `bookstack-collab` | Realtime engine: one Yjs (yrs) document room per actively-edited page, speaking the standard `y-websocket` binary sync + awareness protocol; debounced flatten back to Postgres and revision snapshots |
| `bookstack-semantic` | Semantic search: heading-aware chunking, any OpenAI-compatible embeddings API, per-org in-memory vector cache backed by Postgres, background indexer driven by `pg_notify` triggers (every mutation path auto-indexes), standard + precision blend modes |
| `bookstack-mcp` | Built-in MCP server (JSON-RPC 2.0, tools capability) exposing the bees-roadhouse/bookstack-mcp surface (46 tools, +3 semantic when enabled) directly over the core services |
| `bookstack-api` | Axum binary wiring it all together: REST, WebSockets, `/mcp`, OAuth AS + SSO endpoints, SPA static serving, graceful shutdown with collab flush |

## Quick start

Requirements: Rust 1.85+, PostgreSQL 14+, Node 20+ (for the frontend).

```bash
# 1. Database
createdb bookstack   # or: CREATE DATABASE bookstack; CREATE USER … 

# 2. Frontend
cd frontend && npm install && npm run build && cd ..

# 3. API (migrations run automatically at boot; first boot seeds an admin)
cd rust-api
DATABASE_URL=postgres://bookstack:bookstack@localhost:5432/bookstack \
JWT_SECRET=change-me \
STATIC_DIR=../frontend/dist \
cargo run -p bookstack-api
```

Open http://localhost:8080 and sign in with the seeded admin
(`admin@admin.com` / `password` by default — change via env).

For frontend development with hot reload, run `npm run dev` in `frontend/`
(Vite proxies `/api`, `/ws` and `/mcp` to `:8080`).

### Environment variables

| Variable | Default | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | `postgres://bookstack:bookstack@localhost:5432/bookstack` | PostgreSQL connection string |
| `JWT_SECRET` | random per boot (dev only) | HMAC secret for session JWTs |
| `BIND_ADDR` | `0.0.0.0:8080` | Listen address |
| `ADMIN_EMAIL` / `ADMIN_PASSWORD` | `admin@admin.com` / `password` | Seeded when the users table is empty |
| `STATIC_DIR` | `frontend/dist` | Built SPA to serve at `/` |
| `PUBLIC_URL` | `http://localhost:8080` | Public base URL (OAuth issuer, SSO callbacks, MCP link building) |
| `TIMEZONE` | `UTC` | IANA zone for MCP `_meta.time` |
| `EMBEDDINGS_API_URL` | unset (semantic disabled) | Base of an OpenAI-compatible embeddings API (`POST {url}/embeddings`) |
| `EMBEDDINGS_API_KEY` | unset | Bearer key for the embeddings API |
| `EMBEDDINGS_MODEL` | `text-embedding-3-small` | Embedding model name |
| `BACKUP_ENCRYPTION_KEY` | unset (backups disabled) | Passphrase (min 12 chars) for backup/WAL encryption |
| `BACKUP_DIR` | `var/backups` | Filesystem backup target (global/sql scopes only) |
| `BACKUP_S3_*` | unset | S3-compatible object storage: ENDPOINT, BUCKET, REGION, ACCESS_KEY_ID, SECRET_ACCESS_KEY, PREFIX |
| `WALSHIP_ENABLED` / `WALSHIP_TARGET` / `WALSHIP_SPOOL_DIR` / `WALSHIP_SLOT` | disabled | Realtime WAL shipping (see below) |
| `RUST_LOG` | `info,sqlx=warn,tower_http=info` | Log filter |

## Multi-tenant organizations

Users belong to any number of **orgs**, each with a per-org role (`admin` /
`editor` / `viewer`); every content entity lives in exactly one org, and slug
uniqueness, search, the directory tree, the recycle bin, and MCP structure
instructions are all org-scoped. Existing data migrates into a seeded
`Default` org.

- The SPA picks the active org via the **header switcher** (top right) and
  sends it as `X-Org-Id`; switching orgs reloads the workspace.
- API tokens and OAuth access tokens are **org-bound**, so headless clients
  (MCP included) act in one org without extra headers.
- The top-bar search has an **"all orgs" toggle** — global keyword or
  semantic search across every org you belong to, with per-result org badges
  and automatic org switching on click.
- Org management: `POST /api/orgs` (any user; creator becomes admin),
  member add/remove with roles, instance admins act as admin everywhere.

## Sign-in providers (OAuth/OIDC SSO)

`Sign in with …` buttons come from **auth providers** stored at two scopes:

- **Global** (system admins, Admin → Global): inherited by every org.
- **Per-org** (org admins, Admin → org tab): allowed only when the global
  **"Allow organizations to add and edit their own auth servers"** checkbox
  is on (system admins can always manage any org's providers). Each org can
  also opt out of inheriting the global set (`inherit_global_auth`), and
  settings resolve org-override-else-global.

The flow is standard authorization-code: `/api/auth/oidc/{id}/start` →
provider → `/api/auth/oidc/callback` (code exchange + userinfo) → SPA lands
with a session token. Unknown users are auto-registered (per provider
setting); org-scoped providers auto-enroll the user into that org.

## Built-in OAuth authorization server (MCP login)

BookStack itself is a spec-compliant OAuth 2.1 authorization server, so MCP
clients (Claude, etc.) can connect to `/mcp` with **no pre-shared secrets**:

1. Client gets a 401 from `/mcp` with `WWW-Authenticate: Bearer
   resource_metadata=…` (RFC 9728) and discovers the AS via RFC 8414
   metadata.
2. Dynamic client registration (`POST /oauth/register`).
3. `GET /oauth/authorize` validates and hands off to the SPA consent page —
   the user signs in with **any configured method** (password or any SSO
   provider), picks the org the connection may act in, and approves.
4. `POST /oauth/token` — authorization_code with mandatory PKCE (S256),
   then rotating `refresh_token` grants (reuse of a rotated token is
   rejected).

Access tokens are org-bound BookStack JWTs (1h) accepted by `/mcp` and the
REST API alike.

## Semantic + precision search

With an embeddings provider configured, the top bar (and MCP) gain semantic
modes alongside keyword FTS:

- **standard** — broad semantic sweep blended with keyword signals (0.8/0.2).
- **precision** — tighter blend with an agreement boost when embedding
  similarity and keyword search concur, keeping only confident results
  (approximation of upstream bookstack-mcp's precision cascade; no
  cross-encoder rerank).

Indexing is automatic: Postgres triggers `pg_notify` on every
shelf/book/chapter/page change (any path — REST, MCP, collab persistence),
and the debounced background worker chunks, embeds, and upserts. Vectors are
cached in memory per org for ~ms-latency search (pgvector is the scale-up
path). Run `reembed` once per org to backfill existing content;
`embedding_status` reports progress. MCP registers `semantic_search`,
`reembed`, and `embedding_status` only when the provider is configured —
46 tools without, 49 with.

## Authentication

Two schemes, valid on **REST and `/mcp`** alike:

- `Authorization: Bearer <jwt>` — from `POST /api/auth/login {email, password}`.
- `Authorization: Token <token_id>:<secret>` — BookStack-compatible API
  tokens, minted at `POST /api/auth/tokens {name}` (secret shown once).

Roles: `admin` (user management), `editor` (write content), `viewer`
(read-only). This intentionally replaces BookStack's fine-grained
joint-permission engine with coarse roles for now.

## REST API (summary)

All under `/api`, JSON in/out, list endpoints take `count`, `offset`, `sort`,
`order`:

- `POST /auth/login` · `GET /auth/me` · `GET|POST /auth/tokens` · `DELETE /auth/tokens/{id}`
- `GET|POST /shelves` · `GET|PUT|DELETE /shelves/{id}` · `GET /shelves/slug/{slug}`
- `GET|POST /books` · `GET|PUT|DELETE /books/{id}` · `GET /books/{id}/contents` · `GET /books/slug/{slug}`
- `GET|POST /chapters` (`?book_id=`) · `GET|PUT|DELETE /chapters/{id}`
- `GET|POST /pages` (`?book_id=&chapter_id=`) · `GET|PUT|DELETE /pages/{id}`
- `PUT /pages/{id}/move` · `GET /pages/{id}/revisions` · `POST /pages/{id}/revisions/{n}/restore`
- `GET /pages/by-slugs/{book_slug}/{page_slug}`
- `POST /pages/{id}/collab/save` · `GET /pages/{id}/collab/editors`
- `GET /search?query=…&types=page,book,chapter,shelf`
- `GET|POST /users` · `GET|PUT|DELETE /users/{id}` (admin)
- `GET /system`

Search uses native Postgres full-text search (`websearch_to_tsquery` over
weighted generated `tsvector` columns, GIN-indexed) with `ts_headline`
previews that are HTML-escaped server-side and highlighted with `<mark>`.

## Realtime collaboration

- Endpoint: `ws://…/ws/pages/{page_id}?token=<jwt-or-id:secret>` (editor role
  required).
- Protocol: standard **y-websocket** binary frames (y-sync step1/step2/update
  + awareness), so stock `yjs` + `y-websocket` + `y-codemirror.next` clients
  work unmodified. The shared text root is `doc.getText("content")`.
- Rooms are seeded from the page's persisted CRDT state (`pages.ydoc_state`)
  or, when absent, from its markdown. The server applies every update to its
  authoritative doc, rebroadcasts to the room, and every ~4s (plus on last
  disconnect) flattens the doc back to `markdown`/`html` in Postgres. The last
  editor leaving records a `Collaborative editing session` revision.
- REST/MCP content writes **invalidate** any live room: the server closes
  editor sockets with WebSocket code **4409** (`content-replaced`) and
  refuses joins for a short window so a stale client's auto-reconnect can't
  merge old CRDT state back in (which would duplicate content). The SPA
  editor reacts to 4409 by discarding its local Y.Doc and rebuilding fresh.
- Presence (names/colors/cursors) rides the awareness protocol; departing
  clients' states are removed and rebroadcast by the server.

## Built-in MCP server (`/mcp`)

MCP Streamable HTTP transport (2025-06-18, also accepts 2025-03-26 /
2024-11-05): `POST /mcp` with JSON-RPC 2.0, plain-JSON responses, stateless
sessions, same `Authorization` header as REST.

The tool surface is ported from
[`bees-roadhouse/bookstack-mcp`](https://github.com/bees-roadhouse/bookstack-mcp)
(v0.13.0) — same tool names, argument conventions (`page_id`, `book_id`, …),
input schemas, response formats, and behaviors — so clients configured for
that server work against this endpoint for every feature this backend
supports. Ported conventions include:

- **Surgical editing suite**: `edit_page` (exact-string replace with
  ambiguity detection), `replace_section` (heading-scoped, level-aware),
  `insert_after` (line-anchored), `append_to_page` — same algorithms.
- **Required meaningful descriptions** on `create_shelf` / `create_book` /
  `create_chapter`, with length and placeholder rejection ("TODO", "n/a", …).
- **Duplicate-title stripping**: a leading `# <page name>` H1 is removed
  from create/update content.
- **Live structure tree** (shelves → books → chapters with IDs and truncated
  descriptions) embedded in `initialize` instructions, plus style/placement
  guidance and URL patterns for clickable links.
- **Slim text success responses** for mutations (`Page created successfully.
  / Page ID: … / URL: …`), pretty-JSON for reads, `Error:` text with
  `isError` for failures, and a `_meta.time` block
  (`now_unix`/`now_utc`/`now_local`/`now_human`/`timezone`) on every
  `tools/call` response (`TIMEZONE` env).
- `directory` (scoped, depth-limited tree), exports
  (markdown/plaintext/html), moves (`move_page`, `move_chapter`,
  `move_book_to_shelf`), comments, the recycle bin
  (list/restore/destroy), users, roles, and search operators
  (`{type:page}`, `[tag=value]`, `{in_name:term}`, `{created_by:me}`).

46 tools total. Not ported (features this backend doesn't have yet, so the
tools are simply not registered — mirroring upstream's conditional
registration): attachments, image gallery/staging uploads, per-content
permissions, audit log, and the semantic-search/embedding suite
(`semantic_search`, `reembed`, `embedding_status`, rerank). pgvector is the
natural path for semantic search here, since the index store is already
Postgres.

Example client config (Claude Code):

```bash
claude mcp add --transport http bookstack http://localhost:8080/mcp \
  --header "Authorization: Token <token_id>:<secret>"
```

Write tools enforce the editor role; page-content tools coordinate with the
collab engine (see invalidation above).

## Import from an existing BookStack instance

Admin → Data → **Import**: point at a live BookStack URL with an API token
(`id` + `secret`) and a target org. The importer pulls books → chapters →
pages → shelves (with tags and shelf-book links), fetching page content
through BookStack's markdown export endpoint so WYSIWYG pages convert too,
and stripping the export's duplicated H1 title.

**API limits are respected**: requests are paced client-side to a
configurable budget (default 90/min; BookStack ships 180/min per user), and
`429 Too Many Requests` responses are honored via `Retry-After` with bounded
retries. Progress (per-phase counters, skipped drafts, error samples)
streams into the job record and the admin UI. Images, attachments and drafts
are not imported.

## Backups (always encrypted)

Admin → Data → **Backups**. Every backup is compressed and encrypted with
XChaCha20-Poly1305 (key derived from `BACKUP_ENCRYPTION_KEY` via argon2id;
`BSBK1` header + salt + nonce). Three scopes:

| Scope | Contents | Object storage | Filesystem |
| --- | --- | --- | --- |
| `org` | one org's content (org admins) | ✓ | ✗ — restricted to global |
| `global` | whole instance incl. users/settings/providers | ✓ | ✓ |
| `sql` | `pg_dump` logical snapshot | ✓ | ✓ |

Object storage is any S3-compatible endpoint (`BACKUP_S3_ENDPOINT`,
`BACKUP_S3_BUCKET`, `BACKUP_S3_REGION`, `BACKUP_S3_ACCESS_KEY_ID`,
`BACKUP_S3_SECRET_ACCESS_KEY`, optional `BACKUP_S3_PREFIX`); the filesystem
target writes under `BACKUP_DIR`. Each stored backup has a **verify** action
that downloads, decrypts and parses it, reporting entity counts — proof the
artifact is restorable with the current key.

## Realtime SQL (WAL) shipping

`WALSHIP_ENABLED=true` starts a managed `pg_receivewal` replication stream
(slot `bookstack_walship` by default): every database change streams into a
spool, and each completed WAL segment is immediately gzip+encrypted and
shipped to `WALSHIP_TARGET` (`object_storage` or `filesystem`), then removed
from the spool. Status (running flag, shipped segment count, last error) is
at Admin → Data and `GET /api/admin/walship`.

Requirements: `pg_receivewal` on PATH and the `DATABASE_URL` role granted
`REPLICATION` (plus a replication entry in `pg_hba.conf` — Debian/Ubuntu
defaults already allow localhost). Pair the WAL stream with periodic `sql`
backups as restore baselines; segments are 16 MB, so a
`pg_switch_wal()`/`archive_timeout` cadence bounds worst-case data loss.

## Testing done

- `cargo test` unit tests (slugs, markdown pipeline, auth hashing, search
  operator parsing, section-replace bounds, title stripping, description
  validation, tool-surface count lock).
- End-to-end against Postgres 16: REST CRUD, contents tree, FTS, revisions.
- MCP: full parity sweep — tools/list count, structure-tree instructions,
  `_meta.time`, description rejection, title stripping, `edit_page`
  ambiguity errors, `replace_section`/`insert_after`/`append_to_page`
  round-trips, exports, `directory` scoping, comments (threads + updates),
  operator search (`{type:}` `{in_name:}` `[tag=value]`), all three move
  tools, recycle-bin delete→restore→destroy, role listing, and viewer-role
  write rejection.
- Realtime: two headless `yjs` clients converging concurrent edits, awareness
  join/leave, debounce persistence, revision snapshot on room close; a
  two-browser Playwright session typing into the same CodeMirror editor with
  live sync both ways; and the invalidation flow (MCP edit → live client
  closed with 4409 → stale reconnect rejected → rebuilt client sees exactly
  the new content with no CRDT duplication).

## Deliberate scope cuts (vs. upstream BookStack)

Attachments/images, fine-grained entity permissions, LDAP/OIDC/social auth,
audit log, theming, semantic search embeddings, and the WYSIWYG editor
(markdown-first with live collab instead). Comments and the recycle bin are
available via MCP; frontend UI for them is future work. The legacy PHP app
remains untouched in the repo root during the transition.
