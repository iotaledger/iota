#!/bin/bash

# Array of branches to be ignored
EXCLUDED_BRANCHES=("changeset-release/develop")

CURRENT_BRANCH="$VERCEL_GIT_COMMIT_REF"

if [[ ! " ${EXCLUDED_BRANCHES[@]} " =~ " ${CURRENT_BRANCH} " ]]; then
  echo "✅ - Branch is not excluded."
else
  echo "🛑 - Build cancelled for branch: $CURRENT_BRANCH"
  exit 0
fi
