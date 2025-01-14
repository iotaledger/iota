#!/bin/bash

# Array of branches to be ignored
EXCLUDED_BRANCHES=("changeset-release/develop")

CURRENT_BRANCH="$VERCEL_GIT_COMMIT_REF"

# Only check for preview deployments and not excluded branches
if [ "$VERCEL_ENV" == "preview" ] && [[ ! " ${EXCLUDED_BRANCHES[@]} " =~ " ${CURRENT_BRANCH} " ]]; then
  npx turbo-ignore
else
  exit 0
fi
