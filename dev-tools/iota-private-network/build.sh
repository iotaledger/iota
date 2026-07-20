#!/bin/bash
set -e

# Determine script's location to resolve the relative path correctly
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." >/dev/null && pwd -P)"

build_one() {
  local subdir="$1"
  pushd "$REPO_ROOT/docker/$subdir" >/dev/null
  ./build.sh
  # Capture rc across popd: set -e is suspended inside a function called from
  # a conditional, so a failed ./build.sh must be propagated explicitly.
  local rc=$?
  popd >/dev/null
  return $rc
}

build_all() {
  # &&-chain so a failed build short-circuits instead of running the rest and
  # returning only the last build's status.
  build_one iota-node \
    && build_one iota-indexer \
    && build_one iota-tools
}

# BuildKit's cargo cache mount can serve stale .rlib files (e.g. after a
# develop rebase removed a symbol the working tree still references), failing
# the inner `cargo build` before any image is produced. Try once against the
# existing cache; on failure prune just the cache mounts and retry once. A
# second failure is a real error — propagate it.
build_all_with_retry() {
  if build_all; then
    return 0
  fi
  echo
  echo "=== Build failed — pruning cargo cache mounts and retrying once ==="
  echo "    (likely cause: BuildKit cargo cache has stale .rlib metadata)"
  docker builder prune -f --filter type=exec.cachemount
  build_all
}

# Verify each image's git-revision label matches HEAD: a stale cargo cache
# mount, or an implicit `docker pull` retagging :latest, can leave an image
# whose binary predates HEAD. Detect and self-heal rather than run stale code.
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

build_all_with_retry

echo
echo "=== Verifying built images match HEAD ($HEAD_REV) ==="
if ! verify_all; then
  echo
  echo "=== Stale image(s) detected — pruning cache mounts and rebuilding once ==="
  # Prune only the cargo cache mounts (target/registry/git); keep the layer
  # cache so other users on this shared daemon don't pay a full rebuild.
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
