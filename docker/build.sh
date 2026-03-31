#!/usr/bin/env bash
# Build both Docker images from the repo root.
#
# Usage:
#   ./docker/build.sh               # build with :latest tag
#   ./docker/build.sh 26.03-dev       # build with explicit version tag
#   ./docker/build.sh 26.03-dev push  # build and push to GHCR
#   REGISTRY=docker.io/gumbees ./docker/build.sh 26.03-dev push  # push to Docker Hub instead

set -euo pipefail

VERSION="${1:-latest}"
PUSH="${2:-}"

REGISTRY="${REGISTRY:-ghcr.io/gumbees}"
BOOKSTACK_IMAGE="${REGISTRY}/bookstack:${VERSION}"
COLLAB_IMAGE="${REGISTRY}/bookstack-collab:${VERSION}"

# Resolve the repo root (parent of this script's directory)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "${SCRIPT_DIR}")"

echo "Building from: ${REPO_ROOT}"
echo "BookStack image: ${BOOKSTACK_IMAGE}"
echo "Collab image:    ${COLLAB_IMAGE}"
echo ""

cd "${REPO_ROOT}"

echo "==> Building BookStack image..."
docker build \
    -f docker/Dockerfile \
    -t "${BOOKSTACK_IMAGE}" \
    .

echo ""
echo "==> Building collab sidecar image..."
docker build \
    -f docker/Dockerfile.collab \
    -t "${COLLAB_IMAGE}" \
    .

echo ""
echo "Build complete."
echo "  ${BOOKSTACK_IMAGE}"
echo "  ${COLLAB_IMAGE}"

if [[ "${PUSH}" == "push" ]]; then
    echo ""
    echo "==> Pushing images..."
    docker push "${BOOKSTACK_IMAGE}"
    docker push "${COLLAB_IMAGE}"
    echo "Push complete."
fi
