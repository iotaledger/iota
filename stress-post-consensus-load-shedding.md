# Stress-test post-consensus load shedding

## What we're testing

The branch `protocol-research/feat/post-consensus-load-shedding` adds a quorum-driven, post-consensus shedding mechanism. Each validator monitors local execution congestion through three signals — execution-queue latency, transaction-manager inflight queue depth, and writeback-cache backpressure — and broadcasts its current load-shedding percentage to the network as an `OverloadNotificationV1` consensus transaction. After every commit each validator computes a stake-weighted 2f+1 percentile of received notifications and deterministically drops that fraction of *user-originating* transactions before scheduling them for execution. Only user transactions are eligible: system messages (checkpoint signatures, capability notifications, randomness DKG, the overload notifications themselves) are explicitly skipped at [authority_per_epoch_store.rs:3157](crates/iota-core/src/authority/authority_per_epoch_store.rs#L3157).

Unit and integration tests cover the math, the user-only filter, and the temporal-seed determinism. End-to-end question: against a real n-validator private network with artificially slowed execution, does the mechanism (a) propagate overload signals reliably, (b) converge on a quorum percentage that matches the slowed subset's stake, (c) actually shed at that rate post-consensus, and (d) recover when the slowdown is removed?

## Running an experiment

[`run-pcool-experiment.sh`](run-pcool-experiment.sh) at the repo root is the only entry point. It rebuilds the iota-node image, tears down + rebootstraps the private network, applies any per-validator config overrides, brings everything up, verifies the running binary actually has the new fields, opens the Grafana dashboard, and runs the stress workload — in that order, every time.

### Quickstart

```bash
./run-pcool-experiment.sh
# defaults: QPS=2000, DURATION=120s, EXECUTION_DELAY_MS=100
```

Iterating without paying for a full image rebuild (the staleness check still runs and will force a rebuild if needed):

```bash
./run-pcool-experiment.sh --no-rebuild
```

### Canonical run shapes

#### 1. Baseline (no execution delay)

```bash
EXECUTION_DELAY_MS=0 QPS=2000 DURATION=60s ./run-pcool-experiment.sh
```

Expected: zero overload notifications, zero drops, useful TPS ≈ QPS. Sanity check that the mechanism doesn't fire spuriously when execution is keeping up.

#### 2. Symmetric overload — every validator slowed equally

```bash
EXECUTION_DELAY_MS=100 QPS=5000 DURATION=120s ./run-pcool-experiment.sh
```

Expected: every validator's local % rises within ~`overload_monitor_interval`; received-from-peer counts grow at ~`1/overload_monitor_interval` per peer; quorum % converges across validators within one commit; drop rate grows proportionally. After stress stops, all gauges return to 0 within ~30s (one temporal-seed window) and useful TPS recovers.

#### 3. Asymmetric overload — one validator slowed (below quorum threshold)

```bash
EXECUTION_DELAY_MS_PER_VALIDATOR=0,0,0,200 QPS=2000 DURATION=120s ./run-pcool-experiment.sh
```

With 4 validators of equal stake, validator-4 alone holds 25% of stake — below the 2f+1 (67%) percentile. Expected: validator-4's local % rises, the other three observe it as a peer notification, but the stake-weighted percentile stays at 0 → no drops → useful TPS stays at baseline. Tests that one slow validator does *not* cause network-wide shedding.

#### 4. Asymmetric overload — half the committee slowed (at quorum threshold)

```bash
EXECUTION_DELAY_MS_PER_VALIDATOR=0,0,200,200 QPS=2000 DURATION=120s ./run-pcool-experiment.sh
```

2-of-4 = 50% stake exceeds the f+1 line. Expected: the quorum percentile reflects the slowed pair's advertised value; useful TPS drops on *all 4* validators (post-consensus drops apply globally once the quorum says so). Tests the aggregation logic directly.

### Flags

| Flag | Effect |
|---|---|
| `--no-rebuild` | Skip the iota-node docker image rebuild. Staleness check still enforced. |
| `--no-browser` | Don't open the Grafana dashboard URL after bring-up. |
| `--num-validators N` | Committee size (default 4). N > 4 requires `configs/genesis-template-N.yaml`. |
| `-- <args>` | Forward `<args>` to stress.rs (e.g., `-- --benchmark-stats-path /tmp/x.json`). |

### Environment variables

| Variable | Default | Effect |
|---|---|---|
| `QPS` | `2000` | Target offered load |
| `DURATION` | `120s` | Stress duration |
| `WORKERS` | `12` | stress.rs worker count |
| `IN_FLIGHT_RATIO` | `5` | Per-account outstanding-tx cap |
| `NUM_TRANSFER_ACCOUNTS` | `2` | Min that works without panicking |
| `NUM_CLIENT_THREADS` | `4` | stress.rs client threads |
| `TRANSFER_OBJECT_PCT` / `SHARED_COUNTER_PCT` | `100` / `0` | Workload mix |
| `EXECUTION_DELAY_MS` | `100` | Symmetric per-tx execution delay (ms) |
| `EXECUTION_DELAY_MS_PER_VALIDATOR` | (unset) | Comma list, e.g. `0,0,0,200` — overrides the symmetric default |
| `FULLNODE_RPC` | `http://127.0.0.1:9000` | Comma list for multi-fullnode |
| `PROM_URL` | `http://127.0.0.1:9090` | Prometheus base URL |
| `RUNS_DIR` | `$ROOT/runs` | Where artifacts land |

### Reading the results

While the run is in flight, the dashboard auto-opens at:

```
http://localhost:3000/d/pcool-load-shedding-stress/post-consensus-load-shedding-stress-test?refresh=5s&from=now-15m&to=now
```

The top row — *Arrival TPS* (left) and *Useful TPS* (right) — is the first thing to look at: if arrivals are flat at zero, the run isn't producing traffic that reaches consensus (check `runs/<ts>/stress-stdout.log` for client-side errors). The next rows show how the mechanism is responding: local %, source breakdown, sent/received notification rates, the quorum percentage, and the post-consensus drop rate. For the asymmetric runs the single most useful signal is the **Quorum load-shedding %** panel — it should be 0 in the "below quorum" run and equal-across-hosts in the "at quorum" run, no matter how high the slowed validators' individual percentages get.

After the run, artifacts land in `runs/<utc-ts>/`:

| File | Contents |
|---|---|
| `params.txt` | The env vars, start/end epochs, exit code |
| `deployed-config.txt` | `authority-overload-config` snapshot from each `validator-N-8080.yaml` as actually deployed |
| `summary.txt` | Per-host peak/avg/final for local %, quorum %, useful TPS, sent rate, drop rate, queueing p99; total counters; per-source breakdown |
| `{local_pct, source_pct, sent_rate, received_rate, last_recv_pct, quorum_pct, dropped_rate, useful_tps, queue_p99}.json` | Raw Prometheus `query_range` JSON |
| `{sent_total, dropped_total}.json` | `increase()` totals over the run window |
| `stress-stdout.log` | Full stress.rs output (render with `less -R`) |
| `build.log` | Cargo build output (when stress.rs needs rebuilding from source) |

End state to confirm: stop stress, wait ~30s, then verify all `authority_*` gauges return to 0, the network keeps producing checkpoints, and useful TPS climbs back to baseline. If system messages were being dropped, checkpoint signing and randomness DKG would stall — the network's continued progress implicitly exercises the user-only drop filter.

`runs/` is gitignored.

---

## How `run-pcool-experiment.sh` works

The script performs ten ordered steps on every invocation:

1. **Rebuild the iota-node docker image** (skipped with `--no-rebuild`).
2. **Tear down** the existing private network with `docker compose down -v`.
3. **Wipe** `dev-tools/iota-private-network/data/` (host bind-mounted state).
4. **Bootstrap** the network — regenerates per-validator YAMLs from `validator-common.yaml`, generates a deterministic benchmark keystore.
5. **Apply per-validator YAML overrides** if `EXECUTION_DELAY_MS_PER_VALIDATOR` is set — rewrites the `execution-delay-ms` field on each `validator-N-8080.yaml` before containers start.
6. **Bring up** validators + faucet with white-flag flow enabled (via `IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE` + `IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW`).
7. **Bring up** the local Grafana + Prometheus stack.
8. **Wait** for the fullnode JSON-RPC, Grafana, and Prometheus to be reachable.
9. **Verify** the running iota-node binary actually contains `execution_delay_ms`, `overload_signal_*`, and `post_consensus_load_shedding_dropped_transactions_total`. If any symbol is missing, the image was built from a tree that predated those fields — the script forces a rebuild and bounces the network even when `--no-rebuild` was passed.
10. **Open** the dashboard URL and run the stress workload via `stress-pcool-load-shedding.sh`.

Each step exists because of a specific failure mode that *used* to silently break the experiment:

- **Image rebuild** — after any change to `crates/iota-core/`, `crates/iota-config/`, or `crates/iota-node/`, the docker image must be rebuilt or the running validator continues to use whatever code shipped in the previous image. New YAML fields on a struct the binary doesn't have are silently ignored (`#[serde]` default is to skip unknown keys), so a missing field produces no error — just no effect at runtime.

- **`rm -rf data/`** — each validator and fullnode container mounts `./data/{validator,fullnode}-N` as a host bind mount, not a Docker volume, so `docker compose down -v` doesn't remove them. Without the wipe, the next container start replays the previous run's checkpoints; the transaction manager fills with stale work, and new stress traffic times out behind it.

- **Per-validator YAML overrides applied between bootstrap and bring-up** — validators read config at startup only. Writing the asymmetric delay into the YAML *after* the container starts requires a separate restart, which is fragile; doing it before bring-up means containers boot with the right config on first start.

- **Binary-staleness check after bring-up** — this is the belt-and-suspenders catch for the failure mode where the YAML legitimately has the field but the binary doesn't deserialize it. The symptom is "I set a 200ms delay and saw no effect"; the cause is a stale image; the script greps the running binary's symbol table to detect it.

### macOS vs Linux

Bootstrap needs `sudo` only on Linux (for `chown 999:999` on the postgres data dir). On macOS Docker Desktop handles user-mapping internally — the script detects the OS and does the right thing automatically. **Never run anything in this directory with `sudo` on macOS**: it leaves files owned by `root`, which the next non-sudo `rm -rf data/` silently fails to delete. If that ever happens, fix the ownership once with `sudo rm -rf dev-tools/iota-private-network/data` and never use sudo on macOS again.

### Linux network tuning (high QPS only)

At QPS ≥ 5k with multiple fullnodes, default ephemeral port range and TIME_WAIT behavior can cause `EADDRNOTAVAIL` mid-run. Apply if you hit it:

```bash
sysctl -w net.ipv4.ip_local_port_range="1024 65535"
sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.ipv4.tcp_fin_timeout=15
```

---

## How the post-consensus shedding mechanism works

### Why we need an artificial execution slowdown

Pre-consensus shedding (the white-flag PR) triggers when the consensus *submission* queue fills up — pure spam at high QPS gets you there. Post-consensus shedding triggers on *execution* signals, and on a 4-core laptop those signals only fire once execution can't keep up with consensus output. Just cranking QPS doesn't do it: consensus is the bottleneck long before execution gets behind.

To force the issue, a configurable per-validator `tokio::time::sleep` is inserted right before `try_execute_immediately` in [execution_driver.rs:120](crates/iota-core/src/execution_driver.rs#L120), wired through a new `AuthorityOverloadConfig::execution_delay_ms` field. With this knob set, modest spam (1–5k QPS) reliably builds the queueing latency, transaction-manager queue depth, and cache backpressure that the overload monitor reads.

Companion change: the execution semaphore in `execution_process()` was reduced from `num_cpus::get()` to `1` ([execution_driver.rs:34](crates/iota-core/src/execution_driver.rs#L34)). With multiple permits, a small per-tx sleep gets absorbed by parallelism and never produces queue buildup. Serialized execution turns the sleep into real congestion. Both changes are test-only and should be reverted (or feature-gated) before merge — they cripple normal throughput.

### How stress.rs hits the new path

1. stress.rs sends `execute_transaction_block` via JSON-RPC.
2. The fullnode's `TransactionOrchestrator` picks `TransactionDriver` (because `enable_white_flag_flow=true`) and submits to the validator's `ValidatorV2Server::submit_tx`.
3. The validator certifies, the cert reaches consensus, consensus output is processed in [`process_consensus_transactions_and_commit_boundary`](crates/iota-core/src/authority/authority_per_epoch_store.rs#L3091).
4. Before the transaction-categorization loop, [`compute_quorum_load_shedding_percentage`](crates/iota-core/src/authority/authority_per_epoch_store.rs#L3148) folds the persisted notifications plus this commit's `OverloadNotificationV1` entries into a single drop percentage.
5. In the categorization loop, each user-originating tx hashes through [`should_reject_tx`](crates/iota-core/src/overload_monitor.rs#L277) with the round number as temporal seed; rejected txs are dropped silently (counted in `post_consensus_load_shedding_dropped_transactions_total`) and never scheduled.

In parallel, a per-validator background task ([`start_overload_notifier`](crates/iota-node/src/lib.rs#L287)) polls the local overload monitor every `overload_monitor_interval` and submits a new `OverloadNotificationV1` whenever its percentage changes, so the receiver side has fresh values to layer on top of the persisted state.

### Grafana dashboard panel reference

The dashboard at [dev-tools/grafana-local/dashboards/pcool-load-shedding-dashboard.json](dev-tools/grafana-local/dashboards/pcool-load-shedding-dashboard.json) is auto-provisioned. Prometheus scrapes at `scrape_interval: 1s` so per-host gauges aren't smoothed over.

**Top row — sanity check that the experiment is running:**

- **Arrival TPS (per validator)** — `sum by (host) (rate(consensus_handler_processed{class=~"owned_user_transaction|shared_user_transaction"}[30s]))`. Post-consensus arrival of user txs. Under white-flag flow this is the authoritative arrival signal — `total_transaction_orders` only counts the legacy V1 path and stays near zero.
- **Useful TPS (per validator)** — `rate(total_transaction_effects[30s])`. The execution ceiling. With `execution_delay_ms=100` and serialized execution this caps at ~10 TPS per validator — that is the *expected* steady-state during overload, not a bug.

**The mechanism in action:**

1. **Local load-shedding %** — `authority_load_shedding_percentage`. The value each validator *advertises* (and broadcasts via overload notifications).
2. **Load-shedding source breakdown** — `authority_load_shedding_source{source}`. Splits the local % into its three components (latency, queue_length, cache_backpressure) so you can see which signal is firing.
3. **Notifications sent (rate)** — `rate(authority_overload_notifications_sent_total[30s])`. Notifications are only emitted on change.
4. **Notifications received (rate)** — `sum by (host, from_authority) (rate(authority_overload_notifications_received_total[30s]))`. Each host should observe ~equal counts from every peer.
5. **Quorum load-shedding %** *(full-width)* — `authority_quorum_load_shedding_percentage`. All hosts should converge on the same value within one commit. This is the value actually used to drop.
6. **Post-consensus dropped txs (rate)** — `rate(post_consensus_load_shedding_dropped_transactions_total[30s])`. Should be ≈ `quorum_pct/100 × arrival_rate`.
7. **Execution queueing delay p99** — `histogram_quantile(0.99, ...execution_queueing_delay_s_bucket)`. The latency signal the overload monitor reads.

**Raw inputs to the shedding calculation** (so you can answer *"why is this source's percentage what it is?"*):

8. **TM inflight queue length (raw signal)** — `transaction_manager_num_pending_certificates`. Queue-length-source input. Compared against `max_transaction_manager_queue_length` with graduated scaling above the soft-limit %.
9. **Ready vs execution rate** — `overload_signal_txn_ready_rate_tps` and `overload_signal_execution_rate_tps`. Latency-source inputs. The latency-based shedding formula computes its target as roughly `1 − execution_rate / ready_rate`.
10. **Writeback cache pending count (raw signal)** *(full-width)* — `overload_signal_cache_pending_count`. Cache-backpressure-source input. Note: this also accumulates randomness-DKG and other system state, so it can rise without user traffic if those back up.

Counters labeled by `from_authority` only emit data once that authority has appeared at least once, so before the network has been overloaded these panels show "no data". That's correct.

---

## Files involved

### Rust

- [crates/iota-config/src/node.rs:1260](crates/iota-config/src/node.rs#L1260) — `execution_delay_ms: Option<u64>` field on `AuthorityOverloadConfig`.
- [crates/iota-core/src/execution_driver.rs:34](crates/iota-core/src/execution_driver.rs#L34) — execution semaphore reduced to 1 for the stress build.
- [crates/iota-core/src/execution_driver.rs:118](crates/iota-core/src/execution_driver.rs#L118) — applies the configured sleep before each tx execution.
- [crates/iota-core/src/authority.rs:289](crates/iota-core/src/authority.rs#L289) — new prometheus metrics declared on `AuthorityMetrics`; the two `_sent_*` metrics are fully `pub` because `iota-node` accesses them across crate boundaries.
- [crates/iota-core/src/overload_monitor.rs:140](crates/iota-core/src/overload_monitor.rs#L140) — `authority_load_shedding_source` set per signal each tick.
- [crates/iota-core/src/authority/authority_per_epoch_store.rs:3152](crates/iota-core/src/authority/authority_per_epoch_store.rs#L3152) — `authority_quorum_load_shedding_percentage` set after computing the drop percentage.
- [crates/iota-core/src/authority/authority_per_epoch_store.rs:3170](crates/iota-core/src/authority/authority_per_epoch_store.rs#L3170) — `post_consensus_load_shedding_dropped_transactions_total` incremented on each dropped user tx.
- [crates/iota-core/src/authority/authority_per_epoch_store.rs:4555](crates/iota-core/src/authority/authority_per_epoch_store.rs#L4555) — `authority_overload_notifications_received_total{from_authority}` on receipt.
- [crates/iota-node/src/lib.rs:317](crates/iota-node/src/lib.rs#L317) — `authority_overload_notifications_sent_total` on successful submit.

### Network / dashboards / harness

- [dev-tools/iota-private-network/configs/validator-common.yaml](dev-tools/iota-private-network/configs/validator-common.yaml) — `authority-overload-config` overlay with the new `execution-delay-ms` knob.
- [dev-tools/iota-private-network/bootstrap.sh](dev-tools/iota-private-network/bootstrap.sh) — yq overlay pipeline extended to copy the `authority-overload-config` block onto each validator's YAML.
- [dev-tools/grafana-local/prometheus.yaml](dev-tools/grafana-local/prometheus.yaml) — `scrape_interval: 1s`.
- [dev-tools/grafana-local/dashboards/pcool-load-shedding-dashboard.json](dev-tools/grafana-local/dashboards/pcool-load-shedding-dashboard.json) — 10-panel dashboard, UID `pcool-load-shedding-stress`.
- [run-pcool-experiment.sh](run-pcool-experiment.sh) — sole entry point at the repo root.
- [stress-pcool-load-shedding.sh](stress-pcool-load-shedding.sh) — invoked by `run-pcool-experiment.sh` after bring-up; do not call directly.

## Open work

- **Cleanup of `execution_delay_ms` and semaphore-of-1 for non-stress builds.** Both are test-only changes that should not land on a release branch unless gated. The field is `Option<u64>` so it's a no-op at default; the semaphore reduction is the bigger concern and needs reverting (or feature-gating) before merge.
- **Honest-traffic-alongside-spam check.** Drive a separate low-rate honest workload (e.g. `--target-qps 50` from a distinct keystore) during a shedding event and confirm honest-tx finality stays bounded. The current drop mechanism is hash-based and indifferent to traffic origin, so we expect some honest drops at high quorum percentages, but tail latency should not explode.
- **Production-scale validation.** The 4-validator laptop setup exposes the mechanism but not the failure modes of n=100, `max_transaction_manager_queue_length=100k`, etc.
- **Quorum-percentile boundary cases.** What happens at exactly 2f+1 stake-slowed? The current tests cover the obvious cases (1/4, 2/4) but not 3/4 with mixed delay values (e.g. `100,200,300,0` — what does the 2f+1 percentile of that look like in practice?).
- **Recovery dynamics under intermittent spam.** Pulse the load delay on/off and confirm the system recovers fully each time rather than accumulating residual quorum %.
