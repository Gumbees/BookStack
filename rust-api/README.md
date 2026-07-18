# BookStack-rs — Rust API + realtime collaboration + built-in MCP

A ground-up rewrite of the BookStack backend as a Rust workspace, paired with a
Solid.js frontend (in [`../frontend`](../frontend)), backed by **PostgreSQL**,
with **realtime collaborative editing** (Yjs CRDTs) and a **built-in
`bookstack-mcp` server** so AI agents can read and write the knowledge base
over the Model Context Protocol.

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
| `bookstack-core` | Domain models, PostgreSQL services (shelves/books/chapters/pages/revisions/tags/users), argon2 + JWT + API-token auth, markdown→sanitized-HTML pipeline, Postgres full-text search |
| `bookstack-collab` | Realtime engine: one Yjs (yrs) document room per actively-edited page, speaking the standard `y-websocket` binary sync + awareness protocol; debounced flatten back to Postgres and revision snapshots |
| `bookstack-mcp` | Built-in MCP server (JSON-RPC 2.0, tools capability) exposing 21 BookStack tools directly over the core services |
| `bookstack-api` | Axum binary wiring it all together: REST, WebSockets, `/mcp`, SPA static serving, graceful shutdown with collab flush |

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
| `RUST_LOG` | `info,sqlx=warn,tower_http=info` | Log filter |

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
- REST/MCP content writes **invalidate** any live room (clients auto-reconnect
  and reseed) so out-of-band edits can't silently fork the CRDT history.
- Presence (names/colors/cursors) rides the awareness protocol; departing
  clients' states are removed and rebroadcast by the server.

## Built-in MCP server (`/mcp`)

MCP Streamable HTTP transport (2025-06-18, also accepts 2025-03-26 /
2024-11-05): `POST /mcp` with JSON-RPC 2.0, plain-JSON responses, stateless
sessions, same `Authorization` header as REST. 21 tools:

`search_content`, `list_shelves`, `get_shelf`, `create_shelf`, `list_books`,
`get_book`, `create_book`, `update_book`, `delete_book`, `list_chapters`,
`get_chapter`, `create_chapter`, `list_pages`, `get_page`, `create_page`,
`update_page`, `append_to_page`, `move_page`, `delete_page`,
`list_page_revisions`, `get_system_info`.

Example client config (Claude Code):

```bash
claude mcp add --transport http bookstack http://localhost:8080/mcp \
  --header "Authorization: Token <token_id>:<secret>"
```

Write tools enforce the editor role; page-content tools coordinate with the
collab engine (see invalidation above).

## Testing done

- `cargo test` unit tests (slugs, markdown pipeline, auth hashing).
- End-to-end against Postgres 16: REST CRUD, contents tree, FTS, revisions.
- MCP: initialize/tools-list/tools-call round trips.
- Realtime: two headless `yjs` clients converging concurrent edits, awareness
  join/leave, debounce persistence, revision snapshot on room close; plus a
  two-browser Playwright session typing into the same CodeMirror editor with
  live sync both ways.

## Deliberate scope cuts (vs. upstream BookStack)

Attachments/images, comments, the recycle bin UI (soft deletes exist),
fine-grained entity permissions, LDAP/OIDC/social auth, exports, theming, and
the WYSIWYG editor (markdown-first with live collab instead). The legacy PHP
app remains untouched in the repo root during the transition.
