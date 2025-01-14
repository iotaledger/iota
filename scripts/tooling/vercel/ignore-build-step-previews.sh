#!/bin/bash
# This script is meant to be run from the "Ignored Build Step" in Vercel.

cd "$(dirname "$0")"

if [ "$VERCEL_ENV" == "preview" ]; then
  bash ignore-build-step-base.sh
else
  echo "❌ - Not a preview deployment."
  exit 0
fi
