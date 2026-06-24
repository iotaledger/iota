# Double-Spend Workload Runbook

Run the double-spend stress workload against a local IOTA private network and
reconcile the logs with `log-audit` to prove no double-spend leaked. The
workload submits pairs of conflicting txs that spend the same gas coin;
white-flag must accept exactly one of each pair.

**Two repos:** `iota` (node + private network + audit) and `network-benchmark`
(the `stress` binary with the double-spend workload — it is _not_ in
`iota/crates/iota-benchmark`).

Substitute your own checkout locations for `<path-to-iota-repo>` and
`<path-to-network-benchmark-repo>` below. The private network lives at
`<path-to-iota-repo>/dev-tools/iota-private-network`.

## 1. Build the node image (current branch)

```bash
cd <path-to-iota-repo>
./docker/iota-node/build.sh  -t iota-node  --no-cache
./docker/iota-tools/build.sh -t iota-tools --no-cache
```

Builds from your working tree, so the checked-out branch is what runs.

## 2. Build the stress image

From `network-benchmark`, with the double-spend branch checked out:

```bash
cd <path-to-network-benchmark-repo>
./docker/stress/build.sh          # tags iotaledger/stress
docker run --rm iotaledger/stress /usr/local/bin/stress bench --help | grep double-spend
```

## 3. Bootstrap + start the network (benchmark mode)

`-b` adds the deterministic benchmark gas accounts to genesis and writes
`benchmark.keystore`.

```bash
cd <path-to-iota-repo>/dev-tools/iota-private-network
sudo ./bootstrap.sh -b   # default 4 validators
# Enable the white-flag (P-COOL post-consensus owned-object locking) flow:
export IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1
export IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_PCOOL_FLOW=true
./run.sh faucet          # validators + fullnode-1 on http://127.0.0.1:9000
```

The default `RUST_LOG` (`info,iota_core=debug,...`) already emits every event
the audit needs (conflict/winner lines from `iota_core::post_consensus_validation`).

### Optional: local Grafana

Bring up the bundled Grafana + Prometheus stack to watch the run live:

```bash
cd <path-to-iota-repo>/dev-tools/grafana-local
docker compose up -d
```

Dashboards at <http://localhost:3000/dashboards>.

## 4. Run the workload

On the private network's Docker network so it reaches `fullnode-1` by hostname:

```bash
export PRIVNET=<path-to-iota-repo>/dev-tools/iota-private-network
docker run -d --name stress-benchmark \
  --network iota-private-network_iota-network \
  -v "$PRIVNET/configs/genesis/genesis.blob:/opt/iota/config/genesis.blob:ro" \
  -v "$PRIVNET/configs/genesis/benchmark.keystore:/opt/iota/config/iota.keystore:ro" \
  iotaledger/stress /usr/local/bin/stress \
    --local false \
    --fullnode-rpc-addresses http://fullnode-1:9000 \
    --use-fullnode-for-execution true \
    --use-fullnode-for-reconfig true \
    --genesis-blob-path /opt/iota/config/genesis.blob \
    --keystore-path /opt/iota/config/iota.keystore \
    --primary-gas-owner-id 0xf479d29837d22943aba6afc401f518a36521b990874eca784886185bd26bf681 \
    --num-client-threads 4 --num-transfer-accounts 10 --run-duration 1800s \
    --client-metric-host 0.0.0.0 --client-metric-port 8081 \
    bench --target-qps 500 --in-flight-ratio 5 --num-workers 12 \
    --transfer-object 0 --shared-counter 0 --double-spend 100 \
    --double-spend-num-pairs 16 --double-spend-overlap-factor 2
```

Key flags: `--double-spend 100` (100% double-spend mix), `--double-spend-num-pairs`
(distinct contested coins), `--double-spend-overlap-factor` (oversamples so both
halves of a pair are more likely to land in the same commit). Use a shorter
`--run-duration 120s` for a smoke test. Follow with `docker logs -f stress-benchmark`.

## 5. Collect logs

> **When to grab logs:** once Grafana shows a fork (e.g. diverging checkpoint /
> round across validators) or a hang (progress flatlines), stop the containers
> first (`docker stop validator-* fullnode-1 stress-benchmark`) and then extract
> the logs below — this freezes the state at the incident.

The audit auto-discovers `validator-*.log` (required), `stress*.log`, and
`fullnode-*.log` (opt-in) in one directory:

```bash
mkdir -p /tmp/ds-logs && cd /tmp/ds-logs
for i in 1 2 3 4; do docker logs validator-$i > validator-$i.log 2>&1; done
docker logs stress-benchmark > stress-benchmark.log 2>&1
docker logs fullnode-1 > fullnode-1.log 2>&1     # optional, large
```

## 6. Run the audit

```bash
cd <path-to-iota-repo>/dev-tools/iota-private-network/scripts/log-audit
python3 audit.py /tmp/ds-logs --include-fullnode --json /tmp/ds-audit.json
```

Exit codes: `0` = PASS (no double-spend leaked), `1` = FAIL (safety violation),
`2` = INCONCLUSIVE (coverage check `[0]` failed — the parser matched none of a
signal that must be present, usually because the node log format drifted from
the parser regexes; nothing was verified, so treat it as not-yet-audited rather
than safe). Checks: parser coverage, single winner per contested input,
cross-validator agreement, losers never executed, dropped counts reconcile,
double-spend pair tracking (and fullnode consistency with `--include-fullnode`).
`OVERALL: PASS` means no double-spend leaked; on `FAIL` the per-check detail and
JSON list the offending digests/object refs.

## Cleanup

```bash
docker rm -f stress-benchmark
cd <path-to-iota-repo>/dev-tools/grafana-local && docker compose down
cd <path-to-iota-repo>/dev-tools/iota-private-network && sudo ./cleanup.sh
```
