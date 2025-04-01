#!/bin/bash
# Steps 2–5: Merge coins, split them, call get_flag, then transfer both coins

# Load environment variables
source ../.env

# 🧩 Replace these with actual coin object IDs from Step 1
COIN_1_ID=<coin-1-object-id>
COIN_2_ID=<coin-2-object-id>
COIN_3_ID=<coin-3-object-id>

# 🧩 Replace this with your address (use: iota client addresses)
CALLER_ADDRESS=<caller-address>

iota client ptb \
  --assign coin_1 @"$COIN_1_ID" \
  --assign coin_2 @"$COIN_2_ID" \
  --assign coin_3 @"$COIN_3_ID" \
  --merge-coins coin_1 [coin_2, coin_3] \
  --split-coins coin_1 [5,1] \
  --assign my_coin \
  --move-call "$CHALLENGE_3_PACKAGE::mintcoin::get_flag" @"$CHALLENGE_3_COUNTER" my_coin.0 \
  --transfer-objects [my_coin.0, my_coin.1] @"$CALLER_ADDRESS"
