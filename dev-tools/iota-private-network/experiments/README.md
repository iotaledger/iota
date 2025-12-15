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
4. Runs grafana (available at `http:://localhost:3030/dashboards`)
5. Applies network latencies and controlled disruptions (packet loss, connection blocking, validator restarts).
6. Periodically collects logs and saves them with timestamps.

Supports the following flags:

- `-n <NUM>`: number of validators (default: `4`; any number between `4` and `30` is supported)
- `-p <protocol>`: consensus protocol (default: `starfish`; another option: `mysticeti`)
- `-b <true|false>`: rebuild Docker images before running (default: `true`)
- `-g <true|false>`: enable geodistributed large network latencies (default: `false`)
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
# Run default 4-validator Starfish network with small latencies without any additional disruptions
./run-all-benchmark.sh

# Run 10-validator Mysticeti network with large geodistributed latencies for one hour without rebuilding images
./run-all-benchmark.sh -n 10 -p mysticeti -g true -b false

# Run 30-validator Starfish network with geodistributed latencies, 10% blocked connections, 5% chances for packet loss, 10% for restarts and running for 2 hours
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

This will load the default spammer with a TPS of 500 using Starfish (default protocol).

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
./run-all-benchmark.sh -n 4 -p mysticeti -S true -T 100 -Z 10KiB
```

This will launch the spammer from the external repository with the configured transaction rate, TPS=100, and size, 10KiB.

To use Mysticeti instead of Starfish, add `-p mysticeti`:

```bash
./run-all-benchmark.sh -n 4 -p mysticeti -S true -T 100 -Z 10KiB
```

## Main Fuzz Script: `run-all-fuzz.sh`

`run-all-fuzz.sh` automates the full workflow:

1. Optionally rebuilds the `iota-node`, `iota-tools`, and `iota-indexer` Docker images.
2. Bootstraps the validator network.
3. Runs the private network with the chosen consensus protocol.
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

- `-p <protocol>`\
  Consensus protocol (default: `starfish`; other option: `mysticeti`).

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
  -p starfish \
  -b false \
  -t true \
  -d 3600
```

Here `-t true` maps to `geo-high`.

### 3. 19-validator Starfish, geo-high RTTs, 10% blocked pairs, 5% loss, 10% restarts, 2-hour run

```
./run-all-fuzz.sh \
  -n 19 \
  -p starfish \
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
  -p starfish \
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
   Uses the `iota-tools` stress binary inside a Docker container (`iotaledger/iota-tools`) to send transactions against `fullnode-1`.

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
     -p mysticeti \
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

## Batch Comparison Script: `dual-run.sh`

`dual-run.sh` automates **paired runs** of Mysticeti and Starfish under identical network conditions for direct comparison.

It:

- Builds all Docker images once (`iota-node`, `iota-tools`, `iota-indexer`)
- Uses the same fuzz parameters for both protocols
- Runs several steps defined by parameter lists:

  ```bash
  R_LIST=(25 26 33 33)   # % restarts
  X_LIST=(10 15 10 10)   # % blocked pairs
  L_LIST=(10 15 10 10)   # % packet loss
  ```

Each step executes:

1. Mysticeti run with `(r, x, l)`
2. short pause
3. Starfish run with identical `(r, x, l)`
4. pause before the next step

Global defaults (inside the script):

```bash
NUM_VALIDATORS=10
TOPOLOGY="ring"
DURATION=3600
SPAMMER=true
SPAMMER_TPS=100
FUZZ_ROUND_SPAN=300
HEAL_EVERY_ROUND=2
```

Run from `experiments/`:

```bash
./dual-run.sh
```

This produces a sequence of alternating Mysticeti/Starfish experiments using the same disruption settings.

---
