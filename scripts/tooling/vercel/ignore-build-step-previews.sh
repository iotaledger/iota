#!/bin/bash
# This script is meant to be run from the "Ignored Build Step" in Vercel.

cd "$(dirname "$0")"

if [ "$VERCEL_ENV" == "preview" ]; then
  bash check-excluded-branches.sh
else
  echo "❌ - Not a preview deployment."
  exit 0
fi
