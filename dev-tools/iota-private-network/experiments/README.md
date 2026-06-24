# Run Local Network & Mimic Artificial Latency & Fuzz Disruptions Suite

This suite automates network perturbation experiments against an IOTA private validator network.\
Use it to:

- bring up a local validator cluster,
- mimic realistic latencies (role-based model, or topology profiles: geo-distributed, ring, star, random, …),
- introduce controlled failures (packet loss, blocked connections, validator restarts),
- optionally spam the network with transactions,
- collect logs and basic network statistics.

Three Python runners orchestrate the workflows, sharing `experiment_common.py` (logging, subprocess helpers, Prometheus queries, and the network phases). Each **generates its docker compose file per run** — one service block per validator — and supports 2–30 validators, matching the Prometheus scrape configuration. The runners drive two lower-level Bash injectors:

- `run-benchmark.py` → `network-benchmark.sh` (deterministic role-based latency model + optional block/loss/restart).
- `run-fuzz.py` → `network-fuzz.sh` (topology latency profiles + loss/block/restart + heal rounds / TTL).
- `run-migration-test.py` → `network-benchmark.sh` (rolling upgrade across an epoch boundary).

Run every runner from inside `iota/dev-tools/iota-private-network/experiments/`.

---

## Prerequisites

- Linux host
- Docker (v20.10+)
- **gaiadocker/iproute2** image (for `tc netem` commands)
- **nicolaka/netshoot** image (for `iptables` testing)
- `sudo` access on the host (for `iptables` and `tc` via `nsenter`)
- `docker compose` for Grafana

Only one experiment run (benchmark, fuzz, or migration) can be active at a
time — they share container names and host `tc`/`iptables` state. A second
run aborts immediately, naming the current holder (lock:
`/tmp/iota-experiments.lock`).

The scripts apply:

- host-level `iptables` rules in the `DOCKER-USER` chain to drop traffic between validator containers, and
- `tc netem` in each validator network namespace (via `nsenter`) to simulate latency and loss.

Optional but useful tools for debugging:

```bash
docker pull nicolaka/netshoot
```

---
## Main Benchmark Runner

`run-benchmark.py` automates the full workflow:

1. Optionally rebuilds the `iota-node`, `iota-tools`, and `iota-indexer` Docker images.
2. Generates a docker compose file for N validators and bootstraps genesis.
3. Starts the validators (and a fullnode when the spammer is enabled).
4. Starts Grafana/Prometheus on the experiment network (available at `http://localhost:3000/dashboards`).
5. Applies the role-based latency model (`network-benchmark.sh`) plus optional block/loss/restart disruptions.
6. Optionally starts a transaction spammer, then measures block production under that load, runs for a fixed duration collecting logs, and tears everything down.

Supports the following flags:

- `-n <NUM>`: number of validators (2–30, default: `4`; compose is generated per run)
- `-b <true|false>`: rebuild Docker images before running (default: `true`)
- `-g <true|false>`: enable geodistributed large network latencies (default: `true`; `false` divides all delays by 4 and drops the heavy-tail slot bursts)
- `-s <SEED>`: seed for pseudorandom disruptions (default: `42`)
- `-x <PERCENT_BLOCK>`: percent of validator pairs to block connections (default: `0`)
- `-l <PERCENT_LOSS>`: percent of validators to apply source-wide packet loss while preserving each per-peer latency rule (default: `0`)
- `-r <PERCENT_RESTART>`: percent of validators to restart periodically (default: `0`)
- `-t <RUN_DURATION>`: total experiment duration in seconds (default: `3600`)
- `-d <RESTART_DURATION>`: seconds a validator stays stopped per restart (default: `120`)
- `-w <RESTART_TIMEOUT>`: seconds to wait before restarting (default: `60`)
- `-M <RESTART_MODE>`: `preserve-consensus` | `full-reset` | `simple-restart` (default: `preserve-consensus`)
- `-E <EPOCH_DURATION_MS>`: epoch duration in milliseconds (default: `1200000`, 20 min)
- `-m`: output per-validator network metric statistics (packets and bytes) at teardown.
- `-S <true|false>`: enable the transaction spammer (default: `false`)
- `-T <TPS>`: transactions per second used by the spammer (default: `10`)
- `-Z <SIZE>`: per-transaction size for the `iota-spammer` spammer, e.g. `10KiB` (default: `10KiB`)
- `-C <spammer_type>`: type of spammer to use (default: `stress`; another option: `iota-spammer`, which runs on the host and needs a `~/iota-spammer` clone with a Rust toolchain — the runner then also starts a faucet and publishes the fullnode RPC (`127.0.0.1:9000`) and faucet (`127.0.0.1:5003`))
- `-c <testnet|mainnet>`: protocol-config chain override (default: empty → `testnet`)
- `--block-measurement-seconds <S>`: measurement window under the applied latency, disruptions, and optional load, reporting per-validator block rates, block-creation reasons, and block/transaction commit latencies (p50/p95) (default: `90`; `0` disables)

Run from inside the `iota/dev-tools/iota-private-network/experiments/` directory.

**Usage:**

```bash
# Run default 4-validator Starfish network with geodistributed latencies without any additional disruptions
./run-benchmark.py

# Run 10-validator network with small latencies for one hour without rebuilding images
./run-benchmark.py -n 10 -g false -b false

# Run 30-validator network with geodistributed latencies, 10% blocked connections, 5% chances for packet loss, 10% for restarts and running for 2 hours
./run-benchmark.py -n 30 -g true -x 10 -l 5 -r 10 -t 7200
```
---

## Transaction Spammers

Enable a spammer with `-S true`; both `run-benchmark.py` and `run-fuzz.py` then
bring up `fullnode-1` as the RPC target and generate load against it. Two
options are selectable with `-C`:

### 1. `stress` (default) — the `iota-benchmark` tool

The `stress` binary is the **`iota-benchmark`** load tool, distributed as the
`iotaledger/stress` Docker image (built from the
[`iotaledger/network-benchmark`](https://github.com/iotaledger/network-benchmark)
repo). It submits `--target-qps` transfer transactions through `fullnode-1`.

The runner resolves the image **up front, before the network starts**: it uses
a local copy, else pulls it, else **builds it** from a `~/network-benchmark`
clone (`docker/stress/build.sh` tags `iotaledger/stress`; the first build takes
~30 min, later ones hit the docker cache). A missing clone is fetched only
after a timeout-guarded y/N confirmation (auto-No); an existing clone is
ff-only updated best-effort. If the image still can't be obtained the run
**fails** (load was explicitly requested) — `docker login` to the registry,
clone the repo, or pass a different `--spammer-image`. After startup the runner
re-checks that the container survived its first seconds and fails with the
container logs if not. The migration runner uses the same resolution for
`--load-tools-image`.

```bash
# stress at 500 TPS
./run-benchmark.py -n 4 -S true -T 500
# custom / locally-built image
./run-benchmark.py -n 4 -S true -T 500 --spammer-image my-stress:local
```

During a run, stream its output with `docker logs stress-benchmark`. Cleanup
archives it as `logs/stress-benchmark-latest.log` and a timestamped copy.

### 2. `iota-spammer` — external sizable-transaction spammer

`iota-spammer` is a script from the **private**
[`iotaledger/iota-spammer`](https://github.com/iotaledger/iota-spammer) repo.
Clone it to `~/iota-spammer` (i.e. `../../../iota-spammer` from the runner). It
adds a `sizable` transaction type whose payload size is set with `-Z`:

```bash
./run-benchmark.py -n 4 -S true -C iota-spammer -T 100 -Z 10KiB
```

Logs are written to `logs/spammer.log`. If the script is absent, the run fails
because the requested load was not started. Cleanup terminates the complete
host-side spammer process group.

## Main Fuzz Runner: `run-fuzz.py`

`run-fuzz.py` automates the full workflow:

1. Optionally rebuilds the `iota-node`, `iota-tools`, and `iota-indexer` Docker images.
2. Generates a docker compose file for N validators and bootstraps genesis.
3. Starts the validators (and a fullnode when the spammer is enabled).
4. Starts Grafana/Prometheus on the experiment network (available at `http://localhost:3000/dashboards`).
5. Launches `network-fuzz.sh` to apply network latencies and controlled disruptions:
   - topology-dependent artificial RTTs,
   - packet loss on a subset of validators,
   - host-level connection blocking (bidirectional, `DOCKER-USER` chain),
   - periodic validator restarts,
   - optional heal rounds and TTL.
6. Optionally starts a transaction spammer, then measures block production under that load, runs for a fixed duration collecting logs, and tears everything down (including the host `fuzzdrop` iptables rules).

Run from inside `iota/dev-tools/iota-private-network/experiments/`:

```
./run-fuzz.py [options]
```

Supported flags:

- `-n <NUM>`: number of validators (2–30, default: `4`; compose is generated per run).
- `-b <true|false>`: rebuild Docker images before running (default: `true`).
- `-t <topology>`: topology / latency profile — `ring` | `star` | `non-triangle` | `random` | `geo-high` | `geo-low` (default: `geo-low`).
- `-s <SEED>`: seed for deterministic pseudorandom disruptions (default: `42`).
- `-x <PERCENT_BLOCK>`: percent of unordered validator pairs to block bidirectionally at the host level (default: `0`).
- `-l <PERCENT_LOSS>`: percent of validators to apply `tc netem` packet loss to (default: `0`).
- `-r <PERCENT_RESTART>`: percent of validators to restart periodically (default: `0`).
- `-d <RUN_DURATION>`: total experiment duration in seconds (default: `3600`).
- `--restart-duration <SECONDS>`: seconds a validator stays stopped per restart (default: `120`).
- `--round-span <SECONDS>`: fuzz round length (default: `0` = `2 * restart_duration`).
- `--ttl <SECONDS>`: fuzz TTL; the fuzzer stops itself when reached (default: `0` = none).
- `--heal-every-round <N>` / `--heal-num-rounds <N>`: periodic heal rounds (default: `0` = disabled).
- `-E <EPOCH_DURATION_MS>`: epoch duration in milliseconds (default: `1200000`, 20 min).
- `-m`: output per-validator network metric statistics at teardown.
- `-S <true|false>`: enable the transaction spammer (default: `false`).
- `-T <TPS>`: transactions per second used by the spammer (default: `10`).
- `-Z <SIZE>`: per-transaction size for the `iota-spammer` spammer, e.g. `10KiB` (default: `10KiB`).
- `-C <spammer_type>`: spammer type (default: `stress`; alternative: `iota-spammer`, which runs on the host and needs a `~/iota-spammer` clone with a Rust toolchain — the runner then also starts a faucet and publishes the fullnode RPC (`127.0.0.1:9000`) and faucet (`127.0.0.1:5003`)).
- `-c <testnet|mainnet>`: protocol-config chain override (default: empty → `testnet`).
- `--block-measurement-seconds <S>`: post-fuzz measurement window reporting per-validator block rates, block-creation reasons, and block/transaction commit latencies (p50/p95) (default: `90`; `0` disables).

- `-h`\
  Show help and exit.

### Fuzz round / heal tuning

These `run-fuzz.py` flags control the fuzzer's round schedule and are passed
through to `network-fuzz.sh`:

- `--ttl <SECONDS>` (`0` disables): when reached, `network-fuzz.sh` writes a stopfile and shuts itself down cleanly.
- `--round-span <SECONDS>` (`0` = `2 * restart_duration`): duration of one fuzz round.
- `--restart-duration <SECONDS>`: how long validators stay stopped during restart rounds.
- `--heal-every-round <N>` (`0` disabled): every Nth round becomes a heal window.
- `--heal-num-rounds <N>`: consecutive rounds after a heal trigger during which **no restarts** are applied.

---

## Internal Fuzzing Script: `network-fuzz.sh` (Overview)

You normally don’t call `network-fuzz.sh` directly; `run-fuzz.py` does it for you.\
Conceptual behavior:

- Builds a latency matrix `LAT_MS[i|j]` based on the chosen topology (`geo-high`, `geo-low`, `ring`, `star`, `non-triangle`, `random`).
- Assigns node-level packet loss via `LOSS_PCT_NODE[i]`.
- Builds a set of blocked validator pairs using `PERCENT_BLOCK`:
  - chooses `M * PERCENT_BLOCK / 100` unordered pairs out of all `N(N−1)/2` possibilities,
  - for each pair `(i, j)`, marks `BLOCK_EDGE["i|j"] = BLOCK_EDGE["j|i"] = 1`,
  - applies host-level drops for these pairs on `DOCKER-USER`: both directions (`i → j` and `j → i`) are installed.
- Periodically:
  - re-applies `tc` inside each container (watcher),
  - enforces restart rounds,
  - rebalances the random cut set (`BLOCK_EDGE`) per fuzz round,
  - optionally runs heal rounds (removing all `fuzzdrop:` rules and zeroing packet loss).

All drops installed by the fuzz script are tagged with\
`-m comment --comment "fuzzdrop:..."` and cleaned up by the fuzz cleanup logic and by `run-fuzz.py` at teardown.

---

## Examples

### 1. Default 4-validator Starfish network, low latencies, no extra disruptions

```
./run-fuzz.py
```

- 4 validators
- protocol `starfish` (default)
- topology `geo-low` (low RTTs)
- no blocked pairs, no packet loss, no restarts
- no spammer

### 2. 10-validator Starfish network, high geo-distributed latencies, 1-hour run, no rebuild

```
./run-fuzz.py \
  -n 10 \
  -b false \
  -t geo-high \
  -d 3600
```

### 3. 25-validator Starfish, geo-high RTTs, 10% blocked pairs, 5% loss, 10% restarts, 2-hour run

```
./run-fuzz.py \
  -n 25 \
  -b true \
  -t geo-high \
  -x 10 \
  -l 5 \
  -r 10 \
  -d 7200
```

- 10% of validator pairs are selected and blocked bidirectionally at the host level (`iptables`).
- 5% of validators get 1–5% packet loss.
- 10% of validators are periodically restarted per restart round.

### 4. Same as above, but with a fuzz TTL and heal rounds

```
./run-fuzz.py \
  -n 25 \
  -t geo-high \
  -x 10 \
  -l 5 \
  -r 10 \
  -d 7200 \
  --ttl 3600 \
  --heal-every-round 3 \
  --heal-num-rounds 1
```

- `network-fuzz.sh` will self-terminate after 3600 seconds.
- Every 3rd round is a heal trigger, and the first heal round clears all host-level drops and resets packet loss.

---

## Transaction Spammers (fuzz)

`run-fuzz.py` supports the same two spammers as the benchmark — `stress` (the
`iota-benchmark` tool via `iotaledger/stress`, auto-pulled) and the external
`iota-spammer` — selected with `-C` and enabled with `-S true`. See
[Transaction Spammers](#transaction-spammers) above for setup, the auto-pull
behavior, and `--spammer-image`. Example:

```
./run-fuzz.py -n 4 -t geo-high -S true -C stress -T 500
```

---

## Logs & Outputs

- Experiment coordinator logs (this script):
  - `logs/experiment_script_latest.log`
  - `logs/experiment_script_<TIMESTAMP>.log`

- Per-validator logs (periodically updated “latest” + final snapshot):
  - `logs/exp-validator-<i>-latest.log` / `logs/fuzz-validator-<i>-latest.log`
  - `logs/experiment-<TIMESTAMP>-validator-<i>.log` / `logs/fuzz-<TIMESTAMP>-validator-<i>.log`

- Fuzz script logs:
  - `logs/fuzz_<TIMESTAMP>.log` (the file `run-fuzz.py` passes via `-o` to `network-fuzz.sh`).

- Spammer logs (if enabled):
  - `logs/spammer.log` (iota-spammer)
  - `logs/stress-benchmark-latest.log` and `logs/stress-benchmark-<TIMESTAMP>.log` (stress)

On exit, `run-fuzz.py`:

- kills the fuzzer and spam processes,
- tears down the generated compose project,
- clears any remaining `fuzzdrop:` rules from the host `DOCKER-USER` chain.

---

## Rolling Migration Test: `run-migration-test.py`

`run-migration-test.py` validates that a rolling upgrade from a released validator image to a locally-built image succeeds across an epoch boundary. It pulls the old image from Docker Hub, bootstraps a local network, applies the role-based latency model built into `network-benchmark.sh` (hub / `48-54ms` band / relay follower / one heavy-tail validator per decade of ten), and performs the rolling upgrade under monitoring and optional load. The heavy-tail node is the slowest block producer by design (≥ `1 blk/s` below the fastest), with block-creation reasons ordered AddBlock > AddBlockHeader > MinBlockDelayTimeout — validated live for every `n` in `10..24` and `30`. The effective per-edge matrix is dumped to `logs/latency-matrix.tsv` (`network-benchmark.sh -D`); see that script's header comment for the exact bands and slot bursts.

Two modes (`--mode`, default `simple`):

- **simple** — fast back-to-back rolling upgrade after a fixed warm-up inside epoch 0, then a stable-window comparison (same-length windows after monitoring/latency/load setup and after the next epoch boundary). No post-upgrade restarts.
- **advanced** — full schedule: mid-epoch wait, randomized per-validator offline windows during the rolling upgrade, then keep-DB and wipe-DB restart stress across two post-upgrade epochs.

The script must be run from inside:

```
iota/dev-tools/iota-private-network/experiments/
```

### Usage

```
./run-migration-test.py [options]
```

Supported flags:

- `--mode <simple|advanced>`\
  Test schedule, see above (default: `simple`).

- `-r <network>`\
  Release network to pull the old image from (`devnet`, `testnet`, `mainnet`, `alphanet`; default: `testnet`).

- `-b <true|false>`\
  Build the local upgrade image before running (default: `true`).

- `-n <N>`\
  Number of validators (2–30, default: `10`).

- `-c <chain>`\
  Chain override for protocol feature flags (`testnet`, `mainnet`, or empty; default: empty, which **inherits from `-r`** — `testnet`/`mainnet` set the matching override, `devnet`/`alphanet` set none. With the default `-r testnet` the network therefore runs with testnet feature flags).

- `-e <MINUTES>`\
  Epoch duration in minutes (default: `10`).

- `--geodistributed <true|false>`\
  Use the full geodistributed latency values (default: `true`; `false` divides all delays by 4 and drops the heavy-tail slot bursts).

- `--block-measurement-seconds <S>`\
  Pre-upgrade block-production measurement window after latency is applied, reporting per-validator block rates, block-creation reasons, and block/transaction commit latencies (p50/p95) (default: `120`, `0` disables; simple mode only — the advanced schedule does not budget for it). The legacy name `--block-validation-seconds` is accepted as an alias.

- `--load-qps <QPS>`\
  Start a stress load generator at target QPS (default: `0` = disabled).

- `--load-in-flight-ratio <N>`\
  Stress load in-flight ratio (default: `5`).

- `--load-transfer-objects <N>`\
  Stress load `--transfer-object` value (default: `100`).

### Phases

1. **Image preparation** — pull released image, optionally build local image with BuildKit caching
2. **Compose generation** — write `docker-compose.migration.yaml` for N validators (plus a fullnode when load is enabled)
3. **Genesis bootstrap** — generate genesis template and validator configs
4. **Network startup** — start validators, verify all are running (exact name matching, hard failure)
5. **Monitoring** — (re)create the Grafana/Prometheus stack with `--force-recreate`, so a container left over from a prior run rebinds to the current network
6. **Latency injection** — dump the effective role-based matrix (`network-benchmark.sh -D`) for the log, then launch `network-benchmark.sh`, which computes and applies the same model natively; optionally start the load generator and report block production under it
7. **Pre-rolling wait** — fixed warm-up offset into epoch 0 (simple) or mid-epoch wait (advanced)
8. **Rolling upgrade** — upgrade validators one-by-one; hard failure if any validator isn't running afterwards
9. **Post-upgrade** — simple: wait for the next epoch boundary and run the stable-window comparison; advanced: keep-DB and wipe-DB restart stress across two post-upgrade epochs, then extended checkpoint liveness observation

### Examples

```bash
# Default: simple mode, 10 validators, testnet release (testnet chain flags), 10-min epochs
./run-migration-test.py

# Full restart-stress schedule
./run-migration-test.py --mode advanced

# Devnet release (no chain override), 20 validators, 15-min epochs
./run-migration-test.py -r devnet -n 20 -e 15

# With load generator at 100 QPS
./run-migration-test.py --load-qps 100
```

### Logs

- Main log: `logs/migration_script_latest.log` (archived as `logs/migration_script_<TIMESTAMP>.log`)
- Per-validator logs: `logs/exp-validator-<i>-latest.log` during the run; final snapshots `logs/migration-validator-<i>-latest.log` (+ timestamped copies, and a fullnode snapshot when load is enabled)
- Load generator logs: `logs/load-generator-latest.log`

---
