#!/bin/bash

# Script to update the tx_digest in profiler_tests.rs with a recent transaction involving shared objects
# This script queries the IOTA testnet JSON-RPC API to find a suitable transaction

set -e

TESTNET_URL="https://api.testnet.iota.cafe"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_FILE="$SCRIPT_DIR/profiler_tests.rs"

echo "Querying recent checkpoints from $TESTNET_URL..."

# Get the latest checkpoint sequence number
LATEST_CHECKPOINT=$(curl -s -X POST "$TESTNET_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"iota_getLatestCheckpointSequenceNumber"}' \
  | jq -r '.result')

if [ -z "$LATEST_CHECKPOINT" ] || [ "$LATEST_CHECKPOINT" = "null" ]; then
  echo "Error: Could not fetch latest checkpoint"
  exit 1
fi

echo "Latest checkpoint: $LATEST_CHECKPOINT"

# Search through recent checkpoints for a transaction with shared objects
FOUND_TX=""
for ((i=0; i<20; i++)); do
  CHECKPOINT=$((LATEST_CHECKPOINT - i))
  echo "Checking checkpoint $CHECKPOINT..."
  
  # Get checkpoint data
  CHECKPOINT_DATA=$(curl -s -X POST "$TESTNET_URL" \
    -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"iota_getCheckpoint\",\"params\":[\"$CHECKPOINT\"]}")
  
  # Extract transaction digests
  TX_DIGESTS=$(echo "$CHECKPOINT_DATA" | jq -r '.result.transactions[]?' 2>/dev/null)
  
  if [ -z "$TX_DIGESTS" ]; then
    continue
  fi
  
  # Check each transaction for shared objects
  while IFS= read -r TX_DIGEST; do
    if [ -z "$TX_DIGEST" ]; then
      continue
    fi
    
    echo "  Checking transaction: $TX_DIGEST"
    
    # Get transaction details with showInput and showEffects options
    TX_DATA=$(curl -s -X POST "$TESTNET_URL" \
      -H "Content-Type: application/json" \
      -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"iota_getTransactionBlock\",\"params\":[\"$TX_DIGEST\",{\"showInput\":true,\"showEffects\":true}]}")
    
    # Check the transaction kind - we want ProgrammableTransaction
    TX_KIND=$(echo "$TX_DATA" | jq -r '.result.transaction.data.transaction.kind?' 2>/dev/null)
    echo "    Transaction kind: $TX_KIND"
    if [ "$TX_KIND" != "ProgrammableTransaction" ]; then
      echo "    Skipping non-programmable transaction (kind: $TX_KIND)"
      continue
    fi
    
    # Check if transaction only calls system functions (randomness or clock updates)
    MOVE_CALLS=$(echo "$TX_DATA" | jq -c '.result.transaction.data.transaction.ProgrammableTransaction.commands[]? | select(.MoveCall != null) | .MoveCall' 2>/dev/null)
    if [ -n "$MOVE_CALLS" ]; then
      # Check each MoveCall to see if any are to non-system packages or non-random/clock functions
      HAS_USER_TX=false
      while IFS= read -r CALL; do
        PACKAGE=$(echo "$CALL" | jq -r '.package' 2>/dev/null)
        FUNCTION=$(echo "$CALL" | jq -r '.function' 2>/dev/null)
        
        # Skip system packages (0x1, 0x2, 0x3, etc. - single digit after 0x)
        if echo "$PACKAGE" | grep -qE '^0x0*[0-9]$'; then
          # Even for system packages, skip if it's random or clock related
          if echo "$FUNCTION" | grep -qE -i 'random|clock'; then
            continue
          fi
        else
          # Non-system package found - this is a user transaction
          HAS_USER_TX=true
          break
        fi
      done <<< "$MOVE_CALLS"
      
      if [ "$HAS_USER_TX" = false ]; then
        echo "    Skipping system-only or random/clock transaction"
        continue
      fi
    fi
    
    # Check for shared objects in transaction input
    SHARED_OBJECTS=$(echo "$TX_DATA" | jq -r '.result.transaction.data.transaction.inputs[]? | select(.type == "sharedObject" or .type == "SharedObject")' 2>/dev/null)
    
    # Also check in effects for shared objects
    if [ -z "$SHARED_OBJECTS" ]; then
      SHARED_OBJECTS=$(echo "$TX_DATA" | jq -r '.result.effects.sharedObjects[]?' 2>/dev/null)
    fi
    
    if [ -n "$SHARED_OBJECTS" ]; then
      echo "    Found transaction with shared objects!"
      FOUND_TX="$TX_DIGEST"
      break 2
    fi
  done <<< "$TX_DIGESTS"
done

if [ -z "$FOUND_TX" ]; then
  echo "Error: Could not find a suitable transaction with shared objects"
  exit 1
fi

echo ""
echo "Found suitable transaction: $FOUND_TX"
echo "Updating $TEST_FILE..."

# Update the tx_digest in the test file
if [[ "$OSTYPE" == "darwin"* ]]; then
  # macOS
  sed -i '' "s/let tx_digest = \"[^\"]*\".to_string();/let tx_digest = \"$FOUND_TX\".to_string();/" "$TEST_FILE"
else
  # Linux
  sed -i "s/let tx_digest = \"[^\"]*\".to_string();/let tx_digest = \"$FOUND_TX\".to_string();/" "$TEST_FILE"
fi

echo "Successfully updated tx_digest to: $FOUND_TX"
echo ""
echo "Updated line in profiler_tests.rs:"
grep "let tx_digest = " "$TEST_FILE"
