# gRPC genesis/migration object index fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the node's gRPC owned-object index (`GrpcIndexesStore`) contain genesis and migration-loaded objects, so `ListOwnedObjects` / `ListDynamicFields` return them on a from-genesis fullnode.

**Architecture:** The gRPC index initializes at node startup via a live-object-set scan (`GrpcIndexesStore::new` → `init`, `iota-node/src/lib.rs:589`) that runs _before_ migration transactions execute (`lib.rs:725`), so migration objects — and any genesis object not in the store at scan time — are never indexed. The JSON-RPC index avoids this by explicitly indexing the full genesis+migration object list in `AuthorityState::create_owner_index_if_empty` (`authority.rs:3472`, called at `lib.rs`/`authority.rs:3357`). This plan gives the gRPC index the same explicit, idempotent, once-only genesis-indexing step, reusing the existing `GrpcLiveObjectRestorer` machinery ("the same way we load a formal snapshot").

**Tech Stack:** Rust, RocksDB via `typed-store`, `iota-core`, `iota-e2e-tests`, `cargo nextest` / `cargo simtest`.

## Global Constraints

- No `#[allow(dead_code)]` / `#[allow(unused)]` or other lint suppressions — fix the underlying issue.
- Never disable or skip tests.
- `cargo ci-clippy` must be clean; `cargo +nightly fmt` applied.
- Comments: explain non-obvious _why_, no change-history or conversational notes.
- The fix must be **idempotent** (safe to run every startup) and **once-only** in effect (no repeated work after the first genesis indexing), because base genesis objects may already be present from the `init` scan.

---

## Root cause (confirmed)

- `genesis.objects()` are bulk-inserted at store open (`authority_store.rs:262`), i.e. before the gRPC `init` scan → base genesis objects are usually indexed.
- **Migration objects** are added by executing migration txs at `iota-node/src/lib.rs:725`, _after_ the gRPC index already initialized and finalized → they are missing.
- `AuthorityState::new` receives the full list `genesis_objects = genesis.objects() + migration_tx_data.get_objects()` (`lib.rs:670-673`) and hands it to `create_owner_index_if_empty` (`authority.rs:3357`), which today indexes it into the **JSON-RPC** index only.

The fix indexes that same list into the gRPC index too.

---

## Task 1: `index_genesis_objects` on `GrpcIndexesStore` + once-only marker

**Files:**

- Modify: `crates/iota-core/src/grpc_indexes.rs` (add `Watermark::GenesisIndexed` at the `enum Watermark` ~line 66; store `batch_size_limit` on `GrpcIndexesStore`; add `index_genesis_objects`; add unit test in the existing `#[cfg(test)] mod` ~line 1410)

**Interfaces:**

- Consumes: existing `GrpcIndexesStore::new_without_init`, `tables.live_object_restorer(batch_size_limit)`, `GrpcLiveObjectRestorer::begin_partition` / `GrpcPartitionIndexer::{index_object, finish}` / `GrpcLiveObjectRestorer::finish`, `account_owned_objects_info_iter`, `self.tables.watermark`.
- Produces: `pub fn GrpcIndexesStore::index_genesis_objects(&self, objects: &[Object]) -> Result<(), StorageError>` — idempotent, once-only (guarded by `Watermark::GenesisIndexed`).

- [ ] **Step 1: Add the `GenesisIndexed` watermark variant**

At `enum Watermark` (~line 66), add a variant alongside `Indexed` / `Pruned`:

```rust
pub enum Watermark {
    Indexed,
    Pruned,
    /// Set once genesis + migration objects have been indexed into the live-state
    /// tables, so the one-shot `index_genesis_objects` step is skipped on restart.
    GenesisIndexed,
}
```

- [ ] **Step 2: Store `batch_size_limit` on `GrpcIndexesStore`**

In `GrpcIndexesStore::new` (~line 862-889) the value is already computed as `batch_size_limit` from `bulk_options.batch_size_limit`. Add a field `batch_size_limit: usize` to the `GrpcIndexesStore` struct (~line 845) and set it in `new` (both the init and already-initialized branches) so it is available later.

- [ ] **Step 3: Write the failing unit test**

Add to the test module (model on `live_object_restorer_builds_live_state_indexes`, ~line 1417). Use a couple of address-owned coin objects (helpers already used by nearby tests for building `Object`s):

```rust
#[tokio::test]
async fn index_genesis_objects_adds_owned_objects_and_is_once_only() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let store = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());

    let owner = Address::random_for_testing_only();
    let obj = new_gas_coin_for_owner(owner); // existing test helper pattern for an address-owned Object
    let obj_id = obj.id();

    // Not present before indexing.
    assert!(owned_ids(&store, owner).is_empty());

    store.index_genesis_objects(&[obj.clone()]).unwrap();
    assert_eq!(owned_ids(&store, owner), vec![obj_id]);

    // Second call is a no-op (marker set) and does not error or duplicate.
    store.index_genesis_objects(&[obj]).unwrap();
    assert_eq!(owned_ids(&store, owner), vec![obj_id]);
}

fn owned_ids(store: &GrpcIndexesStore, owner: Address) -> Vec<ObjectId> {
    use iota_node_storage::GrpcIndexes;
    store
        .account_owned_objects_info_iter(owner, None, None)
        .unwrap()
        .map(|item| item.unwrap().0.object_id)
        .collect()
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core index_genesis_objects_adds_owned_objects_and_is_once_only`
Expected: FAIL — `index_genesis_objects` / `new_gas_coin_for_owner` not found (build error) then, once helper is stubbed, assertion or missing-method failure.

- [ ] **Step 5: Implement `index_genesis_objects`**

Add to `impl GrpcIndexesStore` (near `finalize_restore`, ~line 1068):

```rust
/// Index the given genesis + migration objects into the live-state tables.
///
/// Runs once (guarded by `Watermark::GenesisIndexed`): the node calls it on
/// every startup, but only the first call after a fresh index does work.
/// Idempotent per object — re-indexing an object already present from the
/// `init` live-object scan is a keyed upsert, so overlap is harmless.
pub fn index_genesis_objects(&self, objects: &[Object]) -> Result<(), StorageError> {
    if self
        .tables
        .watermark
        .get(&Watermark::GenesisIndexed)
        .map_err(|e| StorageError::custom(e.to_string()))?
        .is_some()
    {
        return Ok(());
    }

    let restorer = self.tables.live_object_restorer(self.batch_size_limit);
    let mut partition = restorer.begin_partition();
    for object in objects {
        partition.index_object(object.clone())?;
    }
    partition.finish()?;
    restorer.finish()?; // flush the aggregated coin index

    self.tables
        .watermark
        .insert(&Watermark::GenesisIndexed, &0)
        .map_err(|e| StorageError::custom(e.to_string()))?;
    Ok(())
}
```

(If `new_gas_coin_for_owner` has no existing analogue in the test module, build the `Object` inline with the same constructor the neighbouring tests use for an address-owned coin.)

- [ ] **Step 6: Run the test to verify it passes**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core index_genesis_objects_adds_owned_objects_and_is_once_only`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/iota-core/src/grpc_indexes.rs
git commit -m "feat(iota-core): index genesis/migration objects into the gRPC live-state index

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Wire genesis indexing into `AuthorityState::create_owner_index_if_empty`

**Files:**

- Modify: `crates/iota-core/src/authority.rs:3472-3533` (`create_owner_index_if_empty`)

**Interfaces:**

- Consumes: `GrpcIndexesStore::index_genesis_objects` (Task 1); `self.grpc_indexes_store: Option<Arc<GrpcIndexesStore>>` (`authority.rs:854`).
- Produces: no new public API; genesis objects now flow into the gRPC index at startup independently of whether the JSON-RPC index is enabled.

- [ ] **Step 1: Split the JSON-RPC body out so the early-return can't skip gRPC**

The current method early-returns when `self.indexes` (JSON-RPC index) is `None`, which would also skip gRPC on a gRPC-only node. Extract the existing JSON-RPC body into a private helper and make the entry point call both indexes:

```rust
fn create_owner_index_if_empty(
    &self,
    genesis_objects: &[Object],
    epoch_store: &Arc<AuthorityPerEpochStore>,
) -> IotaResult {
    self.create_jsonrpc_owner_index_if_empty(genesis_objects, epoch_store)?;

    if let Some(grpc_indexes_store) = &self.grpc_indexes_store {
        grpc_indexes_store
            .index_genesis_objects(genesis_objects)
            .map_err(|e| IotaError::Storage(e.to_string()))?;
    }
    Ok(())
}

fn create_jsonrpc_owner_index_if_empty(
    &self,
    genesis_objects: &[Object],
    epoch_store: &Arc<AuthorityPerEpochStore>,
) -> IotaResult {
    // ... existing body verbatim (the `let Some(index_store) = &self.indexes` guard,
    //     the loop building new_owners / new_dynamic_fields, and
    //     index_store.insert_genesis_objects(...)) ...
}
```

(Use the exact `IotaError` storage-variant constructor already used elsewhere in this file for string-backed storage errors; match the surrounding style.)

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p iota-core`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/iota-core/src/authority.rs
git commit -m "feat(iota-core): feed genesis/migration objects into the gRPC index at startup

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: End-to-end proof — migration objects via gRPC `ListOwnedObjects`

> **Branch note:** this branch is off `develop`; PR #12130 is NOT present, so
> `test-cluster` has no `grpc_client()` helper and the migration test uses
> JSON-RPC (`test_cluster.wallet.get_client()`), with the fullnode gRPC API
> disabled by default (`fullnode_enable_grpc_api: false`). This task therefore
> ADDS a gRPC helper + enables gRPC on the migration cluster + adds a gRPC
> owned-objects assertion — it does not "replace direct-state reads".

**Files:**

- Modify: `crates/test-cluster/src/lib.rs` (add a `grpc_client()` helper next to the existing `grpc_url()` at ~line 135)
- Modify: `crates/iota-e2e-tests/tests/full_node_migration_tests.rs` (enable the fullnode gRPC API on the cluster; add a gRPC `list_owned_objects` assertion for a migration-loaded `NftOutput` object in `test_full_node_load_migration_data_with_address_swap`, ~line 78-90)

**Interfaces:**

- Consumes: `TestCluster::grpc_url()` (exists); `iota_grpc_client::Client::new` / `list_owned_objects`; `TestClusterBuilder::with_fullnode_enable_grpc_api` (`test-cluster/src/lib.rs:1068`).
- Produces: `TestCluster::grpc_client(&self) -> iota_grpc_client::Client`; a migration test that asserts migration objects are returned over gRPC.

- [ ] **Step 1: Add the `grpc_client()` helper to test-cluster**

Next to `grpc_url()` (`crates/test-cluster/src/lib.rs:135`), add:

```rust
/// A gRPC client pointed at the fullnode's gRPC API.
/// Requires the cluster to be built with `with_fullnode_enable_grpc_api(true)`.
pub fn grpc_client(&self) -> iota_grpc_client::Client {
    iota_grpc_client::Client::new(self.grpc_url())
        .expect("failed to build gRPC client for the fullnode")
}
```

Add `iota-grpc-client` to `crates/test-cluster/Cargo.toml` `[dependencies]` if not already present (workspace dep name `iota-grpc-client`).

- [ ] **Step 2: Enable the fullnode gRPC API on the migration cluster**

In `test_full_node_load_migration_data_with_address_swap` (~line 78-90), add `.with_fullnode_enable_grpc_api(true)` to the `TestClusterBuilder` chain (alongside `.disable_fullnode_pruning()` / `.with_migration_data(...)`). Confirm the fullnode gets a gRPC address — if the builder requires an explicit address, also call `.with_fullnode_grpc_api_address(<addr>)` using the same local-address helper other tests use (grep `with_fullnode_grpc_api_address` / `get_available_port` in the repo for the pattern).

- [ ] **Step 3: Add the failing gRPC owned-objects assertion**

The test already verifies migration objects via JSON-RPC (`get_owned_objects` with `IotaObjectDataFilter::StructType(NftOutput::tag(GAS::type_tag()))`, ~line 244-255) for a known migrated owner address. After that JSON-RPC check, add a gRPC parity assertion for the SAME owner and object:

```rust
// The gRPC owned-object index must also contain the migration-loaded object.
let grpc = test_cluster.grpc_client();
let grpc_ids = grpc
    .list_owned_objects(owner, None, None, None) // (owner, type filter, page size, cursor) — match the real signature
    .await
    .unwrap();
assert!(
    grpc_ids.iter().any(|o| o.object_id() == expected_object_id),
    "gRPC ListOwnedObjects must return the migration-loaded NftOutput object"
);
```

Bind `owner` / `expected_object_id` from the object the JSON-RPC assertion already found (reuse the same address and one returned object id). Match the exact `list_owned_objects` argument order and return type from `iota_grpc_client` (read the crate's method signature; the earlier `WalletContext`-analysis notes it takes an owner, optional type filter, optional page size, optional cursor).

- [ ] **Step 4: Confirm it fails without the fix, passes with it**

Run: `MSIM_WATCHDOG_TIMEOUT_MS=180000 cargo simtest -p iota-e2e-tests test_full_node_load_migration_data_with_address_swap`
Expected: PASS with the Task 1–2 fix present. To prove the assertion is non-trivial, temporarily comment out the `index_genesis_objects` call in `create_owner_index_if_empty` and confirm this assertion FAILS, then restore it.

- [ ] **Step 5: Commit**

```bash
git add crates/test-cluster/src/lib.rs crates/test-cluster/Cargo.toml crates/iota-e2e-tests/tests/full_node_migration_tests.rs
git commit -m "test(iota-e2e-tests): assert migration-loaded owned objects are served over gRPC

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Verification sweep

**Files:** none (verification only)

- [ ] **Step 1: Unit + core tests**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core`
Expected: PASS (incl. the new `index_genesis_objects` test and existing `grpc_indexes` tests).

- [ ] **Step 2: Migration + owned-object e2e/sim tests**

Run: `MSIM_WATCHDOG_TIMEOUT_MS=180000 cargo simtest -p iota-e2e-tests full_node_migration`
Expected: PASS.

- [ ] **Step 3: Lint + format**

Run: `cargo ci-clippy` and `cargo +nightly fmt --check`
Expected: clean.

- [ ] **Step 4: Sanity — a plain (non-migration) genesis node still returns genesis-owned objects over gRPC**

Add or run an existing test that queries a genesis-funded address's gas coins via `grpc_client().list_owned_objects`. Expected: PASS both before and after (regression guard confirming the fix didn't disturb the base-genesis path).

---

## Self-Review

- **Spec coverage:** Root cause (init runs before migration txs) → Tasks 1–2 add explicit genesis+migration indexing at the correct point. Idempotency/once-only → `Watermark::GenesisIndexed` guard + keyed-upsert reasoning. gRPC-only-node correctness → Task 2 restructure removes the JSON-RPC early-return. Empirical proof → Task 3 (migration) + Task 4 Step 4 (base genesis regression guard).
- **Placeholder scan:** The two "match the exact signature used elsewhere" notes point at concrete existing call sites (the migrated `tests/grpc/...` and this file's `IotaError` storage constructor), not unspecified behavior; the test helper for building an address-owned `Object` follows the adjacent test module's existing pattern.
- **Type consistency:** `index_genesis_objects(&self, objects: &[Object]) -> Result<(), StorageError>` is defined in Task 1 and consumed with that exact signature in Task 2; `Watermark::GenesisIndexed` and the stored `batch_size_limit` field are introduced in Task 1 and only referenced afterward.

## Open confirmation during Task 1 Step 4

If the Task 1 unit test shows base genesis objects were _not_ already indexed by `init` in some path, nothing changes in this plan — `index_genesis_objects` feeds the full list regardless, and the once-only guard keeps it cheap. Note the observed behaviour in the commit message.
