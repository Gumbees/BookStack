# Production Dockerfile for the BookStack fork.
#
# Build from the repo root (NOT from the docker/ directory):
#   docker build -f docker/Dockerfile -t gumbees/bookstack:latest .
#
# Three-stage build:
#   1. js-builder  -- compiles TypeScript/JS/CSS via esbuild + sass
#   2. wasm-builder -- compiles collab-wasm via wasm-pack
#   3. final        -- linuxserver/bookstack base + PHP overlay + built assets
#
# Version alignment: the base image tag must match the upstream BookStack
# release this fork tracks. This fork tracks v25.12.x.

# ---------------------------------------------------------------------------
# Stage 1: JS/CSS builder
# ---------------------------------------------------------------------------
FROM node:22-alpine AS js-builder

WORKDIR /build

# Copy package files first for layer caching
COPY package.json package-lock.json ./

# Install all dependencies (including devDependencies for the build tools)
RUN npm ci

# Copy source files needed by the build
COPY resources/ ./resources/
COPY dev/build/ ./dev/build/

# Produce minified production assets into public/dist/
RUN npm run production 2>&1 | grep -v "^npm warn" || true

# ---------------------------------------------------------------------------
# Stage 2: WASM builder
# ---------------------------------------------------------------------------
FROM rust:1.85-bookworm AS wasm-builder

# Install wasm-pack
RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /build

# Copy only the collab workspace (Cargo workspace manifest + both crates)
COPY collab/Cargo.toml collab/Cargo.lock* ./
COPY collab/crates/ ./crates/

# Build collab-wasm for web target
# Output lands in /build/public/dist/collab-wasm (relative to the build root
# matching the wasm-pack out-dir in package.json)
RUN mkdir -p public/dist && \
    wasm-pack build --release --target web \
        crates/collab-wasm \
        --out-dir /build/public/dist/collab-wasm

# ---------------------------------------------------------------------------
# Stage 3: Final image
# ---------------------------------------------------------------------------
FROM lscr.io/linuxserver/bookstack:25.12.8

# Overlay our fork's PHP/view/route/migration files
COPY app/       /app/www/app/
COPY database/  /app/www/database/
COPY lang/      /app/www/lang/
COPY resources/ /app/www/resources/
COPY routes/    /app/www/routes/

# Overlay pre-built JS/CSS assets from stage 1
COPY --from=js-builder /build/public/dist/ /app/www/public/dist/

# Overlay WASM client from stage 2
COPY --from=wasm-builder /build/public/dist/collab-wasm/ /app/www/public/dist/collab-wasm/

# Ensure all overlaid files are world-readable.
# The linuxserver init process runs as abc (uid 99) but PHP-FPM worker threads
# need read access to these directories as well.
RUN chmod -R a+rX \
    /app/www/app \
    /app/www/database \
    /app/www/lang \
    /app/www/resources \
    /app/www/routes \
    /app/www/public/dist
