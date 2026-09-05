# Live/Historic Objects Split with Per-Epoch History Buckets

## Context

Re-analysis of the older design doc (`iota_2/.claude/i-wonder-if-we-validated-haven.md`), with every code claim re-verified against this checkout. Goal: an efficient RPC fullnode — keep the hot `objects` table heads-only, make pruning O(1) bucket drops instead of per-key delete + compaction churn, and retain ~100 epochs of historic versions for gRPC.

### Verdict on the old plan

Structurally sound; almost all code claims verified TRUE (pruner mechanics, tombstone-as-head, `LiveSetIter`, Lamport head-resolution for `find_object_lt_or_eq_version` node callers at `writeback_cache.rs:2266` and `authority.rs:5717`, per-epoch DB patterns). Three corrections drive this plan's differences:

1. **"JSON-RPC is being removed" is not visible in the code** — it's fully wired with 4 historic-read paths needing "greatest version ≤ V" scans. **User decision: removal is real and planned out-of-repo; those paths are out of scope** (they return NotFound for relocated versions, same as pruned versions today).
2. **The old plan's "separate RocksDB per epoch + sealed bloom filters" loses to a simpler layout.** typed-store gives every DB instance a _private_ 128 MB block cache (`typed-store/src/rocks/options.rs:449`) and `max_open_files = ulimit/8` _per DB_ — 100 epoch DBs ≈ 12.8 GB cache + FD exhaustion, or new LRU-handle machinery. Instead: **one always-open `history` RocksDB with one column family per epoch; `drop_cf` is O(1)** (CF-exclusive SSTs deleted outright, no tombstones). RocksDB's _native_ per-SST bloom filters (10 bits/key, already typed-store default, pinned) make explicit sealed filters and routing tables redundant: a point-get miss in a sealed, compacted CF is ~0.5–2 µs CPU, zero I/O → probing 100 CFs newest→oldest costs ~50–200 µs worst case, negligible vs. RPC RTT.
3. **Bucket by supersession epoch, not creation epoch.** The pruner always knows the epoch of the checkpoint it is pruning — no dependency on `previous_transaction_checkpoint` (which is `None` on legacy V1 rows), no fallback policy, and retention semantics ("readable for N epochs after superseded") match exactly. This kills the old plan's "legacy None-stamp" hazard entirely.

### On the RocksDB intuition (user question)

RocksDB is an LSM, not a B-tree — inserts append to a memtable; nothing "rebalances". The real wins of a small live table: (a) less compaction write-amplification (fewer levels / less data rewritten), (b) faster reverse-iteration latest-reads and `LiveSetIter` scans (state hashing, formal snapshots, conservation check), (c) pruning stops injecting millions of delete-tombstones + manual SST compaction into the hot CF. The wins are real; they come from compaction and reads, not insert cost.

### Key safety insight

A heads-only live table is exactly what an aggressively-pruned validator (`num_epochs_to_retain = 0`) already runs. All consensus/execution read paths are proven against that regime. This change only alters **where deleted rows go** (history bucket instead of oblivion) and **when tombstone heads die** (historic horizon instead of live horizon).

### Scope decisions (user)

- **Objects split is the priority** (this plan). Checkpoint-keyed history (transactions/effects/events → epoch CFs `hist_tx_e{N}` etc.) is a designed-for follow-up on the same substrate.
- Target: **RPC fullnodes, ~100 epoch retention**. Validators keep today's pruning; feature is fullnode-only and config-gated.

---

## Architecture

```
write path (unchanged): writeback cache → build_db_batch → perpetual.objects
pruner loop (modified, per epoch-homogeneous checkpoint batch at the live horizon):
    superseded keys = effects.modified_at_versions()
    1. multi_get values from perpetual.objects (skip None → idempotent replay)
    2. history.put_objects(epoch, rows) + record tombstone-head expiry list   [WAL-less]
    3. history.flush_epoch(epoch)                                             [durability barrier]
    4. ONE atomic perpetual batch: point-delete superseded keys + advance pruned_checkpoint
history retention task (new): for epoch E ≤ current − historic_retention:
    1. point-delete E's tombstone-head expiry list from perpetual.objects     [idempotent]
    2. history.drop_epoch(E)  → drop_cf, O(1)
gRPC exact-version read: live table → miss → history probe newest→oldest CF   [gRPC only]
```

Invariants:

- **Head invariant**: the newest version of every ObjectID — including `Deleted`/`Wrapped` tombstones — never leaves the live table until its bucket's expiry. `modified_at_versions()` only yields superseded _inputs_, so this holds structurally; assert it.
- **Consensus never touches history**: input loader, `revert_state_update`, `find_object_lt_or_eq_version`, writeback cache stay live-only. A miss there must stay a loud panic — no silent fallthrough. Historic reads are an **explicit separate API** used only by `GrpcReadStore`.
- **Crash consistency**: watermark (`pruned_checkpoint`) is the sole idempotency anchor; history writes happen (durably) before live deletes; duplicates are harmless (readers check live first, bytes identical). Expiry side: delete heads first, then `drop_epoch` (reversed order would lose the list).
- V1 legacy rows: relocated as raw `StoreObjectWrapper` bytes; history reads apply the existing `migrate()`-at-read discipline (`authority_store_types.rs:48-53`).

---

## Work items (ordered)

### WI-1: typed-store runtime CF support

`crates/typed-store/src/database.rs`: add `Database::create_cf(name, &rocksdb::Options)` (~15-line delegate to `DBWithThreadMode<MultiThreaded>::create_cf`, next to existing `drop_cf` at `database.rs:230`). Handle `RocksDB.cf_names` being a fixed Vec (either `RwLock` it or don't rely on `flush_all` for history). Unit test create→write→drop_cf→reopen in `crates/typed-store/src/rocks/tests.rs`.

### WI-2: `HistoricStore` (`crates/iota-core/src/authority/historic_store.rs`)

One RocksDB at `<parent>/history` (next to `perpetual`, `pruner`), bypassing the static `DBMapUtils` derive:

- Open via `open_cf_opts`, but `list_cf(path)` first and pass every discovered `hist_*` CF with tuned options (undiscovered CFs otherwise get default options — verified caveat in `rocks/mod.rs:236-258`). One cloned `DBOptions` across CFs shares one block cache (same pattern as `AuthorityPerpetualTables::open`, `authority_store_tables.rs:199-229`).
- CF options: universal compaction (`optimize_for_write_throughput_no_deletion`), blob files (`optimize_for_large_values_no_scan`, same as perpetual `objects`), native bloom + pinned filter/index blocks; block cache size via env `HISTORY_BLOCK_CACHE_MB`.
- CFs: `hist_obj_e{N}` (ObjectKey → StoreObjectWrapper), expiry list per epoch (tombstone-head keys, stored **inside** the bucket so drop = one unit), tiny always-open `meta` CF (`EpochId → {sealed, key_count, bytes, min/max_checkpoint}`); in-memory `RwLock<BTreeMap<EpochId, EpochTables>>` mirrors it, extended via `create_cf` + `DBMap::reopen` on first write to a new epoch.
- Trait surface (merged from both designs):
  - `put_objects(epoch, &[(ObjectKey, StoreObjectWrapper)], tombstone_heads)` — WAL-less batched insert (`DBBatch::write_opt` with `disable_wal`; relocation is replayable from the watermark).
  - `flush_epoch(epoch)` — durability barrier; MUST precede the caller's live-delete+watermark batch.
  - `seal_epoch(epoch)` — flush + compact-to-bottommost + seal record in `meta` (called when the pruner crosses an epoch boundary).
  - `get_object(key)` — probe CFs newest→oldest, return first hit; `get_object_in_epoch` for tests/tools.
  - `tombstone_heads(epoch)`, `drop_epochs_below(cutoff)` (drop_cf + meta cleanup, idempotent), `earliest_epoch()`, `list_epochs()`.
- Metrics: probe-count histogram, hit-epoch-age, not-found counter, epochs-retained gauge, relocated-bytes/flush/seal durations. Skip per-CF 30 s metric reporters for sealed CFs (add honest `report_metrics: bool` to `DBMap::reopen`, don't abuse `is_deprecated`).

### WI-3: Config (`crates/iota-config/src/node.rs`, `AuthorityStorePruningConfig` :1013)

- `historic_object_store: Option<HistoricObjectStoreConfig>` — `num_epochs_to_retain` (default 100), `max_relocation_batch_bytes` (default ~128–256 MiB).
- Existing `num_epochs_to_retain` keeps meaning (live window; with split, "act" = relocate not delete). RPC operators: live window 1–2 + historic 100.
- Validation, enforced **before `AuthorityPerpetualTables::open`** (the compaction filter is installed at DB-open time via `objects_table_config`, wired in `iota-node/src/lib.rs:424-446`): split + `enable_compaction_filter` = startup error (`ObjectsCompactionFilter` can only Keep/Remove — it would destroy rows before relocation reads them). Validator + split → warn and disable. Warn on `num_epochs_to_retain == u64::MAX` with split (relocation never triggers). Note `enable_compaction_filter` defaults `true` under `cfg!(test)/cfg!(msim)` (node.rs:1090) — split tests must disable it explicitly.

### WI-4: Pruner relocation (`crates/iota-core/src/authority/authority_store_pruner.rs`)

- `prune_for_eligible_epochs` (:453): make checkpoint batches **epoch-homogeneous** (flush pending batch when `checkpoint.epoch()` changes); pass `epoch` into `prune_objects`.
- `prune_objects` (:154-245): when history enabled — multi_get superseded keys (skip `None`), `put_objects` + `flush_epoch`, then one perpetual batch of point-deletes + `set_highest_pruned_checkpoint` — **replacing** both the range-delete branch (:206-211) and the tombstone lineage scan+point-delete (:221-235). Record `effects.all_tombstones()` keys as the bucket's expiry list. Byte-budget flushes (values are now materialized in memory; pattern: `authority_store_migrations.rs::migrate_events`). When disabled: existing behavior byte-for-byte.
- `setup_pruning` (:717): new retention task — while `earliest_epoch() ≤ current − historic_retention`: batched point-delete of `tombstone_heads(E)` from live `objects`, then `drop_epoch(E)`. Residual live-delete volume = one ~40 B key per object death (strict small subset of today's deletes); existing periodic SST compaction absorbs it.
- `seal_epoch(N−1)` when the relocation loop first writes to epoch N.
- `AuthorityStorePruner::new` (:810): accept `Option<Arc<HistoricStore>>`; validator coercion.

### WI-5: Wiring (`crates/iota-node/src/lib.rs` ~:429-450, `crates/iota-core/src/authority.rs` ~:3275-3302)

Open `HistoricStore` when configured; store on `AuthorityState` (like `grpc_indexes_store`, alongside `pruner_db` for shutdown drop-ordering); forward to the pruner. No reconfig hook needed (history DB is epoch-agnostic, always open).

### WI-6: gRPC read path (`crates/iota-core/src/storage.rs`)

- `GrpcReadStore::try_get_object_by_key` (:389): live lookup → on `Ok(None)`, consult `historic_object_store`. `GrpcReadStore` is constructed only for the gRPC server (:369) → structurally unreachable from consensus; doc-comment the containment invariant.
- `GrpcStateReader::get_lowest_available_checkpoint_objects` (:500-510): with history enabled, advertise availability from `earliest_epoch()` via `epoch_last_checkpoint_map` (fallback: today's `pruned_checkpoint + 1`).
- **Do NOT touch**: `ObjectStore for AuthorityPerpetualTables`, `AuthorityStore::multi_get_objects_by_key`, writeback cache reads, `RocksDbStore` `ObjectStore` impl, `TransactionInputLoader`, `revert_state_update`, `try_find_object_lt_or_eq_version`, `get_object_received_at_version`.

### WI-7: Migration & ops

- **Lazy migration — the pruner loop IS the backfill.** Operator flips the flag, lowers live `num_epochs_to_retain` (e.g. 100 → 2); the watermark is ~98 epochs behind and the existing loop relocates everything incrementally with correct supersession epochs, using built-in smoothing/batching/crash-resume. (A one-time table sweep can't know supersession epochs — rejected.)
- **Watermark fast-forward hazard**: a node that ran objects-pruning-off (`u64::MAX`) with checkpoint pruning on has checkpoint data missing below the objects watermark (`prune_checkpoints_for_eligible_epochs` skips the cap for MAX, :412-419). On first enable: if `pruned_checkpoint <` checkpoint-store `HighestPruned`, fast-forward and log that older superseded rows stay in the live table (non-heads, skipped by all readers; optional later "legacy delete" pass).
- **db_checkpoint**: history DB excluded from `checkpoint_all_dbs` (`authority.rs:3817-3866`) by construction — document why (reconstructable, optional, potentially TBs). `db_checkpoint_handler::prune_and_compact` re-runs the pruner inside a snapshot — must run with relocation **disabled** there (explicit flag on the pruner entry point).
- **Formal snapshots** (`iota-snapshot`): live-set only — unaffected, and `iter_live_object_set` gets faster. Restored nodes have empty history → NotFound for old versions; document.
- `iota-tool` db_tool: register the `history` path (`open_cf_opts_secondary` auto-discovers CFs).
- Genesis (~4 GB bulk ingest): all heads, all live — zero interaction.

### WI-8: Tests

Mostly in `authority_store_pruner.rs` tests (reuse `generate_test_data`/`run_pruner`) + `historic_store.rs` unit tests:

1. Relocation correctness: post-prune, live = exactly heads (incl. Deleted/Wrapped heads); history = exactly the superseded set, in the pruned checkpoint's epoch bucket.
2. Tombstone-as-head: `get_latest_object_ref_or_tombstone` returns OBJECT_DELETED/OBJECT_WRAPPED post-relocation.
3. Containment: writeback-cache exact-key reads return `None` for relocated keys; `GrpcReadStore` returns the object.
4. Crash replay: history write lands, live delete doesn't (`fail_point!`); rerun → identical state.
5. Supersession bucketing: created epoch 1, superseded epoch 5 → bucket 5; `drop_epoch(1)` leaves it readable.
6. Expiry + resurrection: `drop_epoch` deletes tombstone heads; an unwrap-resurrected lineage survives the exact-key expiry delete of its stale `Wrapped` tombstone.
7. V1 rows: insert via `insert_store_object_v1_test_only` (:488), relocate, read from history → migrates at read.
8. Strongest invariant: `GlobalStateHash` over `iter_live_object_set` bit-identical before relocation / after relocation / after expiry drops.
9. Config gating: filter+split error; validator+split disabled with warning.
10. HistoricStore: restart CF rediscovery, seal idempotency, drop-while-concurrent-read, WAL-less flush barrier.

Sequencing: WI-1 → WI-2 (+tests 10) → WI-3 → WI-4 (+tests 1,2,4,5,7,8) → WI-5/WI-6 (+test 3) → retention task (+test 6) → WI-7 (+test 9).

---

## Sizing (100 epochs, native blooms pinned)

| relocated versions/epoch | history size | pinned filter+index RAM                              |
| ------------------------ | ------------ | ---------------------------------------------------- |
| 1k (≈ today)             | ~100 MB      | ~200 KB                                              |
| 1M                       | ~100 GB      | ~500 MB                                              |
| 50M (ceiling)            | ~5 TB        | ~25 GB → partitioned filters, pin top level → 1–2 GB |

Explicit sealed bloom filters were dropped: native SST blooms hold the same ~1.25 B/key and are needed anyway; explicit ones would be pure RAM duplication to save ~100 µs per not-found lookup. A global (id,version)→epoch routing table stays rejected: dropping an epoch would re-inject N per-key tombstones into a hot table — the exact pathology this removes. `seal_epoch` remains the natural place to add filters later if profiling disagrees.

## Follow-up (out of scope here, substrate ready)

Checkpoint-keyed history (transactions/effects/events/checkpoint contents) into `hist_tx_e{N}` CFs in the same DB: digest lookups probe newest→oldest identically; checkpoint-seq lookups route free via never-pruned `epoch_last_checkpoint_map` (`checkpoints/mod.rs:190`). Do after the objects split proves out.

## Verification

- `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p typed-store -p iota-core --lib` (10+ min timeout), then `cargo simtest -p iota-e2e-tests`; `cargo ci-clippy`.
- Bench (exit criteria): multi-epoch synthetic load comparing (a) prune cost — disk reclaimed, write-amp (rocksdb stats), CPU of drop_cf vs delete+compact; (b) live-table latest-read latency + `iter_live_object_set` wall time heads-only vs mixed; (c) gRPC historic read latency across bucket ages (probe-count histogram validates the newest-first model).
- Manual e2e: fullnode with split on, live window 2 / historic 5, drive object churn across ≥6 epochs; verify gRPC `get_object_by_key` serves relocated versions, then returns NotFound after the bucket drops; verify state-hash consistency with an untouched peer.
