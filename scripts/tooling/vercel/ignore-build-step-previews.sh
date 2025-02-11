#!/bin/bash
# This script is meant to be run from the "Ignored Build Step" in Vercel.

if [ "$VERCEL_ENV" == "preview" ]; then
  echo "✅ - Preview environment detected. Checking for changes in apps..."
  # Relative to the root directory of the project. E.g relative to apps/wallet-dashboard
  bash ../../scripts/tooling/vercel/ignore-build-step-base.sh
else
  echo "❌ - Not a preview deployment."
  exit 0
fi
