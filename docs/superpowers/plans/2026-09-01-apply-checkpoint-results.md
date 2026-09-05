# Apply Checkpoint Results Without Re-execution — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When syncing from the checkpoint archive, write the verified transaction results straight into the store instead of re-executing every transaction.

**Architecture:** The checkpoint executor already skips any transaction whose effects are already in the store — it calls `multi_get_executed_effects_digests`, runs `assert_not_forked` on the ones it finds, and only enqueues the rest. So the archive sync path can write the downloaded results *before* the executor reaches that checkpoint, and the executor short-circuits on its own. **The checkpoint executor itself needs no changes.** The whole feature lives in the archive sync path plus one new `TransactionOutputs` constructor.

**Tech Stack:** Rust, RocksDB via `typed-store`, `iota-data-ingestion-core` (archive reader), `anemo` (state sync).

**Spec:** `SYNC_WITHOUT_REEXECUTION_PROPOSAL.md` (repo root)

**Branch:** `feat/state-sync-skip-execution-during-historic-sync`

## Global Constraints

- License header on every new file: `// Copyright (c) 2026 IOTA Stiftung` + `// SPDX-License-Identifier: Apache-2.0`. Files derived from Mysten code additionally keep the original `// Copyright (c) Mysten Labs, Inc.` line above it.
- **Never** use `#[allow(dead_code)]`, `#[allow(unused)]`, or any lint suppression to silence a warning.
- **Never** disable or skip a test.
- Lint with `cargo ci-clippy`; format with `cargo +nightly fmt`; TOML/MD/YAML with `dprint fmt`.
- Comments follow `RUST_CONVENTIONS.md`: doc comments address the caller; inline comments explain a non-obvious *why*, never a *what*; no PR/issue numbers or change history in code.
- No coined terminology. Describe things in the words the codebase already uses.
- `crates/iota-network` must **not** gain a dependency on `crates/iota-core` — the dependency runs the other way (`iota-core/Cargo.toml:68`). Anything needing `iota-core` types is reached through a trait defined in `iota-types`.
- The end-of-epoch (change-epoch) transaction is **never** applied from streamed data. It stays on the execution path. It is one transaction per epoch and it drives reconfiguration.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/iota-core/src/transaction_outputs.rs` (modify) | Gains a shared effects-derived helper and a second constructor taking a verified `CheckpointTransaction`. |
| `crates/iota-types/src/storage/apply_checkpoint_results.rs` (create) | Defines the `ApplyCheckpointResults` trait so `iota-network` can call into `iota-core` without depending on it. |
| `crates/iota-types/src/full_checkpoint_content.rs` (modify) | Gains `verify_payload_digests`, checking objects and events against the digests inside effects. |
| `crates/iota-core/src/checkpoint_results_applier.rs` (create) | `CheckpointResultsApplier` — the only implementor of `ApplyCheckpointResults`. Holds the cache writer, object cache reader and epoch store. Kept out of `RocksDbStore`, which is a thin adapter over caches and stores and should not take on execution-adjacent responsibilities. |
| `crates/iota-node/src/lib.rs` (modify) | Constructs the applier and hands it to `state_sync::Builder`. |
| `crates/iota-network/src/state_sync/worker.rs` (modify) | Stops discarding the payloads; reducer applies them. |
| `crates/iota-config/src/node.rs` (modify) | Adds `re-execute-archived-checkpoints` to `CheckpointArchiveConfig`. |

---

### Task 1: Extract the effects-derived logic in `build_transaction_outputs`

Pure refactor, no behaviour change. `build_transaction_outputs` derives `markers`, `wrapped`, `deleted`, `live_object_markers_to_delete` and `new_live_object_markers_to_init` from the effects, the transaction and the written objects. Task 2 needs that same logic. Extract it first so there is one copy, not two.

**Files:**
- Modify: `crates/iota-core/src/transaction_outputs.rs:35-142`
- Test: `crates/iota-core/src/unit_tests/transaction_outputs_tests.rs` (create)
- Modify: `crates/iota-core/src/transaction_outputs.rs` (add `#[cfg(test)] #[path = ...] mod` declaration)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `fn derive_store_updates(transaction: &VerifiedTransaction, effects: &TransactionEffects, written: &WrittenObjects, mutable_inputs: &BTreeMap<ObjectId, (VersionDigest, Owner)>, input_owners: &dyn Fn(&ObjectId) -> Option<Owner>, lamport_version: Version) -> StoreUpdates` where `struct StoreUpdates { markers: Vec<(ObjectKey, MarkerValue)>, wrapped: Vec<ObjectKey>, deleted: Vec<ObjectKey>, live_object_markers_to_delete: Vec<ObjectReference>, new_live_object_markers_to_init: Vec<ObjectReference> }`. Both are private to the module (`pub(crate)` is not needed).

- [ ] **Step 1: Write a characterization test that pins current behaviour**

This is a refactor, so the test must pass *before* the refactor and still pass after. Write it against the existing public constructor.

```rust
// crates/iota-core/src/unit_tests/transaction_outputs_tests.rs
// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::authority::test_authority_builder::TestAuthorityBuilder;

/// `build_transaction_outputs` must derive the store updates from the effects:
/// a transfer of an address-owned coin retires the input's live-object marker
/// and initialises one for the output.
#[tokio::test]
async fn build_transaction_outputs_derives_live_object_markers() {
    let (outputs, sender_coin_id) = execute_one_transfer_for_testing().await;

    assert_eq!(
        outputs.live_object_markers_to_delete.len(),
        1,
        "the transferred coin's old marker must be retired"
    );
    assert_eq!(
        outputs.live_object_markers_to_delete[0].object_id, sender_coin_id,
        "the retired marker must be the transferred coin"
    );
    assert!(
        !outputs.new_live_object_markers_to_init.is_empty(),
        "the written address-owned output must get a marker"
    );
    assert!(outputs.wrapped.is_empty(), "a transfer wraps nothing");
    assert!(outputs.deleted.is_empty(), "a transfer deletes nothing");
}
```

Implement `execute_one_transfer_for_testing` in the same file using `TestAuthorityBuilder`, following the setup already used in `crates/iota-core/src/unit_tests/authority_tests.rs`. Read that file's transfer tests first and copy their fixture construction rather than inventing one.

Register the module in `crates/iota-core/src/transaction_outputs.rs`, matching the pattern already used in `crates/iota-core/src/execution_driver.rs:18-19`:

```rust
#[cfg(test)]
#[path = "unit_tests/transaction_outputs_tests.rs"]
mod transaction_outputs_tests;
```

- [ ] **Step 2: Run the test to verify it passes against unrefactored code**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core --lib transaction_outputs`
Expected: PASS. If it fails, the test's assumptions are wrong — fix the test before refactoring, not after.

- [ ] **Step 3: Commit the test on its own**

```bash
git add crates/iota-core/src/unit_tests/transaction_outputs_tests.rs crates/iota-core/src/transaction_outputs.rs
git commit -m "test(iota-core): pin build_transaction_outputs store-update derivation"
```

- [ ] **Step 4: Extract the helper**

In `crates/iota-core/src/transaction_outputs.rs`, add above `impl TransactionOutputs`:

```rust
/// The parts of [`TransactionOutputs`] that follow from the effects rather
/// than from execution's temporary store.
struct StoreUpdates {
    markers: Vec<(ObjectKey, MarkerValue)>,
    wrapped: Vec<ObjectKey>,
    deleted: Vec<ObjectKey>,
    live_object_markers_to_delete: Vec<ObjectReference>,
    new_live_object_markers_to_init: Vec<ObjectReference>,
}
```

Move the body of `build_transaction_outputs` between `let deleted: HashMap<_, _> = effects.all_tombstones()...` and the `let wrapped = ...` line into:

```rust
/// Derives the store updates a transaction implies.
///
/// `input_owner` answers what owned an object before the transaction, which
/// decides whether a deletion is recorded as owned or shared. Callers that
/// executed the transaction pass the input objects they loaded; callers
/// applying streamed results resolve it from the effects' input state.
fn derive_store_updates(
    transaction: &VerifiedTransaction,
    effects: &TransactionEffects,
    written: &WrittenObjects,
    mutable_inputs: &BTreeMap<ObjectId, (VersionDigest, Owner)>,
    input_owner: &dyn Fn(&ObjectId) -> Option<Owner>,
    lamport_version: Version,
) -> StoreUpdates {
    // ... moved body, with `input_objects.get(&object_id).is_some_and(|o| o.is_shared())`
    // replaced by `input_owner(&object_id).is_some_and(|o| o.is_shared())`
}
```

Then make `build_transaction_outputs` call it, passing an `input_owner` closure backed by its `input_objects` map:

```rust
let updates = derive_store_updates(
    &transaction,
    &effects,
    &written,
    &mutable_inputs,
    &|id| input_objects.get(id).map(|o| o.owner().clone()),
    lamport_version,
);
```

Do not change any derivation logic. This step only moves code.

- [ ] **Step 5: Run the test and the surrounding suite**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core --lib`
Expected: PASS, including the Step 1 test and every existing `iota-core` unit test.

- [ ] **Step 6: Lint and commit**

```bash
cargo ci-clippy -p iota-core && cargo +nightly fmt
git add crates/iota-core/src/transaction_outputs.rs
git commit -m "refactor(iota-core): share the effects-derived store updates in TransactionOutputs"
```

---

### Task 2: Add a `TransactionOutputs` constructor for verified checkpoint data

Everything `TransactionOutputs` needs is in the effects plus the written objects. `effects.old_object_metadata()` supplies the input version, digest and owner per changed object — which is exactly `mutable_inputs`. `effects.lamport_version()` supplies the lamport version. So no execution and no `InnerTemporaryStore` is required.

**Files:**
- Modify: `crates/iota-core/src/transaction_outputs.rs`
- Test: `crates/iota-core/src/unit_tests/transaction_outputs_tests.rs`

**Interfaces:**
- Consumes: `derive_store_updates` and `StoreUpdates` from Task 1.
- Produces: `pub fn TransactionOutputs::build_from_checkpoint_transaction(tx: &CheckpointTransaction) -> IotaResult<TransactionOutputs>`.

- [ ] **Step 1: Write the failing test**

The strongest available test is equivalence: build the outputs both ways for the same transaction and assert they match. Add to `transaction_outputs_tests.rs`:

```rust
/// Building `TransactionOutputs` from streamed checkpoint data must produce
/// exactly what execution produces, since both paths feed the same commit.
#[tokio::test]
async fn build_from_checkpoint_transaction_matches_execution() {
    let (executed, checkpoint_tx) = execute_one_transfer_and_capture_checkpoint_tx().await;

    let applied = TransactionOutputs::build_from_checkpoint_transaction(&checkpoint_tx)
        .expect("streamed data is well formed");

    assert_eq!(applied.effects, executed.effects);
    assert_eq!(applied.events, executed.events);
    assert_eq!(applied.written, executed.written);
    assert_eq!(sorted(applied.markers), sorted(executed.markers));
    assert_eq!(sorted(applied.wrapped), sorted(executed.wrapped));
    assert_eq!(sorted(applied.deleted), sorted(executed.deleted));
    assert_eq!(
        sorted(applied.live_object_markers_to_delete),
        sorted(executed.live_object_markers_to_delete)
    );
    assert_eq!(
        sorted(applied.new_live_object_markers_to_init),
        sorted(executed.new_live_object_markers_to_init)
    );
}

/// Sorts a vec so the comparison does not depend on iteration order, which
/// differs between the two constructors.
fn sorted<T: Ord>(mut v: Vec<T>) -> Vec<T> {
    v.sort();
    v
}
```

`execute_one_transfer_and_capture_checkpoint_tx` builds the `CheckpointTransaction` from the executed transaction's own effects, events and written objects — the same shape the archive ships.

- [ ] **Step 2: Run the test to verify it fails**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core --lib build_from_checkpoint_transaction`
Expected: FAIL to compile — `no function or associated item named 'build_from_checkpoint_transaction'`.

- [ ] **Step 3: Implement the constructor**

```rust
/// Builds the store updates for a transaction whose results came from a
/// verified checkpoint rather than from local execution.
///
/// The caller must have verified `tx` against the checkpoint's digests
/// (see `CheckpointData::verify_payload_digests`); this function trusts the
/// effects and the objects it is handed.
pub fn build_from_checkpoint_transaction(tx: &CheckpointTransaction) -> IotaResult<Self> {
    let transaction = VerifiedTransaction::new_unchecked(tx.transaction.clone());
    let effects = tx.effects.clone();
    let lamport_version = effects.lamport_version();

    // `old_object_metadata` is the input version, digest and owner of every
    // changed object, which is what execution records as `mutable_inputs`.
    let mutable_inputs: BTreeMap<ObjectId, (VersionDigest, Owner)> = effects
        .old_object_metadata()
        .into_iter()
        .map(|owned_ref| {
            let obj_ref = owned_ref.reference();
            (
                obj_ref.object_id,
                ((obj_ref.version, obj_ref.digest), owned_ref.owner().clone()),
            )
        })
        .collect();

    let written: WrittenObjects = tx
        .output_objects
        .iter()
        .map(|o| (o.id(), o.clone()))
        .collect();

    // An empty events blob is represented as `None` in checkpoint data and as
    // an empty `TransactionEvents` in the store.
    let events = tx.events.clone().unwrap_or_default();

    let updates = derive_store_updates(
        &transaction,
        &effects,
        &written,
        &mutable_inputs,
        &|id| mutable_inputs.get(id).map(|(_, owner)| owner.clone()),
        lamport_version,
    );

    Ok(TransactionOutputs {
        transaction: Arc::new(transaction),
        effects,
        events,
        markers: updates.markers,
        wrapped: updates.wrapped,
        deleted: updates.deleted,
        live_object_markers_to_delete: updates.live_object_markers_to_delete,
        new_live_object_markers_to_init: updates.new_live_object_markers_to_init,
        written,
    })
}
```

If the borrow checker objects to `mutable_inputs` being both borrowed by the closure and passed by reference, bind the owner map separately before the call.

- [ ] **Step 4: Run the test to verify it passes**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core --lib transaction_outputs`
Expected: PASS.

If the two constructors disagree on a field, do **not** loosen the assertion. A mismatch means the derivation differs, and that is the bug this task exists to prevent.

- [ ] **Step 5: Extend the test to a shared-object deletion**

The owned-vs-shared decision inside `derive_store_updates` is the one place where the two constructors resolve the input owner differently, so it needs its own case. Follow the fixtures in `crates/iota-core/src/unit_tests/shared_object_deletion_tests.rs`.

```rust
/// The owned-vs-shared marker decision reads the input owner, which the two
/// constructors resolve from different sources. A shared-object deletion must
/// still be recorded as `SharedDeleted` on the applying path.
#[tokio::test]
async fn build_from_checkpoint_transaction_marks_shared_deletion() {
    let (executed, checkpoint_tx) = execute_shared_object_deletion_and_capture().await;

    let applied = TransactionOutputs::build_from_checkpoint_transaction(&checkpoint_tx)
        .expect("streamed data is well formed");

    assert_eq!(sorted(applied.markers), sorted(executed.markers));
    assert!(
        applied
            .markers
            .iter()
            .any(|(_, v)| matches!(v, MarkerValue::SharedDeleted(_))),
        "a shared-object deletion must be marked SharedDeleted, not OwnedDeleted"
    );
}
```

- [ ] **Step 6: Run, lint, commit**

```bash
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core --lib transaction_outputs
cargo ci-clippy -p iota-core && cargo +nightly fmt
git add crates/iota-core/src/transaction_outputs.rs crates/iota-core/src/unit_tests/transaction_outputs_tests.rs
git commit -m "feat(iota-core): build TransactionOutputs from verified checkpoint data"
```

---

### Task 3: Verify the payloads inside downloaded checkpoint data

`StateSyncWorker` already verifies the summary's signatures and `contents_digest`. `contents_digest` covers the transaction and effects digests, so effects are already pinned. What is not yet checked is the object contents and the events, which are pinned by digests *inside* effects.

**Files:**
- Modify: `crates/iota-types/src/full_checkpoint_content.rs`
- Test: `crates/iota-types/src/full_checkpoint_content.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub fn CheckpointData::verify_payload_digests(&self) -> Result<(), StorageError>`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Well-formed checkpoint data passes.
    #[test]
    fn verify_payload_digests_accepts_untampered_data() {
        let data = checkpoint_data_fixture();
        assert!(data.verify_payload_digests().is_ok());
    }

    /// An output object whose contents do not hash to the digest recorded in
    /// the effects must be rejected — this is the check that lets the node
    /// trust streamed objects.
    #[test]
    fn verify_payload_digests_rejects_tampered_object() {
        let mut data = checkpoint_data_fixture();
        tamper_with_first_output_object(&mut data);
        let err = data
            .verify_payload_digests()
            .expect_err("a tampered object must be rejected");
        assert!(
            format!("{err}").contains("object digest mismatch"),
            "error must name the failure: {err}"
        );
    }

    /// Effects that declare events but arrive without them must be rejected;
    /// otherwise the events table would silently lose rows.
    #[test]
    fn verify_payload_digests_rejects_missing_events() {
        let mut data = checkpoint_data_fixture();
        drop_events_from_first_transaction(&mut data);
        assert!(data.verify_payload_digests().is_err());
    }
}
```

Build `checkpoint_data_fixture` from an existing fixture if one is available — check `crates/iota-kvstore/tests/kv_worker.rs`, which constructs `CheckpointData` — otherwise construct it by hand in the test module.

- [ ] **Step 2: Run to verify it fails**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-types --lib full_checkpoint_content`
Expected: FAIL to compile — no `verify_payload_digests`.

- [ ] **Step 3: Implement**

```rust
impl CheckpointData {
    /// Checks the payloads against the digests the checkpoint commits to.
    ///
    /// The caller must separately have verified the summary's authority
    /// signatures and its `contents_digest`, which is what pins the effects
    /// this function reads its expected digests from.
    ///
    /// Every output object is checked. A caller that skips any of them loses
    /// the guarantee entirely, so this deliberately offers no partial mode.
    pub fn verify_payload_digests(&self) -> Result<(), StorageError> {
        for tx in &self.transactions {
            let expected: BTreeMap<ObjectId, ObjectDigest> = tx
                .effects
                .all_changed_objects()
                .into_iter()
                .map(|(owned_ref, _)| {
                    let r = owned_ref.reference();
                    (r.object_id, r.digest)
                })
                .collect();

            for object in &tx.output_objects {
                let Some(want) = expected.get(&object.id()) else {
                    return Err(StorageError::custom(format!(
                        "output object {} is not a changed object in the effects",
                        object.id()
                    )));
                };
                let got = object.digest();
                if got != *want {
                    return Err(StorageError::custom(format!(
                        "object digest mismatch for {}: effects say {want}, contents hash to {got}",
                        object.id()
                    )));
                }
            }

            match (tx.effects.events_digest(), &tx.events) {
                (Some(want), Some(events)) => {
                    let got = events.digest();
                    if got != *want {
                        return Err(StorageError::custom(format!(
                            "events digest mismatch for transaction {}: effects say {want}, \
                             contents hash to {got}",
                            tx.effects.transaction_digest()
                        )));
                    }
                }
                (Some(_), None) => {
                    return Err(StorageError::custom(format!(
                        "transaction {} declares events but none were provided",
                        tx.effects.transaction_digest()
                    )));
                }
                (None, Some(events)) if !events.is_empty() => {
                    return Err(StorageError::custom(format!(
                        "transaction {} provided events but its effects declare none",
                        tx.effects.transaction_digest()
                    )));
                }
                _ => {}
            }
        }
        Ok(())
    }
}
```

Check the exact constructor for `StorageError` in `crates/iota-types/src/storage/error.rs` and use whatever it provides instead of `custom` if the name differs.

- [ ] **Step 4: Run to verify it passes**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-types --lib full_checkpoint_content`
Expected: PASS, all three tests.

- [ ] **Step 5: Lint and commit**

```bash
cargo ci-clippy -p iota-types && cargo +nightly fmt
git add crates/iota-types/src/full_checkpoint_content.rs
git commit -m "feat(iota-types): verify object and event digests in checkpoint data"
```

---

### Task 4: Define the apply trait and its implementor in `iota-core`

`iota-network` cannot depend on `iota-core`, so the state sync reducer reaches the apply logic through a trait defined in `iota-types`, in the same place `WriteStore` lives.

The implementor is a **new type**, `CheckpointResultsApplier`, not `RocksDbStore`. `RocksDbStore` is a thin adapter over the caches, committee store and checkpoint store; the apply path needs an `AuthorityPerEpochStore` handle to do the shared-version assignment, and giving `RocksDbStore` execution-adjacent responsibilities to get it would muddy that boundary.

`acquire_shared_version_assignments_from_effects` needs the epoch store because execution does this assignment for every transaction it schedules — without it the epoch's `next_shared_object_versions` rows stop advancing.

The reducer holds the applier as `Option<Arc<dyn ApplyCheckpointResults + Send + Sync>>` rather than as a second generic bound, so `S: WriteStore` is untouched and `None` means "re-execute".

**Files:**
- Create: `crates/iota-types/src/storage/apply_checkpoint_results.rs`
- Modify: `crates/iota-types/src/storage/mod.rs` (declare and re-export the module)
- Create: `crates/iota-core/src/checkpoint_results_applier.rs`
- Modify: `crates/iota-core/src/lib.rs` (declare the module)
- Modify: `crates/iota-node/src/lib.rs` (construct the applier, pass it to `state_sync::Builder`)
- Test: `crates/iota-core/src/unit_tests/apply_checkpoint_results_tests.rs` (create)

**Interfaces:**
- Consumes: `TransactionOutputs::build_from_checkpoint_transaction` (Task 2), `CheckpointData::verify_payload_digests` (Task 3).
- Produces: `pub trait ApplyCheckpointResults { fn try_apply_checkpoint_results(&self, checkpoint: &CheckpointData) -> Result<(), StorageError>; }` in `iota-types`; `pub struct CheckpointResultsApplier` in `iota-core` with `pub fn new(cache_writer: Arc<dyn ExecutionCacheWrite>, object_cache_reader: Arc<dyn ObjectCacheRead>, epoch_store: <the node's epoch-store handle type>) -> Self`.

- [ ] **Step 1: Write the failing test**

```rust
// crates/iota-core/src/unit_tests/apply_checkpoint_results_tests.rs
// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Applying a checkpoint's results must leave the objects readable and the
/// effects recorded, so the checkpoint executor finds the transactions already
/// executed and skips them.
#[tokio::test]
async fn apply_checkpoint_results_writes_objects_and_effects() {
    let (store, checkpoint_data, expected_object) = fixture_with_one_transfer().await;

    store
        .try_apply_checkpoint_results(&checkpoint_data)
        .expect("applying verified results must succeed");

    let tx_digest = checkpoint_data.transactions[0].effects.transaction_digest();
    assert!(
        store.get_executed_effects_digest(tx_digest).is_some(),
        "effects must be recorded so the executor treats the tx as executed"
    );
    let stored = store
        .get_objects(&[ObjectKey(expected_object.id(), expected_object.version())])
        .expect("store read")
        .pop()
        .flatten()
        .expect("output object must be readable");
    assert_eq!(stored.digest(), expected_object.digest());
}

/// Tampered data must be rejected before anything is written.
#[tokio::test]
async fn apply_checkpoint_results_rejects_tampered_data() {
    let (store, mut checkpoint_data, _) = fixture_with_one_transfer().await;
    tamper_with_first_output_object(&mut checkpoint_data);

    assert!(store.try_apply_checkpoint_results(&checkpoint_data).is_err());

    let tx_digest = checkpoint_data.transactions[0].effects.transaction_digest();
    assert!(
        store.get_executed_effects_digest(tx_digest).is_none(),
        "a rejected checkpoint must not have written anything"
    );
}

/// The end-of-epoch transaction stays on the execution path.
#[tokio::test]
async fn apply_checkpoint_results_skips_end_of_epoch_transaction() {
    let (store, checkpoint_data) = fixture_with_epoch_boundary().await;

    store
        .try_apply_checkpoint_results(&checkpoint_data)
        .expect("applying must succeed");

    let change_epoch = checkpoint_data
        .end_of_epoch_transaction()
        .expect("boundary checkpoint has one");
    assert!(
        store
            .get_executed_effects_digest(change_epoch.effects.transaction_digest())
            .is_none(),
        "the change-epoch tx must be left for the executor"
    );
}
```

Build the store with `TestAuthorityBuilder`; read `crates/iota-core/src/unit_tests/authority_tests.rs` for how it wires the caches and epoch store.

- [ ] **Step 2: Run to verify it fails**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core --lib apply_checkpoint_results`
Expected: FAIL to compile — no trait, no method.

- [ ] **Step 3: Define the trait**

```rust
// crates/iota-types/src/storage/apply_checkpoint_results.rs
// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{full_checkpoint_content::CheckpointData, storage::error::Error as StorageError};

/// A store that can commit a checkpoint's results without re-executing its
/// transactions.
///
/// Implementors must verify the payloads against the checkpoint's digests
/// before writing anything, and must write all of a checkpoint's results or
/// none of them.
pub trait ApplyCheckpointResults {
    fn try_apply_checkpoint_results(
        &self,
        checkpoint: &CheckpointData,
    ) -> Result<(), StorageError>;
}
```

Declare and re-export it from `crates/iota-types/src/storage/mod.rs` alongside `WriteStore`.

- [ ] **Step 4: Create `CheckpointResultsApplier`**

New file `crates/iota-core/src/checkpoint_results_applier.rs`, declared in `crates/iota-core/src/lib.rs`.

```rust
/// Commits the results of checkpoints whose payloads have been verified
/// against the checkpoint's digests, without re-executing their transactions.
pub struct CheckpointResultsApplier {
    cache_writer: Arc<dyn ExecutionCacheWrite>,
    object_cache_reader: Arc<dyn ObjectCacheRead>,
    epoch_store: <the node's epoch-store handle type>,
}

impl CheckpointResultsApplier {
    pub fn new(
        cache_writer: Arc<dyn ExecutionCacheWrite>,
        object_cache_reader: Arc<dyn ObjectCacheRead>,
        epoch_store: <the node's epoch-store handle type>,
    ) -> Self {
        Self { cache_writer, object_cache_reader, epoch_store }
    }
}
```

For the epoch-store handle, reuse whatever `AuthorityState` already uses to share the current epoch store across reconfiguration — look for the `ArcSwap<AuthorityPerEpochStore>` or equivalent accessor in `crates/iota-core/src/authority.rs`. Do not introduce a new sharing mechanism, and do not hold a plain `Arc<AuthorityPerEpochStore>`: it would go stale at the epoch boundary.

- [ ] **Step 5: Implement the trait**

```rust
// in crates/iota-core/src/checkpoint_results_applier.rs
impl ApplyCheckpointResults for CheckpointResultsApplier {
    fn try_apply_checkpoint_results(
        &self,
        checkpoint: &CheckpointData,
    ) -> Result<(), StorageError> {
        // Verify before writing: a mismatch must leave the store untouched.
        checkpoint.verify_payload_digests()?;

        let epoch_store = self.epoch_store.load();
        let epoch_id = epoch_store.epoch();

        for tx in &checkpoint.transactions {
            // Reconfiguration stays on the execution path.
            if tx.transaction.transaction().is_end_of_epoch_tx() {
                continue;
            }

            let outputs = TransactionOutputs::build_from_checkpoint_transaction(tx)?;

            // Execution assigns shared versions for every transaction it
            // schedules; without the same call here the epoch's
            // `next_shared_object_versions` rows stop advancing.
            if tx.transaction.transaction().contains_shared_object() {
                epoch_store.acquire_shared_version_assignments_from_effects(
                    &VerifiedExecutableTransaction::new_from_checkpoint(
                        VerifiedTransaction::new_unchecked(tx.transaction.clone()),
                        epoch_id,
                        checkpoint.checkpoint_summary.sequence_number,
                    ),
                    &tx.effects,
                    self.object_cache_reader.as_ref(),
                )?;
            }

            self.cache_writer
                .write_transaction_outputs(epoch_id, Arc::new(outputs));
        }
        Ok(())
    }
}
```

Confirmed already: `ExecutionCacheTraitPointers` exposes `cache_writer: Arc<dyn ExecutionCacheWrite>` and `object_cache_reader: Arc<dyn ObjectCacheRead>` (`crates/iota-core/src/execution_cache.rs:108-110`), and the constructor is `VerifiedExecutableTransaction::new_from_checkpoint(tx, epoch, checkpoint_seq)` (`crates/iota-core/src/authority.rs:5430`). The node builds the applier from those two pointers plus its epoch-store handle.

Note `write_transaction_outputs` is infallible and panics on a storage error, matching how execution's commit path calls it. Use `try_write_transaction_outputs` instead if the reducer should surface the error rather than abort the process — decide when wiring Task 5 and keep it consistent with what the surrounding archive-sync code does with storage failures.

- [ ] **Step 6: Run the tests**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core --lib apply_checkpoint_results`
Expected: PASS, all three tests.

- [ ] **Step 7: Build the workspace, lint, commit**

```bash
cargo check -p iota-core -p iota-types -p iota-node
cargo ci-clippy -p iota-core -p iota-types && cargo +nightly fmt
git add crates/iota-types/src/storage/ crates/iota-core/src/storage.rs crates/iota-core/src/unit_tests/apply_checkpoint_results_tests.rs crates/iota-node/src/lib.rs
git commit -m "feat(iota-core): apply verified checkpoint results without executing"
```

---

### Task 5: Wire it into archive sync behind a config flag, default off

The feature ships disabled so the acceptance test in Task 6 can compare both paths on the same node.

**Files:**
- Modify: `crates/iota-config/src/node.rs` (`CheckpointArchiveConfig`)
- Modify: `crates/iota-network/src/state_sync/worker.rs` (keep payloads, apply in reducer)
- Modify: `crates/iota-network/src/state_sync/mod.rs:1384-1406` (pass the flag through)
- Test: `crates/iota-network/src/state_sync/tests.rs`

**Interfaces:**
- Consumes: `ApplyCheckpointResults` (Task 4).
- Produces: `CheckpointArchiveConfig::re_execute_archived_checkpoints: bool`; `VerifiedArchiveCheckpoint::data: Arc<CheckpointData>`.

- [ ] **Step 1: Add the config field**

In `crates/iota-config/src/node.rs`, on `CheckpointArchiveConfig`:

```rust
/// Re-execute every transaction in checkpoints downloaded from the archive
/// instead of applying their verified results.
///
/// Applying is much faster and relies on the same authority signatures, so
/// it is the default. Enable this to have the node independently re-derive
/// the effects, which detects a divergence between this binary's execution
/// and the history the network certified.
#[serde(default)]
pub re_execute_archived_checkpoints: bool,
```

Leave the default as `false` in the struct but do **not** flip the runtime behaviour yet — Step 3 gates on the flag with applying disabled via an explicit `false` at the call site until Task 7.

- [ ] **Step 2: Keep the payloads in the worker**

In `crates/iota-network/src/state_sync/worker.rs`, add a field to `VerifiedArchiveCheckpoint`:

```rust
/// The verified checkpoint data, retained so the reducer can apply the
/// results instead of having them re-executed.
data: Arc<CheckpointData>,
```

`process_checkpoint` already receives `checkpoint: Arc<CheckpointData>`. It currently moves clones of the summary and contents into `spawn_blocking`; clone the `Arc` before that and set the field on the returned value.

- [ ] **Step 3: Apply in the reducer**

Add to `StateSyncReducer<S>`:

```rust
/// Applies each checkpoint's verified results so its transactions do not
/// need re-executing. `None` leaves them to the checkpoint executor.
pub(crate) results_applier: Option<Arc<dyn ApplyCheckpointResults + Send + Sync>>,
```

A trait object rather than a second generic keeps the `S: WriteStore` bound untouched.

**Wait for the batch's epoch before applying it.** Object markers and shared object version assignments are stored per epoch, and archive sync inserts up to `max_checkpoints_ahead_of_execution` (default 100 000) ahead of execution while a testnet epoch is roughly 377 000 checkpoints — so it regularly reaches the next epoch before reconfiguration. Waiting keeps those checkpoints on the applying path; skipping them would hand roughly a quarter of each epoch back to the executor.

This cannot deadlock. `should_close_batch` already closes a batch at an epoch boundary, so a batch of epoch N+1 checkpoints never contains epoch N's last checkpoint — that was inserted by an earlier batch, and the executor can therefore reach it and reconfigure while this call waits.

In `commit`, after `wait_for_execution_to_catch_up` and before the per-checkpoint loop:

```rust
if let Some(applier) = &self.results_applier {
    // Batches never span epochs, so the first checkpoint's epoch is the
    // batch's epoch.
    if let Some(first) = batch.first() {
        applier
            .wait_for_epoch(first.data.checkpoint_summary.data().epoch)
            .await;
    }
}
```

Then, after `verify_against_previous` succeeds for a checkpoint and **before** `try_insert_synced_checkpoints`:

```rust
if let Some(applier) = &self.results_applier {
    // Written before the summary is inserted so the executor never sees a
    // checkpoint whose results are still missing. The reverse order would
    // also be correct — the executor falls back to executing — but this
    // avoids the wasted work.
    applier
        .try_apply_checkpoint_results(&message.data)
        .map_err(|e| anyhow!("failed to apply checkpoint results: {e}"))?;
}
```

Count the `false` returns in a metric: the applier declining to apply should be rare once the wait is in place, and a rising count means checkpoints are silently falling back to execution.

Thread the applier from `state_sync::Builder` (the node supplies it, `None` when `re_execute_archived_checkpoints` is set) down to where `StateSyncReducer` is constructed in `crates/iota-network/src/state_sync/mod.rs` (around line 1399).

- [ ] **Step 4: Write the integration test**

```rust
/// With applying enabled, archive sync must advance the executed watermark
/// without the executor running any transactions.
#[tokio::test]
async fn archive_sync_applies_results_without_executing() {
    // Build a fixture archive of N checkpoints, sync it with `re_execute:
    // false`, and assert the executed watermark reaches N.
}

/// With re-execution forced, the same range must produce the same watermark.
#[tokio::test]
async fn archive_sync_re_executes_when_forced() {
    // Same fixture, `re_execute: true`, same resulting watermark.
}
```

Follow the existing archive-sync test setup in `crates/iota-network/src/state_sync/tests.rs`. If no archive fixture exists there yet, build one with a temp directory as the `file://` historical store — `RemoteStore::new` accepts a `file://` prefix (`crates/iota-data-ingestion-core/src/reader/v2.rs`).

- [ ] **Step 5: Run and commit**

```bash
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-network -p iota-config
cargo ci-clippy -p iota-network -p iota-config && cargo +nightly fmt && dprint fmt
git add crates/iota-config/src/node.rs crates/iota-network/src/state_sync/
git commit -m "feat(state-sync): apply archived checkpoint results behind a config flag"
```

---

### Task 6: Acceptance test — the two constructors agree across real traffic

This is the gate for flipping the default. Nothing else in the plan proves the two constructors agree beyond the two hand-built fixtures in Task 2.

**Compare `TransactionOutputs`, not database tables.** Both paths write through the same `write_transaction_outputs(epoch_id, TransactionOutputs)`, so equal outputs mean equal stored state by construction. Comparing the structs needs one node instead of two, no archive fixture, and no table dump — and it localises a failure to the exact transaction and field, where a table diff would only say "column family X differs at key Y".

An in-memory store is not an option here and would not help: `AuthorityPerpetualTables` is `#[derive(DBMapUtils)]` and RocksDB-bound, and `InMemoryStorage` is only a `BTreeMap<ObjectId, Object>` with no marker, effects, or events tables. Tests already run RocksDB in a `tempdir()` cheaply — the cost in the original design was the two-fullnode archive sync, not the storage engine.

**Files:**
- Test: `crates/iota-e2e-tests/tests/apply_checkpoint_results_tests.rs` (create)

**Interfaces:**
- Consumes: everything from Tasks 1–5.
- Produces: nothing.

- [ ] **Step 1: Write the equivalence sweep**

```rust
/// Across real cluster traffic, building `TransactionOutputs` from checkpoint
/// data must produce exactly what execution produced. Both paths write through
/// the same `write_transaction_outputs`, so this is what guarantees the stored
/// state matches.
#[sim_test]
async fn constructors_agree_across_cluster_traffic() {
    // 1. Run a test cluster and generate varied traffic: transfers, shared
    //    object writes, a shared object deletion, an object receive, and a
    //    publish. Cross at least one epoch boundary.
    // 2. For each checkpoint, capture its `CheckpointData` and, for each
    //    transaction, the `TransactionOutputs` execution produced.
    // 3. Assert equality per transaction.
    for (executed, checkpoint_tx) in captured {
        let applied = TransactionOutputs::build_from_checkpoint_transaction(&checkpoint_tx)
            .expect("cluster-produced checkpoint data is well formed");
        assert_outputs_eq(&applied, &executed, checkpoint_tx.effects.transaction_digest());
    }
    assert!(
        captured.len() > 50,
        "the sweep must cover real traffic, not a handful of transactions"
    );
}
```

`assert_outputs_eq` compares every field, sorting the vector fields first (iteration order differs between the constructors), and includes the transaction digest in each failure message.

**This task carries the verification for the whole change.** Five things could not be covered at unit level and are deferred here. If Task 6 slips, they ship untested:

1. **A shared-object deletion** — see below.
2. **The events branches** of `verify_payload_digests`. A transfer emits none, and building an `Event` by hand is not cheap. Cluster traffic with Move calls covers `(Some, None)` and `(None, Some)` naturally.
3. **The end-of-epoch skip.** Constructing a change-epoch transaction in a unit fixture is expensive; a cluster produces one per epoch.
4. **The epoch wait actually waiting.** `wait_for_epoch_returns_immediately_when_already_current` covers the fast path only; the parking path needs a real reconfiguration.
5. **Archive sync end to end with an applier.** `crates/iota-network/src/state_sync/tests.rs` drives state sync against a `SharedInMemoryStore` with no authority, so no applier can be handed to it there.

**A shared-object deletion must be in the traffic.** Task 2 covers the transfer path only: its unit fixture uses `prepare_transaction_for_benchmark`, which returns the `InnerTemporaryStore` needed to build the execution-side `TransactionOutputs`, whereas the shared-deletion fixture (`TestRunner` in `crates/iota-core/src/unit_tests/shared_object_deletion_tests.rs`) returns only effects. Rather than duplicate or refactor that fixture, the case is covered here. It matters because the owned-vs-shared marker decision is the one place the two constructors resolve the input owner from different sources — `input_objects` for execution, `effects.old_object_metadata()` for the applying path. Reasoning says they agree (a deleted shared object has `input_state: ObjectIn::Data { owner: Shared }`, so it appears in `old_object_metadata` with the shared owner), but that is reasoning, not a test.

Traffic variety is what this task buys over Task 2. A sweep of 500 plain transfers proves much less than 50 transactions covering shared deletions, receives and publishes — those are the paths where the input-owner resolution differs. Read `crates/iota-e2e-tests/tests/reconfiguration_tests.rs` for the cluster-with-epoch-change pattern.

- [ ] **Step 2: Add one end-to-end wiring check**

The sweep proves the constructors agree; this proves the archive path is actually wired to use one.

```rust
/// Archive sync with applying enabled must advance the executed watermark
/// without the execution scheduler running any transaction.
#[sim_test]
async fn archive_sync_advances_without_executing() {
    // Sync a fullnode from a fixture archive with applying enabled, then
    // assert the executed watermark reached the archive's last checkpoint and
    // `execution_driver_executed_transactions` counted only the
    // end-of-epoch transactions.
}
```

- [ ] **Step 3: Run**

Run: `cargo simtest -p iota-e2e-tests apply_checkpoint_results`
Expected: PASS. Allow 10+ minutes.

On a mismatch, fix the constructor, not the assertion.

- [ ] **Step 4: Commit**

```bash
git add crates/iota-e2e-tests/tests/apply_checkpoint_results_tests.rs
git commit -m "test(e2e): checkpoint-data and execution TransactionOutputs agree"
```

---

### Task 7: Make applying the default for archive sync

**Files:**
- Modify: `crates/iota-network/src/state_sync/mod.rs` (remove the temporary `false` from Task 5 Step 1, read the config)
- Modify: `crates/iota-config/src/node.rs` (doc comment only)

**Interfaces:**
- Consumes: Tasks 5 and 6.
- Produces: nothing.

- [ ] **Step 1: Read the flag at the call site**

Replace the hard-coded `false` with `checkpoint_archive_config.re_execute_archived_checkpoints`.

- [ ] **Step 2: Log the mode at startup**

Operators must be able to tell from the log which path a node took.

```rust
info!(
    re_execute = checkpoint_archive_config.re_execute_archived_checkpoints,
    "syncing from checkpoint archive"
);
```

- [ ] **Step 3: Run the acceptance test and commit**

```bash
cargo simtest -p iota-e2e-tests apply_checkpoint_results
git add crates/iota-network/src/state_sync/mod.rs crates/iota-config/src/node.rs
git commit -m "feat(state-sync): apply archived checkpoint results by default"
```

---

### Task 8: Skip rebuilding checkpoint data the applying path already holds

`process_checkpoint_data` calls `load_checkpoint_data`, which re-reads events from the transaction cache and input/output objects back out of the object store to rebuild a `CheckpointData` the archive path already downloaded. On the applying path that work is redundant. This is a performance-only change and is safe to defer.

**Files:**
- Modify: `crates/iota-core/src/checkpoints/checkpoint_executor/mod.rs:688-707`
- Modify: `crates/iota-core/src/checkpoints/checkpoint_executor/data_ingestion_handler.rs`

**Interfaces:**
- Consumes: Task 7.
- Produces: nothing.

- [ ] **Step 1: Confirm the epoch-boundary path is unaffected**

`process_checkpoint_data` also feeds `index_epoch_boundary` at epoch boundaries. Before changing anything, read `index_epoch_boundary` and confirm the streamed `CheckpointData` carries everything it reads. Write down what you checked. If it does not, stop and report rather than working around it.

- [ ] **Step 2: Add a cache for the already-downloaded data**

Give the executor a way to receive the streamed `CheckpointData` for a sequence number and use it in place of `load_checkpoint_data`. Keep the fallback: any checkpoint not present must still be rebuilt from the store.

- [ ] **Step 3: Measure**

Run a node against testnet from the archive and compare `checkpoint_executor_pipeline_stage_active_duration_ns{stage="ProcessCheckpointData"}` before and after. Expected: the stage drops toward zero.

Enable the metrics first — they are suppressed by default:

```yaml
metrics:
  groups:
    checkpoints: trace
    storage: trace
```

- [ ] **Step 4: Commit**

```bash
git commit -m "perf(checkpoint-executor): reuse downloaded checkpoint data instead of rebuilding it"
```

---

### Task 9: Compare the accumulated live-object-set hash at epoch boundaries

Not needed for correctness — the per-object digests already cover the applied data — but it is a cheap self-check on the apply and commit code, and it catches an object written to the wrong key or a gap in verification coverage. Currently nothing compares the two: `get_epoch_state_commitments` has only two callers, the snapshot uploader and one e2e test.

Useful on the executing path too, so it is not gated on the applying path.

**Files:**
- Modify: `crates/iota-core/src/checkpoints/checkpoint_executor/mod.rs` (epoch-boundary handling)
- Test: `crates/iota-e2e-tests/tests/apply_checkpoint_results_tests.rs`

**Interfaces:**
- Consumes: Task 7.
- Produces: nothing.

- [ ] **Step 1: Write the failing test**

```rust
/// At an epoch boundary the locally accumulated live-object-set hash must
/// match the certified `EcmhLiveObjectSet` commitment.
#[sim_test]
async fn epoch_boundary_state_hash_matches_commitment() {
    // Run a cluster across an epoch boundary; assert the node logs a
    // successful comparison and does not halt.
}
```

- [ ] **Step 2: Implement the comparison**

At the epoch boundary, after `digest_epoch`, fetch the certified commitment via `checkpoint_store.get_epoch_state_commitments(epoch)` and compare. On mismatch use `fatal!`, matching how `assert_checkpoint_not_forked` treats a fork — a divergent live object set is not recoverable by continuing.

- [ ] **Step 3: Measure the cost**

`digest_epoch` walks accumulated state. Time the boundary before and after on a catching-up node. If it adds more than a few seconds per epoch, put it behind a config flag defaulting to on and report the number.

- [ ] **Step 4: Run and commit**

```bash
cargo simtest -p iota-e2e-tests epoch_boundary_state_hash
git commit -m "feat(checkpoint-executor): verify the epoch live-object-set hash against its commitment"
```

---

## Self-Review Notes

**Spec coverage.** Proposal steps 1–5 map to Tasks 1–2 (constructor), 3 (verification), 4 (apply), 5+7 (archive default), 8 (bypass `load_checkpoint_data`). The "worth adding while we are here" section maps to Task 9. The acceptance test named in the spec is Task 6.

**One thing the spec got wrong, corrected here.** The spec implies the checkpoint executor needs changes. It does not: `schedule_transaction_execution` (`checkpoint_executor/mod.rs:892-925`) already reads `multi_get_executed_effects_digests`, runs `assert_not_forked` on transactions it finds already executed, and enqueues only the rest. Writing the results before the executor arrives makes it short-circuit, and the whole pipeline — state hash accumulation from effects, indexes, watermarks — runs unchanged. This also makes the change fail-safe: if results are not yet written when the executor arrives, it just executes.

**One thing the spec understated.** Because the executor only assigns shared object versions for transactions it schedules, an applying path that writes everything leaves `next_shared_object_versions` un-advanced. Task 4 Step 5 does the assignment explicitly. This is the subtlest part of the change and deserves close review.

**Resolved.** The apply logic lives in a dedicated `CheckpointResultsApplier` in `iota-core`, not on `RocksDbStore`. The store stays a thin adapter over caches and stores; the applier owns the epoch-store handle it needs for the shared-version assignment, and reaches state sync as a trait object so the reducer's `S: WriteStore` bound is untouched.
