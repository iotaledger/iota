#!/bin/bash
# Challenge 5 - Crafting the perfect pizza 🍕

# Load environment variables
source ../.env

# Step 1: Run Rust code to print the expected pizza ingredients
cargo run

# Step 2: Call cook with expected pizza ingredients
iota client call \
  --package "$CHALLENGE_5_PACKAGE" \
  --module pizza \
  --function cook \
  --args 10 3 610 370 18 200 180 0

# Step 3: Call get_flag with the pizza object ID from step 2
# 🧩 Replace this with the actual object ID of the pizza created in step 2
PIZZA_OBJECT_ID=<pizza_object_id_from_step_2>

iota client call \
  --package "$CHALLENGE_5_PACKAGE" \
  --module pizza \
  --function get_flag \
  --args "$PIZZA_OBJECT_ID"
