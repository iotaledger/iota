# Pruning redesign: execution-driven, chain-time paced

Status: implemented on branch `fix/smooth-pruning` (see `PruningCoordinator` in
`authority_store_pruner.rs`, the executor nudge/leash in
`checkpoints/checkpoint_executor/mod.rs`, and the config cleanup in
`iota-config/src/node.rs`).

### Behavioral note: leash throttles on slowness, fails open on errors

The leash makes execution wait on the pruner's progress, where "progress" is the
executed position the pruner has drained up to (see §3). If pruning is merely slow
(IO-bound), the drain still completes fully each pass — it just takes longer — and
the leash holds execution within `SLACK` of the drain's start, so disk stays
bounded (`≤ window + SLACK`) and the node self-throttles. If pruning _persistently
errors_ (a bug, a corrupt store), the frontier still advances (it tracks the
executed position, published regardless of drain success), so execution keeps
going and the database grows — i.e. it **fails open**, with errors logged loudly.
This is deliberate: an earlier design tied the frontier to the pruned watermark so
a stuck pruner would halt execution, but that deadlocked on far-behind nodes (see
§3), and fail-open is the safer choice for liveness. If bounded disk on prune
failure is ever required, gate it explicitly (e.g. halt only after N consecutive
errors) rather than by coupling the frontier to pruned-watermark progress.

## Context

A full node accumulated far more on-disk state than expected while syncing
(observed on testnet: ~840 GB perpetual store, ~400 GB checkpoints store), even
with pruning correctly configured.

Two problems, addressed in two steps:

1. **Unbounded backlog during catch-up (already fixed on this branch, commit
   `fix(core): use on-chain timestamps for smooth pruning`).** Pruning was
   smoothed against _wall-clock_ time: the prune watermark advanced by
   `(max_eligible - pruned) / (epoch_duration_ms / tick_ms)` per tick, leaving a
   backlog of `≈ num_intervals × eligible-growth-per-tick`. During catch-up,
   execution races thousands of checkpoints ahead per tick, so the backlog — and
   disk — ballooned. That was replaced with a **chain-time cutoff**:

   ```
   cutoff_ts = highest_executed_checkpoint.timestamp_ms
             - num_epochs_to_retain * epoch_duration_ms
   prune while checkpoint.timestamp_ms <= cutoff_ts
   ```

   Checkpoint timestamps are consensus-agreed and monotonic, so the cutoff lands
   on the same checkpoint whether history is replayed in an hour or lived at tip.
   The backlog is bounded to one retention window regardless of sync speed.

2. **Execution stalls during pruning bursts (this redesign).** With the cutoff
   fix live, syncing no longer grows unbounded, but the background pruner (a 10s
   timer) accumulates ~10s of backlog and then flushes it in one burst. During
   each burst, execution drops to **0 tx/s** (confirmed in logs: `executed +0`
   in lockstep with `checkpoints/objects pruned +~10k`, resuming the moment
   pruning goes idle).

### Why execution stalls during a pruning burst

Verified in code:

- **Not an app-level lock.** The pruner takes no `execution_lock` / shared
  `RwLock`/`Mutex` (only a private `Mutex` for the compaction bookkeeping).
- **Not RocksDB write-stall backpressure.** Those are deliberately disabled:
  `set_level_zero_stop_writes_trigger(i32::MAX)` and
  `set_soft/hard_pending_compaction_bytes_limit(0)` in
  `typed-store/src/rocks/options.rs`.
- **Not tokio worker starvation.** The pruner yields between batches and the
  executor writes via `spawn_blocking` (separate pool).

It is **shared storage IO + block-cache contention**. The perpetual store is a
single RocksDB instance; execution and pruning hit the same column families
(`objects`, `effects`, `transactions`, `events`). A pruning burst saturates the
disk three ways at once — cold reads (scanning old checkpoint contents/effects),
tombstone writes, and the compaction those tombstones immediately trigger on the
same CFs execution must read — so execution's object reads miss cache, hit a
saturated disk, and go to ~0 until the burst ends.

## Key insight: pruning is inherently rate-matched to execution

With a chain-time window, when execution advances one checkpoint,
`highest_executed.timestamp` moves forward by δ (the chain-time gap to the next
checkpoint) and the cutoff slides by the same δ — exposing exactly the
checkpoints produced in that δ. So **~one checkpoint's worth of data ages out per
checkpoint executed**, at tip _and_ during catch-up (during catch-up you execute
fast in wall-clock, but per executed checkpoint the cutoff still advances by only
one historical inter-checkpoint gap).

Therefore pruning a small, roughly constant amount **per executed checkpoint**
does maximally-even load spreading, fits inside the per-checkpoint slack, and
never bursts — the opposite of the timer, which batches a burst that can exceed
the slack and make the node fall behind the network.

## Design

### 1. Trigger: per executed checkpoint, not on a timer

After each checkpoint is executed **and made available** (watermark bumped;
state-sync / gRPC subscribers notified; validator consensus / checkpoint-cert /
propagation duties done), the executor sends a non-blocking **nudge** to a single
lightweight pruner task. Uniform for validators and full nodes — no role
branching. No timer.

Placement is load-bearing: pruning aged-out data is unrelated to serving the
_current_ checkpoint, so it must run strictly **after** availability/propagation.
It then only consumes the idle slack before the next checkpoint; it never delays
checkpoint propagation to peers or API clients.

### 2. Action: full drain to the cutoff, no per-run cap

On nudge, the pruner drains forward to the chain-time cutoff
(`highest_executed.timestamp - window`) — the existing drain loop, unchanged. In
steady state each run removes ~one checkpoint's worth.

**No per-run size cap.** A fixed "max per run" would be a rate limiter: if
arrival rate exceeds it under sustained load, the backlog grows unbounded and the
DB silently blows up. Full-drain is self-correcting and cannot fall behind by
construction. (The existing `max_*_in_batch` values are only `WriteBatch` flush
sizes for memory bounding — they do **not** cap total work per run — and become
internal constants, removed from operator config so they can't be mistaken for a
rate limit.)

### 3. Backpressure: the leash

Because the pruner now runs concurrently (nudged, not inline), execution could
outrun it under disk saturation — nothing guarantees the pruner wins a fair share
of IO. To preserve the "cannot grow unbounded" guarantee, **leash execution**:

```
frontier      = highest_executed timestamp the pruner caught up to on its last
                completed drain
throttle execution while:
    executing_checkpoint.timestamp_ms - frontier  >  SLACK          (SLACK = 1h)
```

The frontier is the **executed position the pruner has drained up to**, _not_
`pruned_ts + window`. The earlier `pruned_ts + window` formulation deadlocked: it
only equals the executed timestamp when the retention window in checkpoints lines
up exactly, but the drain is also bounded by the epoch guard (can't prune the last
`num_epochs_to_retain` epochs) and `window` uses the _current_ `epoch_duration_ms`
— which does not match the actual durations of the **historical** epochs a
far-behind node replays. When historical epochs are longer than the current epoch
duration, `pruned_ts + window` sits permanently below `executed - SLACK`, the
leash never opens, execution stops nudging, the nudge-driven pruner sleeps, and
the frontier never advances. Deadlock.

Publishing the executed timestamp the pruner has caught up to instead:

- Makes the leash measure **how far execution has run ahead of the pruner's last
  completed drain** — the true backpressure signal — independent of epoch-duration
  variance and the epoch guard.
- Cannot deadlock: each completed drain lifts the frontier to a real executed
  position, and nudges buffered during a drain trigger the next drain immediately,
  so the frontier keeps advancing as execution advances.
- Still bounds disk: the pruner drains to `executed - window` each pass, and the
  leash bounds `executed - frontier ≤ SLACK`, so the retained span stays
  `≤ window + SLACK`.
- `SLACK = 1h` chain-time, hardcoded. It is **not** a cap on prune work (the pruner
  still fully drains); it only _slows execution_ when pruning can't keep up. Chain-
  time (density-invariant) and absolute (sized to burst duration, not the window),
  so it effectively never trips in normal operation and, under sustained overload,
  stabilizes the DB at `window + 1h` — ~2% over a multi-day window.
- If no pruner is enabled, the frontier stays `u64::MAX` and execution is never
  leashed.

### 4. Config surface

- **Remove** (from operator config): the pruning timer / `pruning_run_delay_seconds`,
  the batch-size knobs (`max_checkpoints_in_batch`, `max_transactions_in_batch`)
  — kept as internal constants — and `smooth` (already removed on this branch).
- **Keep**: `num_epochs_to_retain` and `num_epochs_to_retain_for_checkpoints` —
  these define the retention window / cutoff.
- **No new knobs.** Per-checkpoint cadence and the 1h leash are fixed.
  Deserialization does not deny unknown fields, so existing `fullnode.yaml` files
  that still set the removed options continue to load (ignored).

## Correctness / boundedness argument

- **Cannot grow unbounded.** Each drain targets the chain-time cutoff, and the
  leash forces execution to yield if the oldest unpruned data exceeds
  `window + 1h`. Under any load, on-disk retained span ≤ `window + 1h`.
- **Cannot be misconfigured into breakage.** No per-run cap and no pacing knob
  exist to set too low; the only operator knobs are the retention window itself.
- **Self-throttling under overload.** If the disk cannot sustain insert + delete,
  the leash slows execution to the pruner's rate rather than letting the DB grow.
- **Startup / post-downtime backlog** is drained as a one-time catch-up while the
  leash holds execution near the frontier; bounded and temporary.

## Follow-ups

- Bound `certified_checkpoints` (the likely bulk of the 400 GB checkpoints
  folder), which is currently never pruned. Independent of the leash — the leash
  no longer reads pruned-checkpoint timestamps (it tracks the executed position).

## Implementation

- `iota-core/src/authority/authority_store_pruner.rs`
  - `setup_pruning` runs a nudge-driven loop (`watch` of highest-executed): on each
    nudge, run object + checkpoint (+ index) drains to the chain-time cutoff via
    the existing `prune_*_for_eligible_epochs` logic, then publish the frontier =
    the highest-executed timestamp read at the start of the drain.
  - `max_checkpoints_in_batch` / `max_transactions_in_batch` are module constants.
  - Pruning failures are non-fatal (logged); the frontier still advances, so a
    persistent error fails open (see the behavioral note above).
- Checkpoint executor (`checkpoints/checkpoint_executor/mod.rs`)
  - After a checkpoint is executed and made available, nudge the pruner.
  - Before scheduling a checkpoint, `await_leash(checkpoint.timestamp_ms)`.
- `iota-config/src/node.rs`
  - Removed `pruning_run_delay_seconds`, `max_checkpoints_in_batch`,
    `max_transactions_in_batch` from `AuthorityStorePruningConfig`.
- `docs/content/operator/common/pruning.mdx` — removed knobs.

## Test plan

- Unit: cutoff drain already covered on this branch. Add: nudge triggers a drain;
  leash math (`window + SLACK`) trips at the right boundary and releases after the
  pruner advances; leash tracks the furthest-behind of the two watermarks.
- Behavioral (sim / manual on a live node): under catch-up, execution no longer
  flatlines during pruning; `highest_pruned` tracks `highest_executed` within
  ~`window` (+ ≤1h under bursts); disk stays bounded at ≈ `window + 1h` under
  sustained overload.
- `cargo ci-clippy`, `cargo +nightly fmt`, `IOTA_SKIP_SIMTESTS=1 cargo nextest
  run -p iota-core authority_store_pruner`.
