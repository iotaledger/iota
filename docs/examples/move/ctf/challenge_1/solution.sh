#!/bin/bash
source .env

# Challenge 1: Simply call the get_flag function
iota client call \
  --package $CHALLENGE_1_PACKAGE \
  --module checkin \
  --function get_flag
