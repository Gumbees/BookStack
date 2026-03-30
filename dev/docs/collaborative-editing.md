# Collaborative Editing

Real-time collaborative editing lets multiple users work on the same page simultaneously. Changes are merged automatically using a CRDT so no edits are lost.

## Architecture

The system has two parts:

1. **Rust sidecar (`collab-server`)** ... a standalone WebSocket server that manages document state and synchronizes connected clients
2. **WASM client (`collab-wasm`)** ... compiled to WebAssembly and loaded in the browser; handles the Yrs protocol and WebSocket lifecycle

Both are built from the `collab/` directory at the repo root. The sidecar is a separate process; it does not run inside PHP.

```
Browser ──WebSocket──> collab-server (Rust)
   |                        |
collab-wasm              document store (in-memory Yrs docs)
   |                        |
BookStack editor <── onUpdate callback
```

### Document identity

Each page maps to a document ID of the form `page-{pageId}`. The PHP app issues a short-lived JWT before the editor connects; the sidecar validates that JWT and ensures the token's `page_id` matches the requested document.

### Yrs CRDT

Documents are stored as [Yrs](https://github.com/y-crdt/y-crdt) `Text` types. The sync protocol uses the standard Yjs wire format:

| Byte | Meaning |
|------|---------|
| `0x00` | Sync step 1 (state vector) |
| `0x01` | Sync step 2 (missing updates) |
| `0x02` | Update |
| `0x03` | Awareness (cursors/presence) |

On connect the server sends a step-1 message; the client replies with step-2 containing any updates the server is missing, and vice versa. After that, incremental updates flow in both directions.

### JWT authentication

The PHP app calls `GET /collab/token/{pageId}` (requires page edit permission). The response contains a `token` and a `ws_url`. The token is a HS256 JWT signed with `COLLAB_JWT_SECRET` and expires after 5 minutes. The WASM client appends the token as `?token=` in the WebSocket URL.

### Awareness

Cursor position and selection range are broadcast as JSON awareness payloads (message type `0x03`). The server fans them out to all other connected clients for the same document. The WASM `set_awareness(cursor_pos, selection_start, selection_end)` method sends updates; `on_awareness(callback)` fires when peer state changes.

## Configuration

All configuration is done through environment variables.

| Variable | Default | Description |
|----------|---------|-------------|
| `COLLAB_ENABLED` | `false` | Set to `true` to enable the feature |
| `COLLAB_SERVER_URL` | `ws://localhost:7700` | WebSocket URL the browser connects to |
| `COLLAB_JWT_SECRET` | _(empty)_ | Shared secret between PHP and the Rust sidecar. Must be a long random string. Required when enabled. |
| `COLLAB_PORT` | `7700` | Port the sidecar listens on |
| `COLLAB_MAX_CONNECTIONS` | `1000` | Maximum simultaneous WebSocket connections on the sidecar |

`COLLAB_SERVER_URL` is sent to the browser directly, so it must be a URL the end user's browser can reach. In production this is typically a public WebSocket endpoint (e.g., `wss://collab.example.com`).

`COLLAB_PORT` controls the sidecar's listen port via the environment. It is also used in the `docker-compose.yml` port mapping.

## Docker deployment

Two containers are required: the main BookStack app and the collab sidecar.

```yaml
services:
  app:
    # standard BookStack container
    environment:
      COLLAB_ENABLED: "true"
      COLLAB_SERVER_URL: "wss://collab.example.com"
      COLLAB_JWT_SECRET: "your-long-random-secret"

  collab:
    build:
      context: .
      dockerfile: docker/Dockerfile.collab
    environment:
      COLLAB_PORT: 7700
      COLLAB_JWT_SECRET: "your-long-random-secret"
    ports:
      - "7700:7700"
    restart: unless-stopped
```

Both containers **must share the same `COLLAB_JWT_SECRET`** or authentication will fail.

The sidecar stores documents in memory. Restarting it clears all in-progress sessions; clients will reconnect and re-sync from the server's state vector automatically.

### Production networking

The `collab-server` exposes a plain WebSocket server. In production, place a reverse proxy (nginx, Caddy, etc.) in front of it and terminate TLS there. The browser URL in `COLLAB_SERVER_URL` should use `wss://` in production.

Example nginx location block:

```nginx
location /collab/ {
    proxy_pass http://collab:7700/;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
}
```

## Building

### WASM client

Requires [wasm-pack](https://rustwasm.github.io/wasm-pack/):

```bash
npm run build:wasm
```

This runs `wasm-pack build --target web` in `collab/crates/collab-wasm` and outputs the compiled WASM and JS bindings to `public/dist/collab-wasm/`.

### Rust sidecar

```bash
cd collab
cargo build --release -p collab-server
# Binary: collab/target/release/collab-server
```

Or via Docker:

```bash
docker build -f docker/Dockerfile.collab .
```

The Dockerfile uses a multi-stage build: Rust 1.85 builder stage, then a `debian:bookworm-slim` runtime image with only the compiled binary. The final image exposes port 7700.

## Health check

The sidecar exposes a health endpoint:

```
GET /health
```

```json
{
  "status": "ok",
  "active_documents": 3,
  "total_connections": 7
}
```

A `GET /documents` endpoint lists active document IDs.

## Development setup

The development `docker-compose.yml` includes a `collab` service. Set `COLLAB_JWT_SECRET` in your `.env` and start the stack:

```bash
docker compose up collab
```

Set `COLLAB_ENABLED=true` and `COLLAB_SERVER_URL=ws://localhost:7700` in your `.env` for local development.
