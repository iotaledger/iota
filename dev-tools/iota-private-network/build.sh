#!/bin/bash
set -e

# Determine script's location to resolve the relative path correctly
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." >/dev/null && pwd -P)"

build_one() {
  local subdir="$1"
  pushd "$REPO_ROOT/docker/$subdir" >/dev/null
  ./build.sh
  popd >/dev/null
}

build_all() {
  build_one iota-node
  build_one iota-indexer
  build_one iota-tools
}

# Verify each image's git-revision label matches HEAD. BuildKit's cargo-cache
# mount can silently serve a stale binary across rebuilds even when crates/
# source changed. The official
# Docker Hub `iotaledger/iota-{node,tools,indexer}:latest` tags can also clobber
# the local tag via an implicit `docker pull` from external tooling. In both
# cases the resulting binary's git-revision won't match HEAD, so we detect and
# self-heal here rather than silently running stale code.
HEAD_REV="$(git -C "$REPO_ROOT" rev-parse --short=12 HEAD)"

verify_image() {
  local image="$1"
  local image_rev
  image_rev="$(docker image inspect "$image:latest" --format '{{ index .Config.Labels "git-revision" }}' 2>/dev/null || true)"
  # "-dirty" suffix means uncommitted local edits — accept it.
  local image_rev_clean="${image_rev%-dirty}"
  echo "  $image:latest -> git-revision=${image_rev:-(missing)}  (HEAD=$HEAD_REV)"
  [ "$image_rev_clean" = "$HEAD_REV" ]
}

verify_all() {
  local all_ok=true
  for image in iotaledger/iota-node iotaledger/iota-tools iotaledger/iota-indexer; do
    if ! verify_image "$image"; then
      all_ok=false
    fi
  done
  $all_ok
}

build_all

echo
echo "=== Verifying built images match HEAD ($HEAD_REV) ==="
if ! verify_all; then
  echo
  echo "=== Stale image(s) detected — pruning cache mounts and rebuilding once ==="
  # Only prune `--mount=type=cache` data (cargo target, cargo registry, cargo git).
  # The plain layer cache (Debian base, apt steps, etc.) is preserved so other
  # users on this shared Docker daemon don't pay a from-scratch rebuild cost when
  # our verifier fires on a tag revert. This narrow prune is sufficient because
  # the staleness path is always BuildKit's cargo cache mount serving outdated
  # compilation output — not stale Dockerfile layers.
  docker builder prune -f --filter type=exec.cachemount
  build_all
  echo
  echo "=== Re-verifying after prune+rebuild ==="
  if ! verify_all; then
    echo
    echo "ERROR: at least one image is STILL stale after prune+rebuild." >&2
    echo "       This indicates a deeper problem (e.g., :latest tag is being" >&2
    echo "       repointed by an implicit docker pull from external tooling)." >&2
    echo "       Manually retag the dangling images, or use a non-:latest tag." >&2
    exit 1
  fi
fi

echo "=== All images current at HEAD ($HEAD_REV) ==="
