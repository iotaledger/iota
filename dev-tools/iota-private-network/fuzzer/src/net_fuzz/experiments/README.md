# net_fuzz experiments

This directory documents the long-running experiments in
`net_fuzz.experiments`. These scenarios are intentionally heavier than the
core library primitives and are designed for multi-minute stress runs.

All commands below assume:

- the private network is running (e.g. `./run.sh -n 10 -p mysticeti`)
- a virtual environment is active (`source .venv/bin/activate`)
- `net_fuzz` is installed (`pip install -e fuzzer`)

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
- groups validators into three clusters (1–3, 4–7, 8–10)
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
- core validators (1–7) have low mutual latency
- outsiders (8–10) have higher latencies to everyone
- cycles stop/restart windows for outsiders and then a core subset
- runs a background spammer at 100 TPS

Run:

```bash
sudo -E "$PYTHON" -m net_fuzz.experiments.sync_stress
```

## adaptive_fuzz

Purpose: hill-climbing search over latency, loss, and topology settings to
find configurations that maximize consensus pain signals.

Behavior:
- searches across core/minority and triangle-violation strategies
- uses consensus metrics as a feedback signal
- writes a CSV log (`fuzz_results.csv`)

Run:

```bash
sudo -E "$PYTHON" -m net_fuzz.experiments.adaptive_fuzz
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
