#!/bin/bash
# This script is meant to be run from the "Ignored Build Step" in Vercel.

echo "✅ - Building for all environments."
bash scripts/tooling/vercel/ignore-build-command.sh
