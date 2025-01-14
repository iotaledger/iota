#!/bin/bash
# This script is meant to be run from the "Ignored Build Step" in Vercel.

EXCLUDED_BRANCHES=("changeset-release/develop")

CURRENT_BRANCH="$VERCEL_GIT_COMMIT_REF"

if [[ ! " ${EXCLUDED_BRANCHES[@]} " =~ " ${CURRENT_BRANCH} " ]]; then
  echo "✅ - Branch is not excluded."
  npx turbo-ignore --fallback=HEAD^1
else
  echo "🛑 - Build cancelled for branch: $CURRENT_BRANCH"
  exit 0
fi
