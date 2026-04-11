#!/usr/bin/env bash
# Build the BookStack Docker image from the repo root.
#
# Usage:
#   ./docker/build.sh               # build with :latest tag
#   ./docker/build.sh 26.04-dev       # build with explicit version tag
#   ./docker/build.sh 26.04-dev push  # build and push to GHCR
#   REGISTRY=docker.io/bees-roadhouse ./docker/build.sh 26.04-dev push  # push to Docker Hub instead

set -euo pipefail

VERSION="${1:-latest}"
PUSH="${2:-}"

REGISTRY="${REGISTRY:-ghcr.io/bees-roadhouse}"
BOOKSTACK_IMAGE="${REGISTRY}/bookstack:${VERSION}"

# Resolve the repo root (parent of this script's directory)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "${SCRIPT_DIR}")"

echo "Building from: ${REPO_ROOT}"
echo "BookStack image: ${BOOKSTACK_IMAGE}"
echo ""

cd "${REPO_ROOT}"

echo "==> Building BookStack image..."
docker build \
    -f docker/Dockerfile \
    -t "${BOOKSTACK_IMAGE}" \
    .

echo ""
echo "Build complete."
echo "  ${BOOKSTACK_IMAGE}"

if [[ "${PUSH}" == "push" ]]; then
    echo ""
    echo "==> Pushing image..."
    docker push "${BOOKSTACK_IMAGE}"
    echo "Push complete."
fi
