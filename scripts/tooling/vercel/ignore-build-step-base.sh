#!/bin/bash
# This script is meant to be run from the "Ignored Build Step" in Vercel.

PACKAGE_NAME="$(pnpm pkg get name | tr -d '"')"

pnpx turbo query affected --packages "$PACKAGE_NAME" --base=HEAD^1 --exit-code
