#!/bin/bash
# Challenge 6 - Recycle Pizza Boxes 🍕

# Load environment variables
source ../.env

# Step 1: Create three pizza boxes with pineapple
iota client ptb \
  --move-call "$CHALLENGE_6_PACKAGE::pizza::cook" 1 1 1 1 1 1 1 1 \
  --move-call "$CHALLENGE_6_PACKAGE::pizza::cook" 1 1 1 1 1 1 1 1 \
  --move-call "$CHALLENGE_6_PACKAGE::pizza::cook" 1 1 1 1 1 1 1 1

# 🧩 After executing, set these with the returned object IDs
PIZZA_BOX_1=<pizza_box_1_id>
PIZZA_BOX_2=<pizza_box_2_id>
PIZZA_BOX_3=<pizza_box_3_id>

# Step 2: Transfer boxes to recycler and call accept_box
iota client ptb \
  --transfer-objects ["$PIZZA_BOX_1", "$PIZZA_BOX_2", "$PIZZA_BOX_3"] @"$CHALLENGE_6_RECYCLER" \
  --move-call "$CHALLENGE_6_PACKAGE::recycle::accept_box" @"$CHALLENGE_6_RECYCLER" @"$PIZZA_BOX_1" \
  --move-call "$CHALLENGE_6_PACKAGE::recycle::accept_box" @"$CHALLENGE_6_RECYCLER" @"$PIZZA_BOX_2" \
  --move-call "$CHALLENGE_6_PACKAGE::recycle::accept_box" @"$CHALLENGE_6_RECYCLER" @"$PIZZA_BOX_3"

# Step 3: Call get_flag
iota client call \
  --package "$CHALLENGE_6_PACKAGE" \
  --module recycle \
  --function get_flag \
  --args "$CHALLENGE_6_RECYCLER"
