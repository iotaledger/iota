# Run Local Network & Mimic Artificial Latency & Fuzz Disruptions Suite

This suite of Bash scripts automates network perturbation experiments against an IOTA private validator network.\
Use it to:

- bring up a local validator cluster,
- mimic realistic latencies (geo-distributed, ring, star, random, …),
- introduce controlled failures (packet loss, blocked connections, validator restarts),
- optionally spam the network with transactions,
- collect logs and basic network statistics.

All orchestration is done via `run-all-fuzz.sh`, which internally uses `network-fuzz.sh` to apply latency and disruptions.

---

## Prerequisites

- Linux host
- Docker (v20.10+)
- **gaiadocker/iproute2** image (for `tc netem` commands)
- **nicolaka/netshoot** image (for `iptables` testing)
- `sudo` access on the host (for `iptables` and `tc` via `nsenter`)
- `docker compose` for Grafana

The scripts apply:

- host-level `iptables` rules in the `DOCKER-USER` chain to drop traffic between validator containers, and
- `tc netem` in each validator network namespace (via `nsenter`) to simulate latency and loss.

Optional but useful tools for debugging:

```bash
docker pull nicolaka/netshoot
```

---
## Main Benchmark Script

`run-all-benchmark.sh` automates the full workflow:

1. Optionally rebuilds the `iota-node` and `iota-tools` Docker images.
2. Bootstraps the validator network.
3. Runs the private network.
4. Runs grafana (available at `http://localhost:3000/dashboards`)
5. Applies network latencies and controlled disruptions (packet loss, connection blocking, validator restarts).
6. Periodically collects logs and saves them with timestamps.

Supports the following flags:

- `-n <NUM>`: number of validators (default: `4`; any number between `4` and `30` is supported)
- `-b <true|false>`: rebuild Docker images before running (default: `true`)
- `-g <true|false>`: enable geodistributed large network latencies (default: `true`; `false` divides all delays by 4 and drops the heavy-tail slot bursts)
- `-s <SEED>`: seed for pseudorandom disruptions (default: `42`)
- `-x <PERCENT_BLOCK>`: percent of validator pairs to block connections (default: `0`)
- `-l <PERCENT_NETEM>`: percent of validators to apply packet loss (default: `0`)
- `-r <PERCENT_RESTART>`: percent of validators to restart periodically (default: `0`)
- `-t <RUN_DURATION>`: total experiment duration in seconds (default: `3600`)
- `-m`: optional flag to output network metric statistics (packets and bytes).
- `-S <true|false>`: enable the transaction spammer (default: `false`)
- `-T <TPS>`: transactions per second used by the spammer (default: `100`)
- `-Z <TRX_SIZE>`: number of shared objects per transaction for the spammer (default: `10`)
- `-C <spammer_type>`: type of spammer to use (default: `stress`; another option: `iota-spammer`)

The script should be run from inside the `iota/dev-tools/iota-private-network/experiments/` directory.

**Usage:**

```bash
# Run default 4-validator Starfish network with geodistributed latencies without any additional disruptions
./run-all-benchmark.sh

# Run 10-validator network with small latencies for one hour without rebuilding images
./run-all-benchmark.sh -n 10 -g false -b false

# Run 30-validator network with geodistributed latencies, 10% blocked connections, 5% chances for packet loss, 10% for restarts and running for 2 hours
./run-all-benchmark.sh -n 30 -g true -x 10 -l 5 -r 10 -t 7200
```
---

## Optional Transaction Spammer

The experiment suite can optionally include a transaction spammer to generate load on the validator network during the run.
It supports two types of spammer tools, by default the stress test from the iota benchmark, and optionally the `iota-spammer` from a private repository.

### With default spammer enabled:

```bash
./run-all-benchmark.sh -n 4 -S true -T 500
```

This will load the default spammer with a TPS of 500.

### Required Setup for optional Spammer

To enable the optional spammer set `-S true` and '-C iota-spammer' you must clone the following **private** repository:

```
https://github.com/iotaledger/iota-spammer
```

Place it at the following relative path from `run-all-benchmark.sh`, or update the path in the script accordingly:

```
../../../iota-spammer
```

The optional spammer allows a special transaction type, called `sizable`, and can be used as follows:

```bash
./run-all-benchmark.sh -n 4 -S true -T 100 -Z 10KiB
```

This will launch the spammer from the external repository with the configured transaction rate, TPS=100, and size, 10KiB.

## Main Fuzz Script: `run-all-fuzz.sh`

`run-all-fuzz.sh` automates the full workflow:

1. Optionally rebuilds the `iota-node`, `iota-tools`, and `iota-indexer` Docker images.
2. Bootstraps the validator network.
3. Runs the private network.
4. Starts Grafana (available at `http://localhost:3000/dashboards`).
5. Launches `network-fuzz.sh` to apply network latencies and controlled disruptions:
   - artificial RTTs (topology-dependent),
   - packet loss on a subset of validators,
   - host-level connection blocking (bidirectional),
   - periodic validator restarts,
   - optional heal rounds and TTL.
6. Periodically collects validator logs and saves them with timestamps.
7. Optionally runs a transaction spammer to generate load.

The script must be run from inside:

```
iota/dev-tools/iota-private-network/experiments/
```

---

## Usage

```
./run-all-fuzz.sh [options]
```

Supported flags:

- `-n <NUM>`\
  Number of validators (default: `4`; supports `4`–`19`).

- `-b <true|false>`\
  Rebuild Docker images before running (default: `true`).

- `-t <topology>`\
  Topology / latency profile for the fuzz script. Accepted values:
  - `ring`
  - `star`
  - `non-triangle`
  - `random`
  - `geo-high`
  - `geo-low`

  Default: `false` (mapped to `geo-low`).

- `-s <SEED>`\
  Seed for deterministic pseudorandom disruptions (default: `42`).

- `-x <PERCENT_BLOCK>`\
  Percentage of unordered validator pairs to block at the host level (0–100).\
  For each selected pair `(i, j)`, traffic is blocked bidirectionally via `iptables` (`i ↔ j`).

- `-l <PERCENT_LOSS>`\
  Percentage of validators to apply `tc netem` packet loss to (0–100).\
  Selected validators get a random loss in `[1%, 5%]`.

- `-r <PERCENT_RESTART>`\
  Percentage of validators to restart periodically (0–100).\
  The fuzz script chooses a deterministic batch per round, stops them for a configurable duration, then restarts them.

- `-d <RUN_DURATION>`\
  Total experiment duration in seconds (default: `3600`).

- `-m`\
  Enable printing network metrics (TX/RX bytes and packets per validator) at the end.

- `-S <true|false>`\
  Enable the transaction spammer (default: `false`).

- `-T <TPS>`\
  Transactions per second used by the spammer (default: `10`).

- `-Z <SIZE>`\
  For `iota-spammer`**: size per transaction, e.g. `10KiB` (default: `10KiB`).

- `-C <spammer_type>`\
  Spammer type (default: `stress`; alternative: `iota-spammer`).

- `-h`\
  Show help and exit.

### Environment overrides for network fuzzing

These environment variables fine-tune how `network-fuzz.sh` behaves (they are passed through by `run-all.sh`):

- `FUZZ_TTL`\
  TTL in seconds for the fuzz script (`--ttl` argument). `0` disables TTL.\
  When TTL is reached, `network-fuzz.sh` creates a stopfile and shuts itself down cleanly.

- `FUZZ_ROUND_SPAN`\
  Duration of a fuzz “round” in seconds (`--round-span`).\
  `0` means “use `2 * RESTART_DURATION` inside `network-fuzz.sh`”.

- `FUZZ_RESTART_DURATION`\
  Duration (seconds) to stop validators during restart rounds.\
  Passed as `-d` to `network-fuzz.sh` (default inside `run-all-fuzz.sh`: `120`).

- `HEAL_EVERY_ROUND`\
  If `> 0`, every `HEAL_EVERY_ROUND`-th fuzz round becomes a “heal window”.

- `HEAL_NUM_ROUNDS`\
  Number of consecutive rounds after the heal trigger during which **no restarts** are applied (but `tc` may still be active, depending on configuration).

---

## Internal Fuzzing Script: `network-fuzz.sh` (Overview)

You normally don’t call `network-fuzz.sh` directly; `run-all-fuzz.sh` does it for you.\
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
`-m comment --comment "fuzzdrop:..."` and cleaned up by the fuzz cleanup logic and by `run-all-fuzz.sh` before and after runs.

---

## Examples

### 1. Default 4-validator Starfish network, low latencies, no extra disruptions

```
./run-all-fuzz.sh
```

- 4 validators
- protocol `starfish` (default)
- topology `false` → `geo-low` (low RTTs)
- no blocked pairs, no packet loss, no restarts
- no spammer

### 2. 10-validator Starfish network, high geo-distributed latencies, 1-hour run, no rebuild

```
./run-all-fuzz.sh \
  -n 10 \
  -b false \
  -t true \
  -d 3600
```

Here `-t true` maps to `geo-high`.

### 3. 19-validator Starfish, geo-high RTTs, 10% blocked pairs, 5% loss, 10% restarts, 2-hour run

```
./run-all-fuzz.sh \
  -n 19 \
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
FUZZ_TTL=3600 \
HEAL_EVERY_ROUND=3 \
HEAL_NUM_ROUNDS=1 \
./run-all-fuzz.sh \
  -n 19 \
  -t geo-high \
  -x 10 \
  -l 5 \
  -r 10 \
  -d 7200
```

- `network-fuzz.sh` will self-terminate after 3600 seconds.
- Every 3rd round is a heal trigger, and the first heal round clears all host-level drops and resets packet loss.

---

## Optional Transaction Spammer

The experiment suite can optionally include a transaction spammer to generate load on the validator network.

Two modes are supported:

1. **`stress`** (default)\
   Uses the stress binary inside a Docker container (`iotaledger/stress`) to send transactions against `fullnode-1`.

2. **`iota-spammer`** (external repo, optional)\
   Uses a custom spammer script from a private repository.

### Enable default stress benchmark spammer

```
./run-all-fuzz.sh -n 4 -S true -T 500
```

- Starts `faucet-1`.
- Runs the stress benchmark with `target-qps = 500` using Starfish (default).
- Writes spammer logs to `logs/spammer.log`.

### Enable `iota-spammer` (external repo)

To use the `iota-spammer`:

1. Clone the private repository:

   ```
   git clone https://github.com/iotaledger/iota-spammer
   ```

2. Place it at the following relative path from `run-all-fuzz.sh`, or adjust the `SPAMMER_SCRIPT` path in `run-all-fuzz.sh`:

   ```
   ../../../iota-spammer
   ```

3. Run `run-all-fuzz.sh` with `SPAMMER_TYPE=iota-spammer`:

   ```
   ./run-all-fuzz.sh \
     -n 4 \
     -S true \
     -C iota-spammer \
     -T 100 \
     -Z 10KiB
   ```

This launches the external spammer script with:

- TPS = 100
- transaction size ≈ 10 KiB (as interpreted by the spammer).

Logs are written to `logs/spammer.log`.

---

## Logs & Outputs

- Experiment coordinator logs (this script):
  - `logs/experiment_script_latest.log`
  - `logs/experiment_script_<TIMESTAMP>.log`

- Per-validator logs (periodically updated “latest” + final snapshot):
  - `logs/exp-validator-<i>-latest.log`
  - `logs/experiment-validator-<i>-<TIMESTAMP>.log`

- Fuzz script logs:
  - `logs/fuzz_<TIMESTAMP>.log` (or the file specified via `-o` in `network-fuzz.sh`).

- Spammer logs (if enabled):
  - `logs/spammer.log`

On exit, `run-all-fuzz.sh`:

- kills fuzz and spam processes,
- runs `cleanup.sh` (external script) to tear down Docker containers,
- attempts to clear any remaining `tc` and `fuzzdrop:` rules.

---

## Rolling Migration Test: `run-migration-test.py`

`run-migration-test.py` validates that a rolling upgrade from a released validator image to a locally-built image succeeds across an epoch boundary. It pulls the old image from Docker Hub, bootstraps a local network, applies the role-based latency model built into `network-benchmark.sh`, and performs a rolling upgrade. Roles repeat every ten validators: a hub with fast inbound spokes, an ordinary `48-54ms` band, a relay follower whose rounds complete via headers embedded in hub blocks, and one heavy-tail validator per decade with deep volatile direct latencies (`540-659ms ± 150ms`, 80% correlated wander, so every direct edge into it stays above `500ms` on average) plus a single bursty `60ms` hub route (netem `slot`; `100-146ms` at `n=10`, both bounds growing `2ms` per validator above 10 to track the longer band round) that makes it skip rounds. On the testnet image the per-decade heavy-tails are the slowest block producers, each at least `1 blk/s` below the fastest validator, with block-creation reasons ordered AddBlock > AddBlockHeader > MinBlockDelayTimeout (validated live for every `n` in `10..24` and `30`; the absolute band pace declines with `N` — `~18 blk/s` at `n=10`, `~13 blk/s` at `n=30`). The effective matrix is dumped to `logs/latency-matrix.tsv` (`network-benchmark.sh -D`); larger validator sets repeat the same role profiles per decade.

Two modes (`--mode`, default `simple`):

- **simple** — fast back-to-back rolling upgrade after a short fixed warm-up inside epoch 0, then a stable-window comparison (same-length measurement windows before the upgrade and after the next epoch boundary). No post-upgrade restarts.
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
  Number of validators (2–100, default: `10`).

- `-c <chain>`\
  Chain override for protocol feature flags (`testnet`, `mainnet`, or empty; default: empty, which **inherits from `-r`** — `testnet`/`mainnet` set the matching override, `devnet`/`alphanet` set none. With the default `-r testnet` the network therefore runs with testnet feature flags).

- `-e <MINUTES>`\
  Epoch duration in minutes (default: `10`).

- `--geodistributed <true|false>`\
  Use the full geodistributed latency values (default: `true`; `false` divides all delays by 4 and drops the heavy-tail slot bursts).

- `--block-measurement-seconds <S>`\
  Pre-upgrade block-production measurement window after latency is applied (default: `120`, `0` disables; simple mode only — the advanced schedule does not budget for it). The legacy name `--block-validation-seconds` is accepted as an alias.

- `--load-qps <QPS>`\
  Start a stress load generator at target QPS (default: `0` = disabled).

- `--load-in-flight-ratio <N>`\
  Stress load in-flight ratio (default: `5`).

- `--load-transfer-objects <N>`\
  Stress load `--transfer-object` value (default: `100`).

### Phases

1. **Image preparation** — pull released image, optionally build local image with BuildKit caching
2. **Compose generation** — write `docker-compose.migration.yaml` for N validators with Prometheus/Grafana
3. **Genesis bootstrap** — generate genesis template and validator configs
4. **Network startup** — start validators, verify all are running (exact name matching, hard failure)
5. **Latency injection** — dump the effective role-based matrix (`network-benchmark.sh -D`) for the log, then launch `network-benchmark.sh`, which computes and applies the same model natively; optionally start the load generator and report pre-upgrade block production
6. **Pre-rolling wait** — fixed warm-up offset into epoch 0 (simple) or mid-epoch wait (advanced)
7. **Rolling upgrade** — upgrade validators one-by-one; hard failure if any validator isn't running afterwards
8. **Post-upgrade** — simple: wait for the next epoch boundary and run the stable-window comparison; advanced: keep-DB and wipe-DB restart stress across two post-upgrade epochs, then extended checkpoint liveness observation

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
- Per-validator logs: `logs/exp-validator-<i>-latest.log`
- Load generator logs: `logs/load-generator-latest.log`

---
