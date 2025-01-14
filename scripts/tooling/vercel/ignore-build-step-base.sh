#!/bin/bash
# This script is meant to be run from the "Ignored Build Step" in Vercel.

cd "$(dirname "$0")"

# Check for excluded branches and exit if necessary
bash check-excluded-branches.sh
