# Sync without re-execution

## Summary

- Fullnodes re-execute every transaction while syncing. Measured on testnet, this caps a 16-core / 128 GB / NVMe node at ~950 tx/s while 85% of the CPU is idle and the NVMe array sits at ~6% utilisation.
- Everything a syncing fullnode needs is already committed by the quorum-signed checkpoint summary: transactions, effects, events, and the contents of every created or mutated object.
- The checkpoint archive already ships all of it. `StateSyncWorker` downloads it, verifies it, discards the objects and events, and then re-executes to reproduce them.
- Proposal: when syncing from the checkpoint archive, apply the verified results instead of executing. This becomes the default for archive sync, with a flag to force the old behaviour.
- **No protocol change is required, and no new trust root is introduced.** No new apply path either — the commit path already consumes a single structure that can be built from streamed data.
- Stage timings put the pipeline-limited ceiling at roughly **7×**, though write throughput and compaction will bind before that.

## Motivation

Measured on a testnet fullnode at epoch 612–613 (Ryzen 9 7950X3D 16c/32t, 128 GB RAM, 2× Samsung PM9A3 NVMe RAID1, 225 GB `perpetual`, 367 M keys in the objects table):

| | measured |
|---|---|
| throughput | 101 checkpoints/s, 950 tx/s, 6959 input objects/s |
| CPU | 6.68 of 16 cores busy; 85% idle |
| disk | md2 at 5.9–16% util; `r_await` 0.14 ms |
| objects table read latency | 4.2 µs per multiget |
| DB time per transaction | 16 µs of a 16.8 ms wall time (**0.1%**) |
| `WaitForTransactions` stage | ~60% of pipeline time, 99% wall-clock occupancy |
| distance to chain head | 14.85 M checkpoints → ~41 h at 101 cp/s |

Two conclusions:

- **Storage is not the constraint.** The 225 GB objects table costs 4.2 µs per read and near-zero disk I/O. RocksDB tuning, block-cache sizing, and adapting settings to table size were investigated and are worth at most a rounding error here.
- **The constraint is execution, and it does not use the machine.** Raising `checkpoint_execution_max_concurrency` from 4 to 32 bought +18% tx/s (the executor has 9 strictly ordered pipeline stages, so 4 buffered checkpoints can never fill it). Beyond that, `WaitForTransactions` dominates and CPU stays 85% idle. Swapping `TransactionManager` for `ExecutionScheduler` measured ~2.5× *worse* on a workload-normalised basis.

Not executing is the only lever that removes the bottleneck rather than widening it.

## What a checkpoint commits to

A checkpoint carries digests, not payloads. `CheckpointContents` is a list of:

```rust
pub struct CheckpointTransactionInfo {
    pub transaction: TransactionDigest,
    pub effects: TransactionEffectsDigest,
    pub signatures: Vec<UserSignature>,
}
```

But the digest chain reaches everything:

```
quorum-signed CheckpointSummary   (committee chained from genesis via EndOfEpochData)
  └─ contents_digest ──► CheckpointContents
       └─ per tx: (TransactionDigest, TransactionEffectsDigest)
            └─ TransactionEffects
                 ├─ events_digest ──────────────────────► TransactionEvents
                 └─ ChangedObject.output_state:
                      ObjectOut::ObjectWrite { digest }  ─► object contents
```

`ObjectDigest` is a salted hash over the **full BCS encoding of the `Object`**, `previous_transaction` and `storage_rebate` included, so per-object verification is complete rather than partial.

References (types live in the `iota-rust-sdk` sibling repo): `crates/iota-sdk-types/src/checkpoint.rs:148,470`, `crates/iota-sdk-types/src/effects/v1.rs:51,349`, `crates/iota-sdk-types/src/hash.rs:343`.

## Why fullnodes execute today

1. **Availability.** The p2p state sync service exposes only `push_checkpoint_summary`, `get_checkpoint_summary`, `get_checkpoint_availability`, `get_checkpoint_contents`, and `exchange_state_sync_handshake` (`crates/iota-network/src/state_sync/server.rs:68,105,128,149,167`). There is no endpoint for effects, events, or objects. Over p2p a node receives digests only, so it must recompute the payloads. This is an implementation gap, and it is why archive sync is the natural first target — the archive does not have this gap.
2. **Re-derivation of settled results.** Executing confirms that the certified effects are what the protocol rules produce; `assert_checkpoint_not_forked` (`crates/iota-core/src/checkpoints/checkpoint_executor/utils.rs`) calls `fatal!` on divergence.

## The trust question

Applying verified results does **not** change whom the node trusts.

- Today the node trusts genesis plus the committee signature chain, and additionally re-derives each transaction's effects.
- Under this proposal it trusts genesis plus the committee signature chain, and checks every payload against the digests those signatures commit to.

Same trusted parties, same signatures, same root. The only thing given up is local re-derivation of history that the network settled long before this node joined. For a supermajority to have certified invalid effects, the protocol would have had to break at the time — and a newly-syncing node halting today does not remedy that.

What re-execution is genuinely useful for is narrower than it looks: it answers *"does today's binary still reproduce the history older binaries produced?"* That catches execution-layer divergence and protocol-version handling bugs. That is real value, but it belongs in CI and on dedicated audit nodes, not on the critical path of every syncing fullnode. Hence: keep it available behind a flag, do not make every operator pay 41 hours of CPU for it.

The one property that must hold is that verification is **exhaustive** — every output object checked against its digest, every effects blob against its digest, every events blob against `events_digest`. A single unchecked object silently breaks coverage. That is an implementation risk, not a design one, and the epoch-boundary check below backstops it.

## What already exists

Nodes configured with `checkpoint-archive-config` fetch `.chk` files from the historical store (`crates/iota-data-ingestion-core/src/reader/v2.rs:65`). Those are full `CheckpointData` blobs, and `CheckpointTransaction` (`crates/iota-types/src/full_checkpoint_content.rs:150`) already carries exactly what is needed:

```rust
pub struct CheckpointTransaction {
    pub transaction: TransactionEnvelope,
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
    pub input_objects: Vec<Object>,
    pub output_objects: Vec<Object>,
}
```

`StateSyncWorker::process_checkpoint` (`crates/iota-network/src/state_sync/worker.rs:51`) reduces it to `FullCheckpointContents::from_contents_and_execution_data(...)` and verifies against `contents_digest`. `ExecutionData` is `{ transaction, effects }` (`crates/iota-types/src/base_types.rs:261`) — so **events and objects are dropped**, and the node re-executes to reproduce them. The reducer explicitly waits for execution before inserting (`crates/iota-network/src/state_sync/mod.rs:1386`).

The data needed to skip execution already arrives over the wire and is thrown away.

## Proposal

There is no need for a second apply path. The commit path already consumes a single structure:

```rust
pub struct TransactionOutputs {
    pub transaction, pub effects, pub events,
    pub markers, pub wrapped, pub deleted,
    pub live_object_markers_to_delete,
    pub new_live_object_markers_to_init,
    pub written,
}
```

`TransactionOutputs::build_transaction_outputs` (`crates/iota-core/src/transaction_outputs.rs:35`) derives every one of those fields from `effects`, the `transaction`, the written objects, and the input objects' owners. All four are present in `CheckpointTransaction`. So:

1. **Add a second constructor** for `TransactionOutputs` that takes a verified `CheckpointTransaction` instead of an `InnerTemporaryStore`. Everything downstream — `write_transaction_outputs`, the marker tables, the live-object markers, `BuildDbBatch`, `CommitTransactionOutputs` — is untouched and shared. Refactor `build_transaction_outputs` so the effects-derived logic is common to both constructors rather than duplicated.
2. **Keep the payloads** in the archive worker: stop discarding `events` and `output_objects`.
3. **Verify on ingest**: each object against its `ObjectDigest`, effects against the effects digest, events against `events_digest`, contents against `contents_digest`. Treat a mismatch the way a fork is treated today.
4. **Pass the downloaded `CheckpointData` straight through** to `process_checkpoint_data` instead of letting `load_checkpoint_data` rebuild it from the object store — we already hold it.
5. **Default this on for archive sync**, with a flag to force re-execution.

Two things need no special handling. The global state hash: `accumulate_checkpoint` already takes `&effects`, so the accumulator works unchanged. And shared object versions: the executor already assigns them from effects on this path (see below).

## Worth adding while we are here

The locally accumulated live-object-set hash is **never compared against the certified `EcmhLiveObjectSet` commitment** during sync today. `get_epoch_state_commitments` has only two callers: the snapshot uploader (`crates/iota-snapshot/src/uploader.rs:168`) and one e2e test.

This is not needed for the trust argument above — the per-object digests already cover correctness of the applied data. It is worth adding as a cheap self-check on our own apply and commit code: it would catch an object written to the wrong key, a missed deletion, or a gap in per-object verification coverage. The accumulator is already computed per checkpoint, so the comparison is nearly free. Useful on both the executing and applying paths.

## What would need a protocol change

Nothing above. The verification primitives already exist.

A protocol change is only needed for verification *cheaper* than re-hashing every object — for example a per-checkpoint Merkle root over output objects, which would allow verifying a subset of state or syncing partial state. A possible follow-up, not a prerequisite.

## Investigated: the four questions this raised

**Shared object versions need no separate handling.** `acquire_shared_version_assignments_from_effects` (`crates/iota-core/src/authority/authority_per_epoch_store.rs:2482`) is documented as "used by full nodes who don't listen to consensus, and validators who catch up by state sync". It derives every assignment from `effects.input_shared_objects()`, and the checkpoint executor already calls it in `schedule_transaction_execution` (`crates/iota-core/src/checkpoints/checkpoint_executor/mod.rs:935`) *before* enqueueing anything. The applying path keeps that call and skips only `enqueue_with_expected_effects_digest`. One ordering constraint carries over: `get_or_init_versions` initialises from the current version in the object store, so it must run before that checkpoint's writes are applied — which the sequence-ordered pipeline already guarantees.

**The indexes come along for free, and there is a bonus saving.** `process_checkpoint_data` calls `load_checkpoint_data` (`crates/iota-core/src/checkpoints/checkpoint_executor/data_ingestion_handler.rs:20`), which *rebuilds* `CheckpointData` by re-reading events from the transaction cache and input/output objects back out of the object store. On the applying path we already hold exactly that structure, so the reconstruction can be skipped entirely — `ProcessCheckpointData` (0.79–1.35 ms/cp) and its object re-reads disappear. Everything downstream consumes `CheckpointData` and is unaffected.

**Extending past archive sync needs no new service method.** A fullnode already streams full `CheckpointData` over gRPC on :50051 — `RemoteUrl::Fullnode` and `GrpcClient` in `iota-data-ingestion-core` are how the indexer and analytics indexer consume it today. So the same constructor extends to live sync from a trusted fullnode with no protocol work. What remains open is only whether the anemo state-sync *peer* protocol should carry it, which is a peer-selection and DoS-limit question rather than a data-availability gap.

**The earlier 2.5× estimate was wrong — and too pessimistic.** It summed stage times, but the pipeline stages are strictly ordered, so throughput is `1 / max(stage)`, not `1 / sum(stages)`. Removing `WaitForTransactions` makes `CommitTransactionOutputs` the new bottleneck:

| | current max stage | new max stage | ceiling |
|---|---|---|---|
| early segment (7.32 obj/tx) | 9.83 ms/cp | 1.47 ms/cp | 101 → **680 cp/s** |
| late segment (8.91 obj/tx) | 20.91 ms/cp | 2.81 ms/cp | 47.5 → **356 cp/s** |

That is a pipeline-limited **upper bound** of roughly 7×, and it will not be reached. At 680 cp/s the application write rate rises from ~46 MB/s to ~300 MB/s, and compaction traffic scales with the objects table's observed write amplification (interval W-Amp 19.4). The NVMe array will not sustain that.

So the practical ceiling moves to write throughput and compaction — which is worth stating plainly: **at the new operating point, RocksDB write-side tuning becomes the relevant work.** Target file size, level sizing, and blob storage for large object values would matter there, whereas at today's operating point they are worth a rounding error. That is a follow-up, not a blocker, but it is the next thing that will bind.

## Remaining unknowns

- Where exactly does the shared constructor belong so the effects-derived logic is genuinely shared rather than copied? This is the main code-review risk.
- Does bypassing `load_checkpoint_data` change behaviour at epoch boundaries, where `index_epoch_boundary` consumes the reconstructed data?
- Is the epoch-boundary live-object-set hash comparison affordable on a catching-up node, given it walks the live object set?

## Acceptance test

A node that has synced the same checkpoint range both ways should produce byte-identical tables. That comparison is the strongest guard against divergence between the two constructors and should exist before this becomes the default.
