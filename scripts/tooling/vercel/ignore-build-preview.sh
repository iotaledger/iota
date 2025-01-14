#!/bin/bash
# This script is meant to be run from the "Ignored Build Step" in Vercel.

if [ ! "$VERCEL_ENV" == "preview" ]; then
  exit 0
else
  echo "✅ - Building for preview deployment."
  bash scripts/tooling/vercel/ignore-build-command.sh
fi
