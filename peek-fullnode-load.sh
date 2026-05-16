#!/usr/bin/env bash
# Show per-fullnode transaction submission counts (cumulative since start).
# Useful for verifying stress-multi.sh actually distributes load across all fullnodes.
#
# Usage:
#   ./peek-fullnode-load.sh             # snapshot
#   watch -n 1 ./peek-fullnode-load.sh  # live tail

curl -sG --max-time 30 'http://localhost:9090/api/v1/query' \
  --data-urlencode 'query=sum by (host) (transaction_driver_total_transactions_submitted)' \
  | python3 -c "import json,sys; d=json.load(sys.stdin); [print(f\"  {r['metric'].get('host','?'):15s}  {float(r['value'][1]):>12.0f}\") for r in sorted(d['data']['result'], key=lambda r: r['metric'].get('host',''))]"
