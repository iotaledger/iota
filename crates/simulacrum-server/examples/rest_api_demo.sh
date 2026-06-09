#!/bin/bash

# Example script demonstrating how to use the Simulacrum Server REST API

BASE_URL="http://127.0.0.1:8080"

echo "=== Simulacrum Server REST API Demo ==="
echo ""

# Get status
echo "1. Getting simulacrum status..."
curl -s "$BASE_URL/status" | jq '.'
echo ""

# Get current checkpoint
echo "2. Getting current checkpoint..."
curl -s "$BASE_URL/checkpoint" | jq '.'
echo ""

# Create a checkpoint
echo "3. Creating a new checkpoint..."
curl -s -X POST "$BASE_URL/checkpoint/create" | jq '.'
echo ""

# Advance clock
echo "4. Advancing clock by 5 seconds..."
curl -s -X POST "$BASE_URL/clock/advance" \
  -H "Content-Type: application/json" \
  -d '{"duration_ms": 5000}' | jq '.'
echo ""

# Create multiple checkpoints
echo "5. Creating 3 checkpoints with 2 second intervals..."
curl -s -X POST "$BASE_URL/checkpoint/create_multiple" \
  -H "Content-Type: application/json" \
  -d '{"count": 3, "interval_ms": 2000}' | jq '.'
echo ""

# Advance to next epoch
echo "6. Advancing to next epoch with checkpoint..."
curl -s -X POST "$BASE_URL/epoch/advance" \
  -H "Content-Type: application/json" \
  -d '{"create_checkpoint": true}' | jq '.'
echo ""

# Test faucet health
echo "7. Checking faucet health..."
curl -s "$BASE_URL/faucet/" | jq '.'
echo ""

# Request gas from faucet (requires a valid IOTA address)
echo "8. Requesting gas from faucet..."
curl -s -X POST "$BASE_URL/faucet/gas" \
  -H "Content-Type: application/json" \
  -d '{"FixedAmountRequest": {"recipient": "0x0000000000000000000000000000000000000000000000000000000000000000"}}' | jq '.'
echo ""

# Request gas from faucet (batch endpoint)
echo "9. Requesting gas from faucet (batch)..."
curl -s -X POST "$BASE_URL/faucet/v1/gas" \
  -H "Content-Type: application/json" \
  -d '{"FixedAmountRequest": {"recipient": "0x0000000000000000000000000000000000000000000000000000000000000000"}}' | jq '.'
echo ""

# Final status check
echo "10. Final status check..."
curl -s "$BASE_URL/status" | jq '.'
echo ""

echo "=== Demo Complete ==="
