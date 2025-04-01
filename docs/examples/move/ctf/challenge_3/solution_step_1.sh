#!/bin/bash
# Step 1: Call mint_coin 3 times to create 3 coin objects of value 2

# Load environment variables
source ../.env

iota client ptb \
  --move-call "$CHALLENGE_3_PACKAGE::mintcoin::mint_coin" @"$CHALLENGE_3_TREASURY_CAP" \
  --move-call "$CHALLENGE_3_PACKAGE::mintcoin::mint_coin" @"$CHALLENGE_3_TREASURY_CAP" \
  --move-call "$CHALLENGE_3_PACKAGE::mintcoin::mint_coin" @"$CHALLENGE_3_TREASURY_CAP"
