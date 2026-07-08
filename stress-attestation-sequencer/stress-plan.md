# Stress-test plan: TotalComputationUnits congestion-control mode

> [!NOTE]
> This is the initial plan made with Claude, and it may be revised during
> the course of stress testing. It is aligned with the test branch
> [`protocol-research/feat/transaction-attestation-feature-test`](https://github.com/iotaledger/iota/tree/protocol-research/feat/transaction-attestation-feature-test)
> in the `iota` monorepo, which is sourced from the feature branch
> `protocol-research/feat/transaction-attestation-feature`. Testing needs extra
> features - the metrics and attestor changes in "Validator node changes" -
> that live on the test branch, not the feature branch.

---

PR [#11574](https://github.com/iotaledger/iota/pull/11574) adds a new
congestion-control mode, `TotalComputationUnits`. For shared-object scheduling,
it uses the computation cost from the attestation, in gas units
(`computation_cost / gas_price`). Transactions without an attestation fall back
to `gas_budget / gas_price`, also in gas units.

This plan checks that the new mode does not reduce performance or throughput
compared to the modes used before, and that it stays correct and safe.

---

## Local test setup (scripts and config overrides)

> [!NOTE]
> The local network launch scripts and the `docker-compose` overrides live in
> the `iota` monorepo on branch
> [protocol-research/feat/transaction-attestation-feature-test](https://github.com/iotaledger/iota/tree/protocol-research/feat/transaction-attestation-feature-test).
> They let us set protocol config flags and parameters at runtime, without
> rebuilding Rust code or docker images.

### Setting flags and parameters via `docker-compose.yaml`

`dev-tools/iota-private-network/docker-compose.yaml` forwards a set of
`IOTA_PROTOCOL_CONFIG_OVERRIDE_*` and
`IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_*` variables, from its shared
`common-env` block, to every validator and fullnode. Each node applies them
through the `serde-env` override path, gated by
`IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE`. Feature flags use the
`...FEATURE_FLAGS_OVERRIDE_` prefix; top-level parameters use `...OVERRIDE_`.
The boolean flags and the congestion mode carry defaults that match the max
protocol version, so an unset run reproduces the baseline; the numeric limits
are bare pass-through, so they only override when you set them. Confirm what
a node applied by grepping its log for `has been overridden`.

### Scripts (in `stress-attestation-sequencer/`)

Those are thin wrappers around the `iota-private-network` tooling plus
`grafana-local`. The defaults bring up a network in the attested path and
`TotalComputationUnits` mode; tune per run with environment variables and
arguments (e.g., to test against `TotalGasBudget`).

Per-validator CPU/memory comes from cadvisor, now wired into `grafana-local`
(a compose service plus a `cadvisor:8080` scrape job); host-level node-exporter
is also wired for the whole-machine totals. Restart the `grafana-local` stack
after pulling so the cadvisor container starts and Prometheus loads the job.

- `bootstrap.sh` (needs `sudo`): regenerate genesis and validator configs.
  Arguments: `-n N` (validators, default 4), `-e MS` (epoch), `-b` (benchmark
  gas accounts).
  Example: `sudo stress-attestation-sequencer/bootstrap.sh -b -n 4`.
- `start.sh`: bring the network up, verify every validator applied the
  overrides, then start Prometheus and Grafana. Defaults: attestation and
  white-flag flow on, `TotalComputationUnits` mode, `faucet`, 4 validators.
  Tune via environment variables (`MODE`, `ATTEST`, `PCOOL`,
  `MAX_ACCUMULATED_TXN_COST`, `MAX_CONGESTION_OVERSHOOT`,
  `MAX_DEFERRAL_ROUNDS`) and `run.sh` arguments (e.g., `-n 10 faucet`).
  Example: `MODE=TotalGasBudget stress-attestation-sequencer/start.sh`.
  For W6 and W7, `ATTESTOR_SKEW_PERCENT` poisons one validator's attestor to
  misreport its computation cost (`<100` under-reports for W6, `>100`
  over-reports for W7, unset or `100` is honest), and `ATTESTOR_SKEW_VALIDATOR`
  chooses which one (default `validator-1`). This is not a protocol-config
  override; `start.sh` applies it by generating a `docker-compose.override.yaml`
  for that single service and verifies it landed there. Make sure the chosen
  validator is among those started (`-n`). Example:
  `ATTESTOR_SKEW_PERCENT=50 stress-attestation-sequencer/start.sh`.
- `cleanup.sh` (needs `sudo`): bring monitoring down, then the network down
  and wipe data.
  Example: `sudo stress-attestation-sequencer/cleanup.sh`.
- `restart.sh` (needs `sudo`): run cleanup, then bootstrap, then start, in
  order. `-n`/`-e`/`-b` go to bootstrap (`-n` also to start), modes go to
  start, and `start.sh`'s environment knobs are inherited.
  Example: `sudo stress-attestation-sequencer/restart.sh -b -n 4 faucet`.

### Network size (`-n N` / `N=`)

`N` is the single knob for network size. The scripts take `-n N` (default 4) and
the `h1` matrix takes it as an environment variable, e.g.
`N=72 LABEL=... ./h1/run.sh`, which forwards `-n 72` to bootstrap and start.
`bootstrap.sh` generates `genesis-template-<N>.yaml` and validator configs for
`validator-1 … validator-N`; `run.sh` then starts only those N containers
(`docker compose up -d validator-1 … validator-N`).

`docker-compose.yaml` and `prometheus.yaml` **statically hardcode**
`validator-1 … validator-100` (and the matching `Validator_1 … Validator_100`
scrape jobs). Since start brings up only `1 … N`, the extra blocks are inert —
nothing is regenerated per bootstrap, so no tracked file is churned. When `N` is
below 100, Prometheus lists the unused targets as *down*; that is cosmetic, the
dashboards reduce over the running validators only.

> [!NOTE]
> The hardcoded ceiling is `N = 100` — enough for production scale (≈100
> validators) and far above what one test host can run (CPU/RAM-bound at a few
> dozen). To raise it, hand-edit the `validator-*` blocks in
> `docker-compose.yaml` and the `Validator_*` scrape jobs in `prometheus.yaml`
> (or recover the old `gen-topology.sh` from git history). The addressing limit
> is `190`: validators take static IPs `10.0.0.(10+i)` and the fixed infra sits
> at `10.0.0.201–.209`, leaving `200 − 10 = 190` slots in the `/24` subnet.

---

## Current test framework: what it can and cannot do

> [!NOTE]
> The `iota-benchmark` stress tool now lives in the sibling repository
> [`network-benchmark`](https://github.com/iotaledger/network-benchmark)
> (`crates/iota-benchmark/`), not in this monorepo. It is built against the `iota`
> commit under test and produces the `iotaledger/stress` docker image. The
> workload and knob references below live in that repo.

- The `iota-benchmark` stress tool has two submission paths, and only one
  exercises attestation:
  - Direct to validators (`--local`, or remote with
    `--use-fullnode-for-execution false`): the old certificate-based
    `QuorumDriver` path. It submits certificate-based transactions
    (`ConsensusTransactionKind::CertifiedTransaction`), not `UserTransactionV1`,
    and never enters the certificate-less white-flag and attestation flow.
  - Through a fullnode (`--use-fullnode-for-execution true`) with
    `enable_white_flag_flow` on: the fullnode routes via `TransactionDriver`,
    which submits on the attesting `submit_tx` path. That produces
    certificate-less `UserTransactionV2` (attested) when
    `enable_validator_attestation` is on, or `UserTransactionV1` (unattested)
    when off.
  So to test `TotalComputationUnits` on attested transactions, run the stress
  tool against the fullnode with attestation enabled (`start.sh` sets the
  flags). The kind is set by the submission path (direct vs fullnode) and by
  the `enable_white_flag_flow` and `enable_validator_attestation` flags, not
  by the congestion mode; an unattested run (`UserTransactionV1`, or the
  certificate path) uses the fallback (`gas_budget / gas_price`).
- Available workloads in the `iota-benchmark` stress:
  - `--shared-counter`: increments a shared `Counter` object
    (`call_counter_increment`), so it is a shared-object transaction subject
    to per-object congestion control. Light, fixed cost. Knobs:
    `--shared-counter-hotness-factor`, `--num-shared-counters`,
    `--shared-counter-max-tip-amount`. This is the W1 baseline and the main
    workload for comparing the congestion modes against each other.
  - `--slow`: runs the clock-driven Move call `slow::bimodal` (two hardcoded cost
    levels toggled every 10s) and adds a mutable shared-object input
    specifically to activate congestion control, so it is a shared-object
    transaction with variable / heavy computation. The workload for W4
    (owned-object, used for H1) and W5 (shared-object, for the mode comparisons
    and the scheduling-accuracy check), since the attested cost actually
    varies. (Extended for this plan with configurable knobs — see the
    "Configurable slow mode" entry under workloads added below.)
  - `--adversarial`: generates max-resource / edge-case transactions
    (selectable payload types: large objects / events / runtime vectors / pure
    arguments, dynamic-field reads, max shared-object reads, max package
    publish). The `MaxReads` type takes many shared objects as input, so it
    stresses per-object congestion with high contention; the heavy variants
    produce very expensive transactions useful for stressing the scheduler and
    the overshoot limit. Most variants are owned-object, so treat it as a
    robustness / edge-case workload, not a controlled comparison.
  - `--randomized-transaction`: per-transaction, randomly mixes shared-counter
    increment / read / delete, randomness calls, and owned / pure inputs, with
    a random number of shared inputs. It exercises congestion control, but only
    periodically and with a random per-transaction shape and cost, so it suits
    fuzz / robustness runs, not careful mode comparisons or attributing results.
  - `--expected-failure` is a signature failure, not an execution abort: it only
    implements `InvalidSignature`, which is rejected at signature verification
    (pre-consensus, no gas, no effects), so it never reaches consensus or
    congestion control. It cannot stand in for W3 (a cheap transaction that
    aborts early during execution on a shared object); W3 needs a real aborting
    Move call on a shared object.
  - Other workloads exist but are mostly owned-object, so they do not exercise
    per-object congestion control and are not relevant here.
- Workloads added or to be added in the `iota-benchmark` stress:
  - Configurable slow mode (added): `--slow-n` / `--slow-size` select the fixed
    `slow::slow(n, size)` cost, and `--slow-shared` toggles a shared vs
    owned-object input - giving W5 (shared-object, cost sweep) and W4
    (owned-object, pure computation, used for H1). Previously `--slow` ran only
    the clock-driven `slow::bimodal` with a shared input (two hardcoded cost
    levels, not settable). See the `--slow` entry above.
  - W2 (inflated budget): a gas-budget knob, so a shared-object workload can set
    its gas budget well above its real computation cost (one run per 1x / 10x /
    100x ratio). No such knob exists today (`options.rs` has none) and
    `--shared-counter` hardcodes the budget (`MAX_GAS_FOR_TESTING`), so the
    budget / cost ratio cannot be controlled. This separates
    `TotalComputationUnits` (weighs by attested cost) from `TotalGasBudget`
    (weighs by the inflated budget).
  - W3 (early abort): a cheap shared-object Move call that aborts early during
    execution. No existing workload does this (`--expected-failure` fails at the
    signature check, pre-consensus, and never reaches execution). Needed to
    check that an aborting attested transaction is scheduled and charged by its
    small real cost, not by its budget.

---

## Validator node changes needed for this plan

These changes belong in the validator node software, not the test framework.
They are needed for the testing only, not to be merged to upstream branches.

- Add three metrics - Prometheus histograms on the validator, observed per
  attested transaction after attestation and after execution:
  - `attested_computation_units`: the attestor's pre-consensus estimate in gas
    units (CUs), which is not in the effects and so must be exposed here.
  - `actual_computation_units`: the real cost in gas units (CUs), i.e. the
    effects' `gas_cost_summary().computation_cost` divided by the gas price.
    Derived at record time for Grafana, where effects are not accessible.
  - `actual_to_attested_computation_units_ratio`: `actual / attested`, computed
    per transaction at record time, after the execution. It cannot be derived
    later from the other two: each is an aggregated histogram, so the
    per-transaction pairing is lost, and dividing the aggregates gives the
    wrong answer.
  These metrics (with slightly different names) were added in commit
  [3357d7d1d0](https://github.com/iotaledger/iota/pull/11574/commits/3357d7d1d069d1baddf04ece7e0890fde6c57bd7)
  and dropped the next day in
  [471fdd8ab1](https://github.com/iotaledger/iota/pull/11574/commits/471fdd8ab1daa8ad415daeef350742e5ef130989).
  We need to re-add them for the testing.
- Add three attestation-overhead metrics on the validator, to measure the cost
  of the pre-consensus dry-run that attestation performs:
  - `validator_attestation_latency`: a histogram timing the `attest_transaction`
    call (`validator_v2.rs:234`). Nothing times it today, and
    `tx_verification_latency` covers only signature verification. This isolates
    the dry-run cost.
  - `validator_attestations_total`: a counter of attestations performed, to
    normalize the latency by rate (CPU per attestation, attestation rate).
  - `validator_attestation_task_panics`: a counter of dry-run panics. The
    spawned attest task's join-error arm (`validator_v2.rs:245`) only logs today
    (`tracing::error!`); increment a counter there. Robustness signal - counts
    how often the dry-run crashes (e.g., a Move VM arithmetic panic).
  No new metric is needed for end-to-end user latency: the `TransactionDriver`
  already exposes `transaction_driver_settlement_finality_latency` (submit to
  finality) and `transaction_driver_submit_transaction_latency` (submit RPC
  round-trip). They just need to be watched in Grafana. Per-stage CPU is not a
  metric; correlate per-validator container CPU (via cadvisor, now wired into
  grafana-local; plus host-level node-exporter for the whole machine) with
  attestation on/off.
- Split `validator_attestation_latency` into its three parts, since
  `attest_transaction` runs on `tokio::task::spawn_blocking` and the total hides
  where the cost goes:
  - `validator_attestation_queue_wait`: time queued before a blocking-pool
    worker starts the dry-run.
  - `validator_attestation_execution_latency`: the Move-VM dry-run itself.
  - `validator_attestation_async_resume_latency`: time from the dry-run
    finishing on the pool until the waiting async task runs again. Under load
    this is the largest part (full ≈ wait + exec + resume): the blocking-task
    pool grows to many threads that keep every CPU core fully busy, so the fixed
    number of async workers get almost no CPU time, and a finished dry-run waits
    seconds before its task is picked up again. All four attestation histograms
    use the 90s `LATENCY_SEC_BUCKETS`, not the 1s `SUBSECOND` ones, which cuts
    off every value above 1s under load.
- Label pre-consensus overload rejections by source. `check_system_overload`
  rejects a transaction before consensus when the consensus queue is saturated,
  and its `transaction_overload_sources` counter used one `consensus` label for
  every cause. Split it into `consensus_graduated` (graduated soft-limit shed),
  `consensus_max_pending` (`num_inflight` past `max_pending_transactions`), and
  `consensus_semaphore` (submit semaphore out of permits), so it is clear which
  limit was hit. The semaphore (`max_pending_transactions * 2 / committee_size`)
  is reached first and holds `num_inflight` below `max_pending`.
- Add three congestion-control metrics on the validator (the existing ones cover
  deferred/cancelled counts and the max per-object scheduled estimate, but not
  these):
  - `consensus_handler_scheduled_transactions_per_object_per_commit`
    (histogram): how many transactions the scheduler admits to an object per
    consensus commit - the
    per-round output that explains throughput differences and shows limit
    utilization (`consensus_committed_user_transactions` is only the commit-wide
    count, not per object).
  - `consensus_handler_transaction_deferral_rounds` (histogram): how many
    consensus rounds a transaction was deferred before being scheduled or
    cancelled (the gap from its deferral key's `deferred_from_round` to the
    current round). The existing deferred/cancelled counters give how many, not
    how long - this is the latency / starvation signal central to W6 and W7.
- Add a `post_consensus_validation_latency` histogram timing
  `validate_and_resolve_conflicts`. Only the drop count
  (`consensus_handler_validation_dropped_transactions`) is metered today, not
  the time. Worth timing because Check `#3` (attestor verification plus the
  floor/ceiling bounds) is new work added to that path.
  - Caveat: do not read a V1-vs-V2 difference in this metric as attestation
    overhead. Check `#3` is a few integer comparisons and in-memory protocol
    config reads per attested transaction - no crypto (the attestation is
    already verified pre-consensus via the block signature), no database or
    cache access. The pass latency is dominated by Check `#1`
    (`try_is_tx_already_executed`, a cache/database lookup) and the owned
    object conflict resolution and lock acquisition, so any V1-vs-V2 delta is
    buried in that noise. The real attestation overhead is pre-consensus, in
    `validator_attestation_latency` (the dry-run and the async resume it waits
    on - see the split above); post-consensus is effectively free.
- W6 (under-reporting attestor): a validator-side change that makes the
  attestor report a computation cost below the real one, to check that the
  scheduler and the safety limits hold when the estimate is wrong.
- W7 (over-reporting attestor): the same change in the other direction, making
  the attestor report a cost above the real one, to check that the mode does
  not over-defer and that one inflated transaction cannot starve others on a
  hot object.

### Checklist

- [x] Accuracy metrics:
  - [x] `attested_computation_units`
  - [x] `actual_computation_units`
  - [x] `actual_to_attested_computation_units_ratio`
- [x] Attestation-overhead metrics:
  - [x] `validator_attestation_latency` (split into `_queue_wait`,
    `_execution_latency`, `_async_resume_latency`; 90s buckets)
  - [x] `validator_attestations_total`
  - [x] `validator_attestation_task_panics`
- [x] Pre-consensus overload sources labeled by cause (`consensus_graduated` /
  `consensus_max_pending` / `consensus_semaphore`)
- [x] Congestion metrics:
  - [x] `consensus_handler_scheduled_transactions_per_object_per_commit`
  - [x] `consensus_handler_transaction_deferral_rounds`
- [x] `post_consensus_validation_latency` histogram
- [x] `validator_transaction_execution_latency` histogram (receipt -> executed)
- [x] W6 under-reporting attestor (`IOTA_ATTESTOR_COST_SKEW_PERCENT` < 100)
- [x] W7 over-reporting attestor (`IOTA_ATTESTOR_COST_SKEW_PERCENT` > 100)

> [!NOTE]
> A `consensus_handler_max_actual_object_costs` metric (the actual per object
> per commit cost, mirroring the scheduled
> `consensus_handler_max_congestion_control_object_costs`) was considered and
> deliberately left out. The scheduled estimate is computed synchronously in
> the consensus handler, but the actual cost only exists after execution, in
> the execution driver, which runs transactions one at a time with no commit or
> object grouping and no commit-completion signal. Producing a faithful mirror
> would need a new consensus -> transaction-manager -> execution-driver
> accounting path plus per-commit flush/evict logic - too much for the value,
> especially since the per-transaction accuracy metrics
> (`attested_computation_units`, `actual_computation_units`,
> `actual_to_attested_computation_units_ratio`) already capture the estimate vs
> actual gap.

---

## Phase 0: prerequisites (must be done first)

- P0a. Add back the accuracy metrics: `attested_computation_units`,
  `actual_computation_units`, and `actual_to_attested_computation_units_ratio`.
- P0b. Set `max_accumulated_txn_cost_per_object_in_mysticeti_commit` to a value
  that fits the gas-units scale (roughly the `TotalGasBudget` value divided by
  the reference gas price). A wrong limit makes the comparisons meaningless.
- P0c. Run the stress tool against a fullnode with
  `--use-fullnode-for-execution true` and attestation enabled, so transactions
  take the attesting `submit_tx` path and become V2. No change to the load
  generator is needed; `start.sh` sets the flags.

---

## What we want to check/confirm

- H1 - attestation overhead: measure and report what attestation costs, using
  W4 traffic (slow owned-object) run V1 vs V2:
  `validator_attestation_latency`, and the V1->V2 deltas in
  `settlement_finality_latency`, `submit_transaction_latency`, and container
  CPU. Only report the numbers; no pass/fail threshold.
- H2 - new mode vs `TotalTxCount`: measure and report the difference in
  throughput (`transactions_included_in_checkpoint`) and latency between
  `TotalComputationUnits` and `TotalTxCount` (the only mode enabled so far on all
  networks). Only report the numbers; no pass/fail threshold.
- H3 - new mode vs `TotalGasBudget`: with gas budgets much larger than the real
  computation cost, measure and report the difference in shared-object
  throughput (`transactions_included_in_checkpoint`) between
  `TotalComputationUnits` and `TotalGasBudget` (which schedules on the
  inflated budget, while the new mode schedules on the attested computation
  cost). `TotalGasBudget` was never enabled, so this is a relative comparison.
  Also watch execution queue depth and latency, not just admitted TPS. Only
  report the numbers; no pass/fail threshold.
- H4 - safety: no transactions get stuck and no validators fork. This one is
  pass/fail: any occurrence is a failure, not a number to report.
  - Note: H4 caught a real fork on the attested path -
    `check_coin_deny_list_for_attested_tx` dropped transactions on a transient
    post-consensus input-load race, diverging checkpoints. Root-caused and
    fixed (see stress-test.md H4 warning); tracked in iota-private#438.

---

## Workloads

> [!NOTE]
> Only shared-object workloads exercise per-object congestion control.
> Owned-object traffic (e.g., `--transfer-object`) and
> `--randomized-transaction` are filler / fuzz only, not used for the controlled
> comparisons below.

### W1 - shared-counter contention (baseline)

- Network parameters: attestation on, white-flag flow on; one run per
  congestion control mode (`TotalTxCount` and `TotalComputationUnits`);
  per-object cost limit set per mode to the same effective per-object capacity,
  not the same numeric value (10 means 10 transactions under `TotalTxCount`
  but ~10 gas units - below one transaction - under `TotalComputationUnits`);
  see P0b.
- Stress parameters: `--shared-counter` with `--num-shared-counters` (fewer =
  more contention) and `--shared-counter-hotness-factor` (skew toward hot
  objects); raise the submission rate to saturation. Cost is light and fixed.
  Uses the existing workload, no additions needed.
- Measure: throughput, latency (p50/p95/p99), and per-object deferral /
  cancellation rate, per mode.
- Tests: H2 (no regression: `TotalComputationUnits` keeps up with
  `TotalTxCount`). With uniform cost, count and computation cost are
  proportional, so this is a control, not where the new mode wins.

### W2 - inflated budget

- Network parameters: attestation on, white-flag flow on; compare
  `TotalGasBudget` vs `TotalComputationUnits` (the two budget-aware modes);
  per-object cost limit set per mode to the same effective capacity
  (`NANOS`-scale for `TotalGasBudget`, gas-units for `TotalComputationUnits` -
  see P0b), not the same numeric value.
- Stress parameters: a shared-object workload (e.g., shared-counter) with a new
  gas-budget knob; one run per budget / real-cost ratio (1x, 10x, 100x); raise
  the rate to saturation. Needs the gas-budget knob (not in the tool today).
- Measure: shared-object throughput per mode at each inflation.
- Tests: H3 - `TotalComputationUnits` throughput is clearly higher than
  `TotalGasBudget` as the inflation grows, because it schedules on the real
  attested computation cost, not the inflated budget; execution overshoot stays
  bounded.

### W3 - early-abort robustness (not a cost-differentiation test)

- Network parameters: attestation on, white-flag flow on; `TotalComputationUnits`
  mode.
- Stress parameters: a new cheap shared-object Move call that aborts early in
  execution; drive it at a high rate on a hot object. Needs the early-abort call
  (`--expected-failure` cannot serve this; it fails at signature check,
  pre-consensus).
- Note: an aborting transaction is NOT cheaper than a normal one in attested
  computation cost. Gas bucketing rounds computation up to one
  `gas_rounding_step` (1000 units), so an early abort and a normal shared
  counter increment land in the same 1000-unit bucket and pack identically.
  Cost-based differentiation needs a multi-bucket spread of real cost (W5, the
  `--slow` workload), not aborts.
- Measure and test (robustness, not benefit): an aborting dry-run still
  produces a valid attestation; the abort is costed as one bucket (like any
  cheap successful transaction); a stream of aborts schedules and executes
  without breaking the attestation or scheduling path.
- Tests: supports H4 (safety) - aborts must not break attestation or scheduling.

### W4 - slow owned-object transactions (attestation overhead, V1 vs V2)

- Network parameters: white-flag flow on; two runs at the same mode
  (`TotalComputationUnits`): (i) attestation off (all V1, fallback
  `gas_budget / gas_price`) and attestation on (all V2, attested computation
  cost). The two kinds never coexist in a run.
- Stress parameters: `--slow` in owned-object mode (`--slow-shared false`), so each
  transaction only does CPU work with no shared-object congestion control to
  confound the A↔B delta; sweep `--slow-n`/`--slow-size` across the
  computation-cost range; same workload, seed, and rate in both runs.
- Measure: diff scheduling, throughput, and latency between the two runs.
- Tests: H1. The V1 run is the zero-attestation control, so this isolates
  attestation overhead: diff the e2e latency (`settlement_finality_latency`,
  `submit_transaction_latency`), the validator `validator_attestation_latency`,
  and container CPU.

### W5 - slow shared-object transactions

- Network parameters: attestation on, white-flag flow on; one run per mode
  (especially `TotalComputationUnits` vs `TotalTxCount` / `TotalGasBudget`);
  per-object cost limit set per mode to the same effective capacity, not the
  same numeric value (see P0b); watch the overshoot limit.
- Stress parameters: the same configurable `--slow` workload as W4 but in
  shared-object mode (`--slow-shared true`, the default), which attaches a mutable
  shared-object input to activate congestion control; set `--slow-n`/`--slow-size`
  to run the fixed `slow::slow(n, size)` for a controllable per-transaction
  computation cost; sweep `n`/`size` for a real, varying cost spread and raise the
  rate to saturation.
- Note on the workload: W4 (owned) and W5 (shared) are the same `--slow` workload
  with `--slow-shared` toggled. The `--slow-n`/`--slow-size` (fixed, controllable
  cost) and `--slow-shared` (shared vs owned) knobs were added for this plan;
  originally `--slow` only ran `slow::bimodal`, which is clock-driven rather than
  configurable - it toggles every 10s between two hardcoded levels
  (`slow(100, 100)` heavy / `slow(10, 10)` light), so the per-tx cost was limited
  to two discrete points chosen by wall-clock timing. Leaving both `--slow-n` and
  `--slow-size` unset still selects the old bimodal behavior.
- Measure and test: `TotalComputationUnits` should defer these correctly without
  overloading execution; throughput vs the other modes. Best workload for the
  scheduling-accuracy check (attested vs actual ratio near 1.0). This is where
  variable cost makes the new mode behave differently from `TotalTxCount`.
- Tests: H3 (accuracy ratio near 1.0) and the cost-based differentiation behind
  H2/H3. To be run later.

### W6 - under-reporting attestor

- Network parameters: attestation on, white-flag flow on; `TotalComputationUnits`
  mode; per-object cost limit at a normal value (the reference the overshoot is
  measured against); one validator skewed to under-report (a value between the
  minimum computation floor and the real computation cost). Done with the
  `IOTA_ATTESTOR_COST_SKEW_PERCENT` env var, read once per validator process:
  set it on a single validator's container to make that attestor report the
  given percent of the real dry-run cost (`100` or unset = honest, `<100` =
  under-report for W6, `>100` = over-report for W7). Keep the percent high
  enough that the skewed cost stays above the Check `#3` floor
  (`min(base_tx_cost_fixed, gas_rounding_step)`, in gas units), otherwise the
  transaction is dropped before it can under-schedule.
- Stress parameters: a shared-object workload (e.g., W5 or a hot W1) routed to
  the poisoned attestor; raise the rate to push the object.
- Measure and test (safety): an under-reported cost makes the sequencer admit
  more work per object per commit than the limit intends, so report (a) how far
  per-object execution overshoots the limit, and (b) the resulting execution
  delay - execution queue depth, execution latency, and execution falling behind
  consensus (backpressure). Main risk of the mode: the cost is reported by the
  block author and trusted in the sequencer.
- Tests: H4 (safety): the under-report stresses overshoot and overload; H4 is
  also watched on every run.

### W7 - over-reporting attestor

- Network parameters: attestation on, white-flag flow on; `TotalComputationUnits`
  mode; one validator skewed to over-report (above the real cost, up to and
  beyond `gas_budget / gas_price`); `max_deferral_rounds_for_congestion_control`
  at the default 10 (one run per low / default / high only to confirm it bounds
  the delay). Same `IOTA_ATTESTOR_COST_SKEW_PERCENT` knob as W6, set above `100`
  to over-report; a percent that pushes the reported units above
  `gas_budget / gas_price` is dropped by the Check `#3` ceiling, which is itself
  part of what W7 checks.
- Stress parameters: inflated transactions on a hot shared object alongside
  honest co-located transactions; raise the rate.
- Measure and test: unnecessary deferral and lost throughput; whether one
  inflated transaction pushes co-located transactions to defer; whether the
  attested cost is clamped; and whether
  `max_deferral_rounds_for_congestion_control` bounds the damage (bounded delay
  and cancellation, not sustained starvation).
- Tests: H4 (safety): the over-report stresses deferral and starvation; H4 is
  also watched on every run.

---

## Metrics

A dedicated Grafana dashboard collects the metrics below:
`dev-tools/grafana-local/dashboards/attestation-sequencer-stress.json` (title
"Attestation / Sequencer Stress (`TotalComputationUnits`)"). It auto-provisions
when `grafana-local` starts and has a `validator` template variable (the `host`
label) to focus on one validator - use it to watch the poisoned attestor in W6
and W7. Rows: throughput, attestation accuracy, attestation overhead,
sequencing and congestion, validator-internal pipeline latency, queues and
health. Open it at [localhost](http://localhost:3000/d/attestation-sequencer-stress?refresh=auto&from=now-5m&to=now).

- Throughput: finalized TPS via `rate(transactions_included_in_checkpoint[2m])`
  (the primary signal - what congestion control let through and committed; the
  starfish-overview "Current TPS" / "Checkpoint TPS" panels), plus per-validator
  execution rate via `rate(execution_driver_executed_transactions[2m])`
  (validator-dashboard), and the `iota-benchmark` tool's own client-side TPS.
  Report steady and peak, and the saturation point.
- Latency: submit-to-finality (p50, p95, p99) via
  `settlement_finality_latency` and `submit_transaction_latency` (these are
  client/driver-side and include network and fullnode time). For the pure
  validator-internal pipeline latency - from when a validator receives a
  transaction via `submit_tx` until it finishes executing it, spanning the
  pre-consensus check/attestation, consensus, post-consensus validation,
  sequencing (including any congestion deferral), and execution, with no
  client/fullnode time - use `validator_transaction_execution_latency`. It is
  recorded only on the validator that received the transaction directly (route
  the workload to a known validator), and its tail includes deferral time, so
  it reads together with `consensus_handler_transaction_deferral_rounds`.
- Checkpoint lag: time from a consensus commit being created to the checkpoint
  being built, via `checkpoint_creation_latency` (p50, p95, p99, per validator).
  It measures how far checkpoint construction trails consensus: the builder can
  only seal a checkpoint once the transactions in that commit have executed,
  and shared-object transactions execute in consensus-assigned order
  (serialized per object), so under contention the sequential-execution backlog
  surfaces directly as checkpoint lag. The metric records seconds (via
  `as_secs_f64`) despite help text that says milliseconds.
- Congestion: per-object deferrals, cancellations, accumulated cost per object,
  commit sizes, deferral-round depth.
- Execution overshoot: how far actual per-object execution exceeds the limit
  the scheduler intended (the risk when the estimate under-counts). The
  scheduled side exists -
  `consensus_handler_max_congestion_control_object_costs` (max accumulated
  estimate per commit), so scheduled-vs-limit overshoot is derivable. The actual
  per-object executed cost is not metered (see validator node changes); compare
  it against the limit, or derive from the accuracy ratio.
- Execution queue depth and latency (all exist): depth via
  `execution_driver_dispatch_queue` and
  `transaction_manager_num_pending_certificates`; queueing latency via
  `execution_queueing_delay_s` (and the overload monitor's
  `execution_queueing_latency`, soft 1s / hard 10s); execution time via
  `authority_state_internal_execution_latency`; backpressure via
  `execution_cache_backpressure_status` / `_toggles`.
- Attestation cost via `validator_attestation_latency`, split into
  `validator_attestation_queue_wait` (blocking-pool wait),
  `validator_attestation_execution_latency` (the Move-VM dry-run), and
  `validator_attestation_async_resume_latency` (the wait for the async task to
  run again after the dry-run finishes) - under load this resume part is the
  largest, so read the split, not just the total. Plus the
  `validator_attestation_task_panics` counter.
- Consensus-queue depth and load shedding. Depth via
  `sequencing_certificate_inflight` (`num_inflight` - transactions submitted to
  consensus but not yet sequenced, the value the graduated / max-pending limits
  gate on, per validator). Pre-consensus admission shedding via
  `transaction_overload_sources` (by source; see validator node changes) and
  `validator_service_num_rejected_tx_during_overload`, with the graduated
  `consensus_queue_load_shedding_percentage`. Post-consensus (execution
  overload) shedding via `consensus_handler_load_shedding_dropped_transactions`
  and the percentages `consensus_handler_load_shedding_percentage` (enforced
  quorum) and `authority_load_shedding_percentage` (this validator's local view).
- Attested vs actual computation cost ratio (histogram).
- Consensus handler commit rate via `rate(consensus_committed_subdags[2m])`
  (and `consensus_handler_processed`). Post-consensus validation time is NOT
  metered today (only `consensus_handler_validation_dropped_transactions`, a
  drop count); see validator node changes - worth adding since Check `#3`
  (attestor verification) is new work on that path.
- Correctness alarms: checkpoint forks via `split_brain_checkpoint_forks` /
  `remote_checkpoint_forks` / `global_state_hash_inconsistent_state`;
  equivocation via `validator_service_num_rejected_tx_soft_lock_conflict` and
  `total_client_double_spend_attempts_detected`. Stuck objects have no direct
  alarm - infer from `transaction_manager_num_pending_certificates` not
  draining, plus deferral / cancellation rate and queue depth.
- CPU and memory: host-level (whole machine) via node-exporter
  (`node_cpu_seconds_total`, `node_memory_*`), already scraped. Per-validator
  container CPU/memory (`container_cpu_usage_seconds_total`,
  `container_memory_rss`) via cadvisor, now wired into grafana-local. cadvisor
  labels series by container `name` (e.g. `validator-1`), which equals the
  `host` label values, so the dashboard's `validator` variable still filters
  them. The node has no per-process metrics on :9184, so cadvisor is the only
  per-validator source.

---

## Method and reporting

Compare the modes on the same workload, seed, and hardware, changing one
variable at a time. Repeat each run several times and report medians and
variance, because stress results are noisy. For each mode, raise the
transaction rate until it saturates.

- H1, H2, H3 have no pass/fail threshold: measure and report the numbers
  (attestation overhead; `TotalComputationUnits` vs `TotalTxCount`;
  `TotalComputationUnits` vs `TotalGasBudget`, plus the accuracy ratio). Any
  threshold is a later product decision, not a gate here.
- H4 is the only pass/fail: it fails if any run forks, leaves a transaction
  stuck or starved, or crashes a validator.

---

## Other things to test

1. Wrong cost limit. Vary the per-object limit and find the safe range. Too
   high means no congestion control; too low means throughput drops.
2. Fallback scale. V1 and V2 never mix, so there is no intra-run fairness issue;
   instead confirm that an all-V1 run (fallback, `gas_budget / gas_price`) and
   an all-V2 run (attested cost) use the same units scale, so one per-object
   limit is valid for both.
3. Post-consensus validation cost. At high attested-transaction rates, check
   whether the per-transaction object load in the consensus handler slows down
   commit processing. Plot handler throughput against the attested rate.
4. Gas-price feedback. The suggested and clearing gas-price calculator now uses
   durations in gas units. Check that it still produces sensible clearing prices
   and that the feedback loop is stable (no oscillation or runaway).
5. Epoch boundaries and reconfiguration under sustained load in the new mode.

---

## Phases

1. Validator node code changes. Add the metrics and attestor changes from the
   "Validator node changes needed for this plan" section. Must be first - every
   measurement depends on them.
2. Setup. Rebuild the docker images with those changes, bootstrap, bring up the
   network and monitoring (`start.sh`), set the per-mode cost limit, and verify
   the overrides applied (P0b, P0c).
3. H1 - attestation overhead (W4, owned-object): V1 vs V2.
4. H2 - new mode vs `TotalTxCount` (W1).
5. H3 - new mode vs `TotalGasBudget`, plus accuracy (W2, W5).
6. H4 - safety / robustness: W6, W7, one run per cost limit, fallback.
7. Scale and performance on EPYC: saturation runs and consensus handler cost,
   plus gas-price feedback and epoch / reconfiguration under load.

---
