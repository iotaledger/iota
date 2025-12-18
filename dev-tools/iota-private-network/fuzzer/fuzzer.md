# net_fuzz — package rationale and design

This document explains what the `net_fuzz` package does, why it exists,
and how to extend it. It is aimed at engineers who want to build or run
repeatable network fault experiments for the IOTA private network.

## Why this package exists

The legacy bash-based experiments are useful for ad-hoc runs but are
difficult to scale or reason about:

- **Non-deterministic runs**: shell scripts rarely capture seeds and
  configuration in a structured way, making results hard to reproduce.
- **Poor composability**: reusing logic across experiments is difficult,
  leading to code duplication.
- **Fragile error handling**: shell pipelines often mask failures, making recovery logic brittle.

`net_fuzz` replaces these scripts with a **deterministic, programmable**
Python package that can be imported, scripted, and tested.

## What `net_fuzz` provides

At its core, `net_fuzz` is a small library of **building blocks** for
network disruption and verification. These primitives can be composed
into larger experiments without rewriting orchestration logic.

The package exposes:

- **Disruption primitives** (`net_fuzz.disruptions`): latency/loss, link
  blocking, node restart/kill, and full network reset.
- **Verification helpers** (`net_fuzz.checks`): confirm that the intended
  network manipulations actually took effect.
- **Docker boundary** (`net_fuzz.docker_env`): a single layer that owns
  all Docker and shell interactions to keep the rest of the code clean.
- **Scenario runner** (`net_fuzz.scenarios`): a CLI entry point for
  repeatable short scenarios.
- **Long experiments** (`net_fuzz.experiments`): multi-minute to multi-hour tests with
  consistent logging and protocol comparison runners.

This split keeps each layer small and testable.

## Design principles

1. **Determinism by default**\
   Experiments should accept explicit parameters and seeds. The same
   inputs should produce the same topology and timing decisions.

2. **Idempotent operations**\
   Disruptions are applied in a way that can be safely re-run (e.g.
   repeated latency enforcement or repeated resets).

3. **Single responsibility**\
   Disruptions apply faults; checks verify them; experiment scripts
   orchestrate order and timing. This makes failures easier to debug.

4. **Explicit cleanup**\
   Each experiment is responsible for restoring the network (resetting
   `tc`, removing iptables rules, restarting validators).

5. **Measurable outcomes**\
   Experiments log progress, durations, and metrics, so results are
   comparable across runs and protocols.

## How this replaces bash scripts

The end-state is for Python scripts to be the source of truth for
network experiments. Shell scripts should only remain as thin wrappers around Python runs (if needed) or be removed entirely.

Benefits:

- repeatable runs with a fixed seed
- stronger validation of preconditions
- robust recovery/cleanup logic
- structured logging and metrics capture

## Extending the package

When adding a new experiment:

1. Build it using primitives from `net_fuzz.disruptions` and
   `net_fuzz.checks`.
2. Log parameters at the start of the run.
3. Keep the experiment deterministic by seeding random choices.
4. Write logs into the standard experiments log folder.
5. Clean up network state in `finally` blocks.

For shorter workflows, add a scenario to `net_fuzz.scenarios` and wire it
into the CLI. For longer runs, place the script in
`net_fuzz.experiments` and document it in
`dev-tools/iota-private-network/fuzzer/src/net_fuzz/experiments/README.md`.

## Where to start

- Local usage guide: `dev-tools/iota-private-network/fuzzer/README.md`
- Experiments catalogue: `dev-tools/iota-private-network/fuzzer/src/net_fuzz/experiments/README.md`

## Why these longer experiments exist

The long-running experiments are meant as “reference tests” for the most
important failure modes we care about: partial partitions, unstable
latency, skewed topologies, and rolling outages. They are not random
fuzzers; each one encodes a specific story about how the network can go
wrong and what we want to learn from it.

### Block stress

Block stress models an operator or network adversary that does not cut
links completely, but makes a minority of paths _much_ slower than the
rest. We work with `n = 3f + 1` validators and, for every node, pick
exactly `f` peers that are treated as “blocked”. Traffic on those edges
is not dropped; instead a large fixed latency is applied. All remaining
edges get a moderate, randomly chosen latency that stays stable for the
whole run.

Under this topology proposals and votes can still flow, but the cheapest
paths shift away from the “blocked” peers. With a background spammer
driving load, we observe how each protocol behaves as we ramp the
block-latency from “annoying” to “almost broken”: do timeouts explode,
does throughput collapse, does finality time degrade smoothly or hit a
cliff? Because the pattern is symmetric and parameterized, we can repeat
the run later and compare improvements in a meaningful way.

### Mirage stress

Mirage stress asks: what happens when links _look_ fast on average but
are actually unstable? In this scenario every validator pair gets the
same low base latency, but we progressively increase jitter so that the
instantaneous delay can swing widely around that mean.

This creates a “mirage” for any component that samples latency: the
network seems healthy when averaged over time, yet individual rounds see
outliers that break pipelining and make timeouts harder to tune. By
increasing jitter in stages, we can see when each protocol’s control
logic starts to struggle, and whether changes to timeout heuristics or
batching improve robustness in this regime.

### Non-triangle stress

Non-triangle stress is about violating triangle inequality at the
network level. We split `n = 3f + 1` validators into three groups of
sizes `f`, `f`, and `f + 1`. Within each group we deliberately make
links slower and lossier; between groups we make links faster (but not
perfect).

This construction creates situations where going “via another group” can
be faster than talking to a neighbour in your own group, even though
that is counterintuitive. Many routing and gossip strategies implicitly
assume that local peers are preferable; this scenario probes whether
those assumptions leak into consensus performance. Over a short schedule
we change intra- and inter-group latencies and watch how quickly the
system adapts.

### Sync stress

Sync stress focuses on recovery time when the system operates with only
`2f + 1` nodes online. We keep a core of `2f + 1` validators and an
“outsider” set of size `f`. At any point in time, only the core plus one
side of this split is live; the remaining `f` nodes are intentionally
offline.

In each iteration we first run with outsiders down, then swap the
outage: we take `f` core validators offline and bring the outsiders
online. After the swap we watch the executed checkpoint sequence across
all online validators and measure how long it takes until they are both
aligned (small spread) and have advanced by a fixed number of
checkpoints. This produces a concrete “time-to-resync” measurement under
rolling outages, which we can track across releases.

## Protocol comparison runners

For each of these stress scenarios there is a corresponding
`run_*_stress.py` script. These runners are thin orchestration layers
that:

- bootstrap a fresh validator network with a chosen protocol,
- execute the Python experiment,
- tear the network down, then repeat for the second protocol.

Because the topology, timing, and logging are controlled from Python,
these runs are suitable as future simtests: we can attach thresholds
like “median resync time must be below X seconds” or “block stress must
complete without safety violations” and use them as quantitative
regression tests when we evolve Mysticeti, Starfish, or other protocols.
