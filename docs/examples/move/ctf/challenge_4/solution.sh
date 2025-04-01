#!/bin/bash
# Challenge 4 - Airdrop Coin Merge and Flag

# Load environment variables
source ../.env

# Step 1: Call airdrop to get 1 coin
iota client call \
  --package "$CHALLENGE_4_PACKAGE" \
  --module airdrop \
  --function airdrop \
  --args "$CHALLENGE_4_VAULT"

# Step 2: Generate a new address
iota client new-address ed25519 second-caller

# Step 3: Transfer coin to second-caller
# 🧩 Replace this with the actual coin object ID received in Step 1
FIRST_COIN_OBJECT=<first-coin-object-id>
iota client transfer --to second-caller --object-id "$FIRST_COIN_OBJECT"

# Step 4: Switch to second-caller
iota client switch --address second-caller

# Step 5: Get funds for gas
iota client faucet

# Step 6: Call airdrop again as second-caller
iota client call \
  --package "$CHALLENGE_4_PACKAGE" \
  --module airdrop \
  --function airdrop \
  --args "$CHALLENGE_4_VAULT"

# Step 7: Merge coins and call get_flag
# 🧩 Replace these with the actual object IDs of the coins received in steps 1 and 6
SECOND_COIN_OBJECT=<second-coin-object-id>

iota client ptb \
  --assign coin_1 @"$FIRST_COIN_OBJECT" \
  --assign coin_2 @"$SECOND_COIN_OBJECT" \
  --merge-coins coin_1 [coin_2] \
  --move-call "$CHALLENGE_4_PACKAGE::airdrop::get_flag" @"$CHALLENGE_4_COUNTER" coin_1
