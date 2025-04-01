#!/bin/bash
# Challenge 7 - Craft dough from ingredients and claim the flag 🥖

# Load environment variables
source ../.env

iota client ptb \
  --move-call "$CHALLENGE_7_PACKAGE::ptb::get_ingredients" --assign ingredients \
  --move-call "$CHALLENGE_7_PACKAGE::ptb::make_dough" ingredients.0 ingredients.1 ingredients.2 ingredients.3 \
  --assign dough \
  --move-call "$CHALLENGE_7_PACKAGE::ptb::get_flag" dough
