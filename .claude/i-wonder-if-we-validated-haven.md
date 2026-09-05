# Design Doc / Feasibility Spike: Epoch-Partitioned Historic Stores

> **Deliverable**: this is a written design + risk + validation spike. **No production code** is
> proposed here — the goal is to decide whether (and how) to build it, and to scope the follow-up
> implementation plans. Scope covers **both** checkpoint-keyed history **and** object versions, and
> targets **both** goals: cheaper pruning and a faster live `objects` table.

## Context

Today, pruning (`crates/iota-core/src/authority/authority_store_pruner.rs`) runs at **checkpoint
granularity** and deletes rows out of **shared perpetual tables**. Two costs result:

1. **Pruning is expensive.** Deletes go into the LSM as range-deletes / point-deletes / a compaction
   filter, then need a _separate_ manual SST-compaction pass (`PERIODIC_PRUNING_TABLES`,
   `periodic_compaction_threshold_days`, `compact_next_sst_file`) to actually reclaim disk. We pay
   large write-amplification to remove data we already know we want gone.
2. **The hot `objects` table mixes live and historic versions.** `objects: DBMap<ObjectKey,
   StoreObjectWrapper>` (key `ObjectKey = (ObjectID, version)`) holds _every_ version. Latest-version
   reads reverse-iterate past superseded versions; the table is larger than the working set.

**Idea under evaluation** (from the requester): partition historic data into **per-epoch RocksDB
stores**, and when an epoch ages out of retention, **drop the whole directory** (`rm -rf epoch_N/`)
instead of range-deleting. Keep the live `objects` table to the live set only.

**This pattern already exists in the repo** for ephemeral epoch data (`epoch_N/` dirs via
`EPOCH_DB_PREFIX` in `authority_per_epoch_store.rs`), for **archival** (`archive/epoch_N/` in
`iota-archival`), and for **db_checkpoint** (`db_checkpoint_handler.rs`). So "one store per epoch,
drop the folder on expiry" is a proven shape here, not a new risk class — _for the checkpoint-keyed
half_. The object-version half is more involved (it touches the live `objects` table), but caller
analysis below shows its read paths never reach historic stores on the node, so the residual risk is
about _relocation correctness_, not reads.

## Conclusion up front (recommendation)

- **Checkpoint-keyed history: clear win, low risk. Build it.** Routing already exists; the main
  consumer (state sync) already gates on a watermark. This is the part that turns O(data) compaction
  churn into O(1) folder drops.
- **Object live/historic split: valuable, and lower risk than first feared. Still feature-flag +
  benchmark.** Caller analysis (§B) shows `find_object_lt_or_eq_version` always resolves to the
  **live head** on node paths (Lamport invariant), so there is **no hot-path multi-DB scan**. The
  consensus paths (re-execution, revert) only touch recent versions that relocation never moves
  (relocation lags the retention watermark, exactly like today's pruning) — so relocation is strictly
  safer than today's deletion. The only genuine historic read left is **gRPC `get_object_by_key` at a
  past version**, a clean exact-key point lookup the filter design serves directly. Residual risk is
  the head-invariant + atomic relocation, not the read path.
- **Routing is done with immutable per-epoch membership filters, not routing tables.** A global
  `digest → epoch` table reintroduces incremental per-epoch row deletes (LSM tombstones + compaction)
  — the exact churn we're escaping. Instead: a sealed epoch DB is _immutable_, so build a bloom/ribbon
  filter once at seal time, persist it, load it into RAM, and drop it with the folder on expiry. No
  incremental maintenance, no compaction. Filters sit on top of an **LRU of lazily-opened read-only
  epoch DB handles** (they say _which_ DB to open, not where the value is). See §C for the limits.

---

## Part A — Checkpoint-keyed history (low risk)

Data: `transactions`, `executed_effects`, `executed_transactions_to_checkpoint`, `effects`,
`events` / `events_2`, and the checkpoint tables (`checkpoint_content`, `checkpoint_by_digest`,
`checkpoint_sequence_by_contents_digest`).

### Why this is the easy half

- **Access is by checkpoint sequence number** (state sync — `iota-network/src/state_sync/server.rs`,
  `RocksDbStore` in `storage.rs`), which maps to an epoch trivially. State sync already advertises a
  `lowest_available_checkpoint` watermark to peers, so a coarser (per-epoch) availability horizon is
  a config change, not a protocol change.
- **Routing already exists for the common cases**: `epoch_last_checkpoint_map` (checkpoints/mod.rs)
  gives checkpoint-seq → epoch; `executed_transactions_to_checkpoint` gives tx-digest → `(epoch,
  checkpoint)`. Only the other digest lookups (effects, checkpoint) need the per-epoch filters of §C.

### Design sketch

- Bulk bodies (transactions / effects / events / checkpoint contents) live in per-epoch stores
  `history/epoch_N/`, written as checkpoints finalize (or relocated in a background sweep).
- **Checkpoint-seq routing needs no extra structure**: `epoch_last_checkpoint_map`
  (checkpoints/mod.rs) already gives checkpoint-seq → epoch (one tiny entry per epoch, already
  retained). State sync — which reads by checkpoint-seq — routes for free.
- **Digest routing uses immutable per-epoch filters** (see §C), not new tables: `tx_digest`,
  `effects_digest`, `checkpoint_digest` membership is answered by querying each sealed epoch's filter,
  then opening the one DB that says "maybe". `tx_digest → (epoch, checkpoint)` also already exists in
  `executed_transactions_to_checkpoint` and can serve as the tx route directly.
- Pruning = drop `history/epoch_N/` + drop its (immutable) filter + evict it from RAM. All O(1). No
  routing table to range-delete, no SST compaction pass for these column families at all.
- **Open epoch DBs lazily, read-only, behind an LRU** (see §C). Never hold 100 open.

### Consumers to reroute (find-and-verify, not enumerated line-by-line)

- State sync read path (`storage.rs` `RocksDbStore::try_get_full_checkpoint_contents`,
  `try_get_checkpoint_by_*`) — the primary consumer.
- gRPC server read path (`iota-grpc-server`) for transactions/effects/events/checkpoints by digest or
  seq, and `get_object_by_key`. (JSON-RPC is being removed — its read paths are out of scope.)
- gRPC indexes store prune (`grpc_indexes.rs`).

---

## Part B — Object versions (moderate risk; feature-flag + benchmark)

Goal: keep the main `objects` table to **live heads only**; relocate strictly-superseded versions to
per-epoch historic object stores; drop the folder on expiry.

### Key correction to the original framing

- **"Move at creation time" is not possible.** A version becomes historic only when a _later_
  transaction (possibly a later epoch) supersedes it. So relocation must still be triggered at the
  same moment pruning detects supersession today (a version appearing in
  `effects.modified_at_versions()`). It is **relocate-instead-of-delete**, not write-time routing.
- **Routing key already in the data**: `StoreObjectValueV2.previous_transaction_checkpoint:
  Option<CheckpointSequenceNumber>` (`authority_store_types.rs`) lets us derive version → checkpoint
  → epoch — _but_ it is `Option` and `None` on legacy (pre-V2) rows and dirty-cache rows. Routing
  needs a checkpoint→epoch map plus a fallback policy for `None`.

### Invariants the design MUST guarantee

1. **Head invariant.** The newest version of every ObjectID — _including Deleted/Wrapped tombstones_
   — stays in the live table until the whole lineage is retention-expired. Tombstones are real and
   current: `write_one_transaction_outputs` (`authority_store.rs:899-902`) writes `StoreObject::Deleted`
   / `StoreObject::Wrapped` rows into `objects` at the deleting tx's Lamport version, and
   `object_reference` (`authority_store_tables.rs:298-305`) maps them to `ObjectDigest::OBJECT_DELETED`
   / `OBJECT_WRAPPED`. Every reverse-iteration "latest" read depends on the tombstone staying live:
   - `get_latest_object_ref_or_tombstone`, `get_latest_object_or_tombstone`, `try_get_object`
     (`authority_store_tables.rs`) and the live-set iterators below. (Deletion is _also_ recorded in
     `object_per_epoch_marker_table` as `(epoch, ObjectKey) → MarkerValue`, which is already per-epoch
     and prunes with the epoch — but the `objects`-table tombstone is the load-bearing "head".)
2. **Atomic, complete relocation.** When relocating an ID, move _all_ versions ≤ the boundary
   together. This preserves the existing leak-avoidance guarantee in `prune_objects` (the explicit
   reason tombstones use point-deletes, not range-deletes). No reader window may see a gap.
3. **`find_object_lt_or_eq_version` stays a main-table-only lookup — no historic reach needed.**
   Caller analysis: `read_child_object` (`writeback_cache.rs:2266`) and the authenticator lookup
   (`authority.rs:5613`) pass the parent's _incoming_ version as the bound; by the Lamport invariant
   the child's current version is always ≤ that bound, so the answer is always the **live head** (the
   execution-cache doc comment states this). Its only historic-needing callers are off the node store
   — JSON-RPC (being removed), the indexer (own Postgres/archive `ObjectProvider`), and replay (own
   remote fetcher, not the local store). gRPC does not call it at all. So there is **no multi-DB
   descending scan** to build.
4. **Relocation must never move versions the consensus path can still read.** Re-execution loads
   inputs by exact `ObjectKey` (`transaction_input_loader.rs` → `multi_get_objects_by_key`) and
   `revert_state_update` re-reads _previous_ versions and panics if absent — but both only touch
   _recent_ (current-epoch) versions, which relocation never moves: relocation lags the retention
   watermark, exactly as today's pruner does. Keep that watermark coupling (invariant 6) and these
   paths never miss. The only exact-key read that legitimately reaches historic data is **gRPC
   `get_object_by_key` at a past version** — a clean point lookup served by filter → one epoch DB →
   point-get.
5. **Retire the compaction-filter pruner.** `ObjectsCompactionFilter` + `object_tombstones` can only
   Keep/Remove — it **cannot relocate** — so it must be removed/repurposed. `compact_next_sst_file`,
   `PERIODIC_PRUNING_TABLES`, and `try_checkpoint_db` (db_checkpoint) must learn about N stores.
6. **Watermark coherence.** `pruned_checkpoint` couples object relocation with checkpoint/effects
   pruning (`authority_store_pruner.rs`); keep them consistent or the checkpoint pruner stalls/races.

### Where the split actually _helps_

- `iter_live_object_set` / `range_iter_live_object_set` (`LiveSetIter`) and their consumers — global
  state hasher (`global_state_hasher.rs`), snapshot writer (`iota-snapshot/src/writer.rs`),
  conservation check — all want **live objects only**. With heads-only in the main table these get
  _cheaper_ (no skipping superseded versions). This is the concrete perf upside.

### Sharpest hazards (must be designed around, with tests)

- **Tombstone-as-head**: a freshly-deleted object's newest state _is_ a tombstone. If tombstones
  don't stay in the live table, "latest" reads return "never existed" — a correctness fork.
- **Relocation watermark must trail the consensus working set** so re-execution/`revert_state_update`
  never read a relocated previous version. This is the same boundary today's pruner already respects;
  the design must not let relocation run ahead of it.
- **Legacy `None` checkpoint stamp** on pre-V2 rows → unroutable for relocation; need a migration /
  fallback (e.g. route via a checkpoint→epoch map, or leave legacy rows in the main table).
- (No longer a hazard for node execution: child resolution never reaches historic stores — see
  invariant 3. It only mattered for the now-removed JSON-RPC / off-node consumers.)

---

## Part C — Routing & RocksDB instance management (shared by A and B)

The "100 open DBs is slow" worry is real but avoidable. Two layers:

**Handle layer — never keep them open.** Open epoch stores **lazily, read-only** (mirror the existing
`open_readonly` path) behind a **bounded LRU of handles**. The repo already caps live epoch DBs at
`num_latest_epoch_dbs_to_retain: 3` precisely because each instance costs block cache + memtables +
WAL + FDs + bg threads. A read hits the LRU; a miss opens (reads MANIFEST) and evicts the coldest.

**Routing layer — immutable per-epoch membership filters.** A sealed epoch DB is immutable, so build a
bloom/ribbon filter over its keys **once at seal time**, persist it, and load it into RAM. Query the
filters to decide _which_ epoch DB(s) to open. On expiry, drop the folder + the filter together — O(1),
no incremental cleanup. Use a plain immutable bloom/ribbon (cuckoo's deletion support buys nothing
here). This is what replaces routing _tables_ and their compaction churn.

### Honest limits of the filter approach (objections to be designed around)

1. **Membership, not location** — a filter hit only means "open this DB and read." The filter is an
   optimization _on top of_ the handle LRU, not a replacement; you still pay the DB open.
2. **Historic object reads are all exact-key — the filter's good case.** The only historic object
   read on a node-served path is gRPC `get_object_by_key(id, version)`: filter on `(id, version)` →
   open one epoch DB → point-get. `find_object_lt_or_eq_version` (the "greatest version ≤ V" case a
   membership filter _can't_ answer) never reaches historic stores on node paths (invariant 3), so the
   descending-scan limitation is moot here.
3. **False positives scale with epoch count.** At 1% FPR × 100 epochs a lookup opens ~1 true + ~1
   spurious DB. Tightening FPR enlarges filters → RAM-vs-wasted-opens knob.
4. **All-epochs-resident RAM cost.** ~1.2 B/key at 1% FPR ⇒ `retained_historic_keys × 1.2 B`
   (rough feel: ~1B entries ≈ ~1.2 GB RAM), growing with throughput/retention. This mandatory RAM is
   the price for "no surgical disk cleanup"; a disk routing table only caches its hot part. Tunable
   via FPR; older epochs' filters may themselves be LRU'd if RAM-bound.
5. **Where filters live / restart load.** Storing each filter inside its epoch DB means opening all N
   DBs once at boot to load them; a tiny always-resident sidecar keyed by epoch (one row per epoch,
   trivially dropped) avoids that. Decide during Phase A.

---

## Critical files

- `crates/iota-core/src/authority/authority_store_pruner.rs` — pruning triggers, `prune_objects`,
  `prune_checkpoints`, `ObjectsCompactionFilter`, SST compaction, watermarks.
- `crates/iota-core/src/authority/authority_store_tables.rs` — `objects` and the checkpoint-keyed
  tables; reverse-iteration reads; `LiveSetIter`; `find_object_lt_or_eq_version`.
- `crates/iota-core/src/authority/authority_store_types.rs` — `StoreObjectValueV2`
  (`previous_transaction_checkpoint`).
- `crates/iota-core/src/authority/authority_store.rs` — `multi_get_objects_by_key`,
  `revert_state_update` (panic-on-miss).
- `crates/iota-core/src/execution_cache/writeback_cache.rs` — every backing-store read; `read_child_object`.
- `crates/iota-core/src/storage.rs` + `iota-network/src/state_sync/server.rs` — state-sync read path.
- `crates/iota-config/src/node.rs` — `AuthorityStorePruningConfig` (new flags: feature toggle,
  per-epoch retention, handle-LRU size).
- Reference patterns to reuse: `authority_per_epoch_store.rs` (`EPOCH_DB_PREFIX`, `open_readonly`,
  `release_db_handles`), `iota-archival`, `db_checkpoint_handler.rs`.

---

## Recommended path (phasing)

1. **Phase 0 — this doc + decision.** Confirm appetite; decide whether B is feature-flagged-prototype
   or deferred.
2. **Phase A — checkpoint-keyed history.** Implement per-epoch history stores + immutable per-epoch
   filters + drop-folder pruning + lazy read-only LRU. Reroute state-sync / RPC reads. Lowest risk,
   delivers most of the pruning-cost win.
3. **Phase B — object live/historic split**, feature-flagged, behind the A infrastructure. Enforce
   the head-invariant + atomic relocation, gate relocation behind the consensus-working-set watermark,
   add the gRPC exact-key historic point fallback, then retire the compaction-filter pruner. No
   descending-scan read path is needed (invariant 3).

## Validation / benchmarking plan (no code yet — this is the spike's exit criteria)

**Correctness (must pass before either phase ships):**

- Existing `iota-e2e-tests` + pruner unit tests with the feature on (`cargo simtest`,
  `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core`).
- Targeted tests for each hazard in §B: tombstone-as-head latest read; relocation watermark never
  moves a version still reachable by re-execution/`revert_state_update`; gRPC `get_object_by_key` at a
  relocated past version; `read_child_object`/`find_object_lt_or_eq_version` still resolving to the
  live head with relocation active; legacy `None`-stamp routing.
- State-sync serving checkpoints whose bodies live in a historic store; behavior at the
  `lowest_available_checkpoint` horizon.

**Performance (the reason we're doing this):**

- Pruning cost: disk reclaimed per prune, write-amplification, and CPU of a folder-drop vs the
  current range-delete + manual SST compaction, over a multi-epoch synthetic load.
- Live `objects` table: latest-version read latency and live-set iteration time (state hash,
  snapshot) with heads-only vs mixed.
- Historic read latency: cold-open + LRU-warm gRPC `get_object_by_key` exact-key reads at relocated
  historic versions (filter hit → open one epoch DB → point-get).

**Exit criterion:** Phase A shows materially lower prune cost with no read-path regression; Phase B
prototype shows the live-table read/iter win and meets a hard latency bound on historic reads, with
all hazard tests green — _otherwise B stays deferred_.
