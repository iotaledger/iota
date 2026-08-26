# Calibration sweeps

Stage 1 data collection for the multidimensional gas metering calibration:
sweep one workload knob at a time, several runs per point, and collect the
per-transaction `{tx_digest, measured_ns, profile}` rows that the benchmark's
`--profile-output` flag writes.

## Usage

The scripts drive the `calibrate` binary — a dedicated entry point over the
same validator/workload substrate as `iota-single-node-benchmark`, exposing
only the calibration surface (sequential execution-only capture by default,
sustained mode via `--duration-secs`; signing always skipped). Two invariants
of that substrate are pinned by unit tests in the crate (`src/tests.rs`): the
measured window opens before input-object loading, and setup transactions
never reach `--profile-output`.

```sh
# timing data is only meaningful from a release build
cargo build --release -p iota-single-node-benchmark --bin calibrate

# full Stage 1 sweep set (defaults: 5 runs x 100 txs per point)
./sweep.py --out ~/calibration-data/$(date +%Y%m%d-%H%M%S)-macbook

# plumbing check (3 values, 2 runs, 20 txs per point)
./sweep.py --out /tmp/sweep-check --quick

# one sweep, recompute summaries without re-running
./sweep.py --out DIR --sweeps interpreter
./sweep.py --out DIR --summarize-only
```

## Dataset layout

```
<out>/manifest.json                       machine, git commit + dirty flag, rustc,
                                          binary sha256 — the reproducibility record
<out>/<sweep>/<knob>=<value>/run-<i>.jsonl  raw rows (first line: run metadata)
<out>/summary.jsonl                       one row per sweep point: median measured_ns
                                          (median of per-run medians), p10/p90, and
                                          the median of every profile counter
<out>/slopes.json                         least-squares slope of measured_ns on the
                                          swept counter, per sweep — quick-look
                                          coefficient estimates, not the real fit
```

Existing non-empty run files are skipped, so an interrupted sweep resumes.

## Sweeps

| Sweep | Knob | Drives | Stage 1 row |
|---|---|---|---|
| interpreter | `--computation` | instructions + stack flow (conflated) | interpreter components |
| reads-runtime | `--num-dynamic-fields` | child-object reads during execution | read, warm |
| reads-input | `--num-transfers` | input objects loaded before execution | read, warm |
| writes-count | `--num-mints` | objects written | write path |
| writes-bytes | `--nft-size` (8 mints) | bytes written | write path |

All reads are **warm** in this setup (fresh store, state created in-process);
the cold-read rig needs the sustained-run work — see the Phase 2 section of
the plan. The real coefficient fit (non-negative least squares over all
counters jointly) is a later step; `slopes.json` exists to sanity-check each
sweep's signal, not to ship constants.

### Native crypto family coverage

Every native crypto family reachable from user Move code has a sweep: the
hashes and hmac, ed25519, both BLS12-381 verifies, secp256k1 and secp256r1
(fixtures signed over sha256, matching the hash selector the loops pass),
ECVRF (fixture generated at setup with the same fastcrypto the native uses;
alpha-string size is a knob), Groth16 on both curves (known-good proofs
copied from the framework's own unit tests — proof generation needs a
circuit, so these cannot be generated at setup), poseidon, and the
`0x2::group_ops` operations (add, scalar mul, hash-to-curve, pairing — no
fixtures needed, elements are built from generators in Move).

Two families are deliberately absent: **zklogin** — its Move-facing
functions (`check_zklogin_id`, `check_zklogin_issuer`) are disabled in the
framework (`EFunctionDisabled`), so no user transaction can reach those
natives; and **VDF** — feature-flagged off outside devnet chains, like
poseidon and group-ops MSM. Poseidon is included despite its flag (the
benchmark chain enables it and the coefficient is ready for the flip); MSM
and VDF can follow the same pattern when they matter.

## On the reference machine

The script is stdlib-only Python 3. Record-keeping to do around each session:
fixed CPU governor, turbo/SMT state, dedicated NVMe — plus a `fio` and CPU
microbenchmark baseline. `manifest.json` captures what it can automatically
(CPU model, memory, governor on Linux, commit, binary hash).

## Cold reads (`cold_read.py`)

Cold reads are measured by a standalone store-level microbenchmark
(`cold_read_bench`), not through transaction execution: the integrated sweeps
measure the warm in-execution read cost, and the cold coefficient composes as
that plus the cold-minus-warm fetch delta measured here.

```sh
cargo build --release -p iota-single-node-benchmark --bin cold_read_bench

# macOS development machine
./cold_read.py --out DIR --purge-cmd "sudo purge"

# Linux reference machine
./cold_read.py --out DIR --purge-cmd "sync && echo 3 | sudo tee /proc/sys/vm/drop_caches"

# plumbing check (2 sizes, 64 MiB stores, no purge)
./cold_read.py --out DIR --quick
```

Per object size, a store is populated once (`--db-bytes`, default 2 GiB)
and measured in fresh processes. The objects table's block cache defaults to
5 GiB in production — larger than these stores — so the runner pins it small
for the measure process (`--block-cache-mb`, default 128; a cold read's cost
does not depend on the size of the cache it missed). Object IDs are
regenerated from the seed rather than scanned, so nothing warms the caches
before the cold pass. Every run reads the same sample cold then warm —
its own contrast — and the summary flags points whose cold/warm ratio
suggests the page cache stayed warm. The first size point also stores the
real framework packages and times their fetch + module deserialization
(bytecode verification is not yet included).

## Fitting (`fit.py`)

The joint fit over one or more sweep datasets — the real coefficients, as
opposed to `slopes.json`'s single-knob quick looks:

```sh
./fit.py --data DATASET_DIR [MORE_DIRS ...] --out calibration-artifact.json
```

Non-negative least squares of `measured_ns` on every profile counter, fit
on 80% of the transactions. Native cost enters as two columns per native
function — its charged gas and its call count — because (calls, gas) spans
the same space as (per-call cost, per-byte cost). Both a per-module gas
column and a per-function gas column were tried first and under-predicted
the expensive cases by an order of magnitude: real per-call time varies
far more within a module than the charged gas does (a `group_ops` pairing
is ~18x a G1 add), and a function's per-byte gas rate can be
disproportionate to its per-call rate relative to real time (`ecvrf`).

Why this regression: measured time is linear in the counters by construction
— every counted operation adds its own time — so each coefficient reads
directly as that operation's cost in nanoseconds. The non-negativity
constraint is the point, not a detail: costs cannot be negative, and
unconstrained least squares splits collinear counters into large cancelling
positive/negative pairs that predict well while shipping a nonsense constant
an adversarial transaction shape could ride. The solver is the Lawson-Hanson
active-set method, run on the Gram matrix (one pass over the rows; columns
scaled to unit maximum for conditioning; deterministic given the data and
holdout seed). NNLS zeroes redundant counters — that is model selection, not
attribution — so separability is verified independently by the VIF check
rather than read off the fit's sparsity. The artifact records: the fitted ns-per-unit coefficients and
intercept; the safety multiplier `m` (smallest value reaching 99% coverage
on the held-out 20%, with the p95 overestimate it costs); the variance
inflation factors of the three interpreter components against the
separability threshold — when they are not separable, ship one combined
interpreter coefficient; each sweep's slope anchor next to the fitted
coefficient for the same counter; per-sweep holdout residuals (which shapes
the model mispredicts); and the source datasets' machine manifests. Stdlib
only, deterministic given the same inputs and holdout seed.

## Write side (`write_side.py`)

Stage 2 data collection: sustained rounds of a write-heavy workload committed
through the real store, with RocksDB write stalls enabled (they are disabled
by default in the test store — the stall onset is the signal `B` is defined
against).

```sh
./write_side.py --out DIR --quick          # 30 s plumbing check
./write_side.py --out DIR --duration 14400 # real run: hours to steady state
```

The benchmark's sustained mode (`--duration-secs` + `--db-path` +
`--enable-write-stall` + `--stats-output`) reuses accounts across rounds and
emits one JSON line per round (generate/execute/commit times, object counts,
store size). The runner derives: throughput and commit-latency trend
(first-third vs. last-third windows), stall/stop events and cumulative
compaction bytes from the RocksDB LOG, and write amplification once enough
compaction has run. Short runs only measure the burst the memtables absorb —
the summary says so explicitly instead of reporting hollow numbers.

Current limitation: deletion-heavy (tombstone) sustained traffic isn't
supported yet — dynamic-field children are deleted permanently, so the
delete workload can't repeat across rounds. The tombstone constant `w_tomb`
needs a create-then-delete round pattern (follow-up).

## Validation (`validate.py`) and the mixed workload

Stage 3: score a calibration artifact on data it was not trained on, against
the plan's acceptance criteria (coverage >= 99%, p95 overestimate <= ~2x).

```sh
# collect a mixed dataset: shapes interleaved within one run
./validate.py collect --out DIR [--spec mixed-default.json]

# score any artifact against any dataset — per-shape coverage table
./validate.py score --artifact artifact.json --data DIR [DIR ...]
```

The `mixed` workload assigns each account a shape from a weighted JSON spec,
deterministically by address. Two lessons already demonstrated on this
machine: (1) a mixed-trained artifact passes on mixed data but failed
single-shape sweeps at 89.9% coverage — under-predicting a hash family absent
from the mixture — which is why the single-shape check is the gate that
matters; (2) interleaving shapes does not break within-transaction
collinearity: counter pairs that are one physical event counted twice (an
object created and its `object::new` call) stay tied in any workload, and
should be read as one quantity.

Real checkpoint ranges are scored through `iota-replay`'s profile capture:

```sh
cargo build --release -p iota-replay
target/release/iota-replay --rpc-url $FULLNODE ch --start N --end M \
    --max-tasks 1 --profile-output replay-N-M.jsonl
./validate.py score --artifact artifact.json --data replay-N-M.jsonl
```

Each user transaction is executed twice: the first execution may fetch child
objects over the network, so the row's `measured_ns` comes from a second
execution against the by-then-local state — a warm lane-time, comparable
with the benchmark's capture (and subject to the same warm-read caveat: the
shipped predictor prices reads cold). Rows whose re-execution effects
diverge are dropped with a warning; system transactions are skipped
(unmetered, bypass admission). Timings are only meaningful with
`--max-tasks 1`.

## Resident memory (`--rss-output`)

The benchmark records the process's current resident memory right after setup
(the baseline) and the kernel-tracked lifetime peak after the measured phase;
their difference is the phase's memory footprint, the response variable for
the memory scale factors. `sweep.py` requests it for every run and summarizes
the median delta per point; a `peak_before_phase` flag marks readings where
setup dominated the peak (meaning: raise the workload's memory knob).

One measured constraint shapes the memory sweeps: values built byte-by-byte
through the interpreter are gas-capped below what process-level readings can
resolve, so the flat-vector sweep keeps time as its response and the
resident-memory slope comes from the struct tree (cheap to build, heavy in
real memory, held in locals). First reading on the development machine:
about 5.9 resident bytes per abstract locals byte (r² 0.999).

## Running on a remote server

The whole collection is designed to run unattended on a Linux machine and be
copied back for inspection; every dataset carries its own manifest (machine,
CPU, governor/turbo/SMT state, commit, binary hash), so results from several
machines can sit side by side without confusion.

```sh
# on the server, once
git clone <repo> iota && cd iota && git checkout protocol-research/feat/multidimensional-gas-metering
# build prerequisites (Debian/Ubuntu): build-essential clang cmake pkg-config libssl-dev protobuf-compiler
curl https://sh.rustup.rs -sSf | sh            # the toolchain is pinned by rust-toolchain.toml

# every session: run inside tmux so an SSH drop does not kill the collection
tmux new -s calibration
crates/iota-single-node-benchmark/calibration/run_all.sh /data/calibration/$(date +%Y%m%d-%H%M%S)
# add --write-duration 14400 for a four-hour sustained write run
# add --turbo on (default off) for the boosted-clock comparison run — use a separate OUT_DIR

# back on your machine
rsync -az server:/data/calibration/ ~/calibration-data/server/
```

`run_all.sh` records the machine state (`machine_prep.sh`, which also sets
the governor to `performance` and disables turbo/boost when it has root),
builds the release binaries, runs the Stage 1 sweeps, the mixed workload, the
cold reads (dropping the page cache when root or passwordless sudo is
available — without it the cold numbers are lower bounds and the log says
so), the optional sustained write run, then the fit and validation scores.
Every stage is resumable: re-running the same command after an interruption
skips finished runs.

Put `OUT_DIR` on the NVMe the validator would use — the cold-read and
write-side stages measure that disk. Keep the machine otherwise idle; the
manifest's load average and the per-run `measured_ns` spread will show if it
was not. `fio`, if installed, adds a 4k random-read baseline to
`machine-state.txt`.
