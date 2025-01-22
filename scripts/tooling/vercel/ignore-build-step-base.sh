#!/bin/bash
# This script is meant to be run from the "Ignored Build Step" in Vercel.

npx turbo-ignore --fallback=HEAD^1
