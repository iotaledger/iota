#!/usr/bin/env bash
# Quick peek at current num_inflight_transactions per validator via Prometheus.
# Usage:
#   ./peek-inflight.sh            # instantaneous snapshot
#   watch -n 1 ./peek-inflight.sh # live tail, refreshes every 1s

curl -sG --max-time 30 'http://localhost:9090/api/v1/query' \
  --data-urlencode 'query=sum by (host) (sequencing_certificate_inflight{host=~"validator-.*"})' \
  | python3 -c "import json,sys; d=json.load(sys.stdin); [print(f\"  {r['metric'].get('host','?'):15s}  {float(r['value'][1]):>10.0f}\") for r in sorted(d['data']['result'], key=lambda r: r['metric'].get('host',''))]"
