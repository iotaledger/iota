// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end comparison: run a staking transaction through the local
//! [`LocalVm`] (objects pre-fetched over gRPC into a [`GrpcStore`]) and against
//! a live [`test_cluster::TestCluster`]'s own dry-run, then assert both agree.

use std::collections::BTreeSet;

use iota_json_rpc_types::{IotaExecutionStatus, IotaTransactionBlockEffectsAPI};
use iota_sdk_types::Owner;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::ObjectRef, effects::TransactionEffectsAPI, transaction::TransactionData,
};
use iota_vm_sdk::{ExecuteOptions, LocalVm, ObjectId, grpc::GrpcStore};
use test_cluster::TestClusterBuilder;

/// Build a staking transaction, simulate it with both the node's dry-run and
/// the local VM, and assert the two produce the same object changes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compare_local_vm_staking_against_test_cluster() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;

    test_cluster.wait_for_checkpoint(1, None).await;

    let validator = test_cluster
        .iota_client()
        .governance_api()
        .get_latest_iota_system_state()
        .await
        .expect("should get system state")
        .active_validators()
        .first()
        .expect("should have at least one validator")
        .iota_address;

    // One coin pays for gas, a second is staked.
    let sender = test_cluster.wallet.get_addresses()[0];
    let rgp = test_cluster.get_reference_gas_price().await;
    let all_coins = test_cluster
        .wallet
        .get_gas_objects_owned_by_address(sender, 5)
        .await
        .unwrap();
    assert!(
        all_coins.len() >= 2,
        "need at least 2 coins, got {}",
        all_coins.len()
    );
    let gas = all_coins[0];
    let stake_coin = all_coins[1];

    let tx_data = TestTransactionBuilder::new(sender, gas, rgp)
        .call_staking(stake_coin, validator)
        .build();

    // Reference result: the node's own dry-run.
    let dry_run = test_cluster
        .iota_client()
        .read_api()
        .dry_run_transaction_block(tx_data.clone())
        .await
        .expect("node dry-run should succeed");
    assert!(
        matches!(dry_run.effects.status(), IotaExecutionStatus::Success),
        "node dry-run staking should succeed"
    );

    // Local VM: pre-fetch every referenced object over gRPC, then dev-inspect
    // offline against the same Move engine the node uses.
    let mut store = GrpcStore::connect(&test_cluster.grpc_url()).expect("connect gRPC store");
    let ctx = store
        .fetch_chain_context()
        .await
        .expect("fetch chain context");
    store.prefetch(&tx_data).await.expect("prefetch objects");
    // Staking reads the validator set stored as dynamic fields inside the
    // system state, so the offline store also needs those children.
    store
        .prefetch_dynamic_fields()
        .await
        .expect("prefetch dynamic fields");

    // Snapshot the chain parameters and non-framework objects before the store
    // is moved into the VM, so an optional fixture dump (for the wasm example)
    // can capture exactly the replay set: everything fetched minus the
    // framework packages the wasm side re-seeds itself.
    let chain = (
        ctx.protocol_version.as_u64(),
        ctx.reference_gas_price,
        ctx.epoch_id,
        ctx.epoch_timestamp_ms,
    );
    let framework_ids: std::collections::HashSet<ObjectId> =
        iota_framework::BuiltInFramework::genesis_objects()
            .map(|o| o.id())
            .collect();
    let fixture_objects: Vec<(ObjectId, Vec<u8>)> = store
        .store()
        .iter()
        .filter(|(id, _)| !framework_ids.contains(*id))
        .map(|(id, obj)| (*id, bcs::to_bytes(obj).expect("encode object")))
        .collect();

    let mut vm = LocalVm::new(ctx, store).expect("build LocalVm");
    let result = vm
        .execute(tx_data.clone(), ExecuteOptions::dev_inspect())
        .expect("local dev-inspect should succeed");
    assert!(
        result.effects.status().is_success(),
        "local dev-inspect staking should succeed: {:?}",
        result.effects.status()
    );

    // Compare the full object references: the `ObjectRef` carries each object's
    // version and content digest, so both backends must agree on the resulting
    // contents, not merely on which objects were touched.
    //
    // The gas object is the one principled exception: local dev-inspect and the
    // node's dry-run meter gas differently, so the gas coin's post-execution
    // balance — and therefore its digest — can legitimately differ. We compare
    // it by id and owner only, and exclude it from the full-ref mutated set.
    let node_gas = (
        dry_run.effects.gas_object().object_id(),
        dry_run.effects.gas_object().owner,
    );
    let (local_gas_ref, local_gas_owner) = result.effects.gas_object();
    let local_gas = (local_gas_ref.object_id, local_gas_owner);
    assert_eq!(
        node_gas, local_gas,
        "node dry-run and local VM should agree on the gas object id and owner"
    );

    let node_created: BTreeSet<(ObjectRef, Owner)> = dry_run
        .effects
        .created()
        .iter()
        .map(|o| (o.reference, o.owner))
        .collect();
    let node_mutated: BTreeSet<(ObjectRef, Owner)> = dry_run
        .effects
        .mutated()
        .iter()
        .filter(|o| o.object_id() != node_gas.0)
        .map(|o| (o.reference, o.owner))
        .collect();
    let node_deleted: BTreeSet<ObjectRef> = dry_run.effects.deleted().iter().copied().collect();

    let local_created: BTreeSet<(ObjectRef, Owner)> =
        result.effects.created().into_iter().collect();
    let local_mutated: BTreeSet<(ObjectRef, Owner)> = result
        .effects
        .mutated()
        .into_iter()
        .filter(|(r, _)| r.object_id != local_gas.0)
        .collect();
    let local_deleted: BTreeSet<ObjectRef> = result.effects.deleted().into_iter().collect();

    assert_eq!(
        node_created, local_created,
        "node dry-run and local VM should create the same objects"
    );
    assert_eq!(
        node_mutated, local_mutated,
        "node dry-run and local VM should mutate the same objects"
    );
    assert_eq!(
        node_deleted, local_deleted,
        "node dry-run and local VM should delete the same objects"
    );

    // Optionally capture this run as a wasm-example fixture. Off by default;
    // set IOTA_VM_SDK_DUMP_FIXTURE=<path> to (re)generate it.
    if let Ok(path) = std::env::var("IOTA_VM_SDK_DUMP_FIXTURE") {
        dump_fixture(&path, &tx_data, chain, &fixture_objects);
    }
}

/// Serialize a run into the JSON fixture format the wasm example consumes
/// (see `examples/wasm/` and `tests/fixtures/`). Staking is captured unsigned:
/// the example runs it in dev-inspect, so no signature is needed.
fn dump_fixture(
    path: &str,
    tx: &TransactionData,
    chain: (u64, u64, u64, u64),
    objects: &[(ObjectId, Vec<u8>)],
) {
    use base64::Engine;
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let (protocol_version, reference_gas_price, epoch_id, epoch_timestamp_ms) = chain;
    let objects: Vec<_> = objects
        .iter()
        .map(|(id, bytes)| serde_json::json!({ "id_hex": id.to_string(), "bcs_b64": b64(bytes) }))
        .collect();
    let fixture = serde_json::json!({
        "name": "stake",
        "description": "Stake a coin with a validator via 0x3::iota_system::request_add_stake; \
            succeeds and emits staking events.",
        "protocol_version": protocol_version,
        "reference_gas_price": reference_gas_price,
        "epoch_id": epoch_id,
        "epoch_timestamp_ms": epoch_timestamp_ms,
        "tx_b64": b64(&bcs::to_bytes(tx).expect("encode tx")),
        "signatures": [],
        "objects": objects,
    });
    std::fs::write(
        path,
        serde_json::to_string_pretty(&fixture).expect("serialize fixture"),
    )
    .expect("write fixture");
    eprintln!(
        "wrote fixture to {path} ({} objects)",
        fixture["objects"].as_array().unwrap().len()
    );
}
