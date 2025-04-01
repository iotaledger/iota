#!/bin/bash
# Solution: Call get_flag with counter object and a random u64 number

# Load environment variables
source ../.env

iota client call \
  --package "$CHALLENGE_2_PACKAGE" \
  --module luckynumber \
  --function get_flag \
  --args "$CHALLENGE_2_COUNTER" 52
