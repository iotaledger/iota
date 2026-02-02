# net_fuzz experiments

This directory documents the long-running experiments in
`net_fuzz.experiments`. These scenarios are intentionally heavier than the
core library primitives and are designed for multi-minute stress runs.

All commands below assume:

- the private network is running (e.g. `./run.sh -n 10 -p mysticeti`)
- a virtual environment is active (`source .venv/bin/activate`)
- the `PYTHON` environment variable is set to the current interpreter: `export PYTHON=$(python -c 'import sys; print(sys.executable)')`
- `net_fuzz` is installed (`pip install -e fuzzer`)

Experiment runs automatically write logs under
`dev-tools/iota-private-network/experiments/logs/` with timestamped
filenames. Each run writes:

- `<experiment>-<timestamp>.log` for the experiment itself.
- `<experiment>-<timestamp>-validators/` containing per-validator logs:
  `validator-<n>-latest.log` is refreshed periodically and
  `validator-<n>-final.log` is captured on shutdown.

## block_stress

Purpose: enforce a symmetric topology where each node blocks f peers in a
3f+1 network. This stresses routing across medium-distance peers while
validators remain connected.

Behavior:

- for n = 3f + 1 validators, blocks f peers per node
- if f is even: blocks f/2 neighbors on each side
- if f is odd: blocks (f-1)/2 neighbors per side plus the antipode
- applies high latency to the blocked edges (no iptables DROP rules)
- applies stable random latency on all other edges
- ramps the block latency over time
- runs a background spammer at 150 TPS

Run:

```bash
sudo -E "$PYTHON" -m net_fuzz.experiments.block_stress
```

## mirage_stress

Purpose: create a “mirage” network where links look fast on average but are
unstable due to high jitter.

Behavior:

- applies low base latency with high jitter on every edge
- increases jitter over time
- runs a background spammer at 100 TPS

Run:

```bash
sudo -E "$PYTHON" -m net_fuzz.experiments.mirage_stress
```

## non_triangle_stress

Purpose: enforce a three-group topology that violates triangle inequality
assumptions to stress gossip and synchronization paths.

Behavior:

- partitions `n = 3f + 1` validators into clusters of sizes `f`, `f`,
  and `f + 1`
- applies slow+lossy intra-group links and faster inter-group links
- updates latencies every minute for 5 minutes
- runs a background spammer at 100 TPS

Run:

```bash
sudo -E "$PYTHON" -m net_fuzz.experiments.non_triangle_stress
```

## sync_stress

Purpose: stress synchronization by cycling restarts between core and outsider
validators while applying asymmetric latencies.

Behavior:

- uses `n = 3f + 1` validators with a core of size `2f + 1` and outsiders `f`
- core validators have low mutual latency; outsiders have higher latencies to everyone
- cycles outages between outsiders and a core subset of size `f` while keeping
  only `2f + 1` validators online
- after each restart, waits for online validators to converge and advance by
  10 checkpoints (or the best available progress metric)
- runs a background spammer at 100 TPS

Run:

```bash
sudo -E "$PYTHON" -m net_fuzz.experiments.sync_stress
```

## Protocol comparison runners

The `run_*_stress.py` scripts orchestrate full end-to-end runs for both
consensus protocols (Mysticeti and Starfish) under the same conditions. Each
runner:

- cleans up any existing network
- bootstraps a fresh validator set
- starts the network for a specific protocol
- runs the corresponding experiment
- repeats for the second protocol
- auto-detects the current validator count from running containers (falls back
  to 10 if none are running)

These scripts live alongside this README:

- `run_block_stress.py`
- `run_mirage_stress.py`
- `run_non_triangle_stress.py`
- `run_sync_stress.py`

Example:

```bash
python dev-tools/iota-private-network/fuzzer/src/net_fuzz/experiments/run_block_stress.py
```

Use `--skip-build` to avoid rebuilding Docker images:

```bash
python dev-tools/iota-private-network/fuzzer/src/net_fuzz/experiments/run_block_stress.py --skip-build
```
