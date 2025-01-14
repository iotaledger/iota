#!/bin/bash

# Check for excluded branches and exit if necessary
bash scripts/tooling/vercel/check-excluded-branches.sh

npx turbo-ignore --fallback=HEAD^1
