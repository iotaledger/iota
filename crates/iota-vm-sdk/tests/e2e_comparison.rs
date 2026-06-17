// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end comparison: run a staking transaction through the local
//! [`LocalVm`] (objects pre-fetched over gRPC into a [`GrpcStore`]) and against
//! a live [`test_cluster::TestCluster`]'s own dry-run, then assert both agree.

use std::collections::BTreeSet;

use iota_json_rpc_types::{IotaExecutionStatus, IotaTransactionBlockEffectsAPI};
use iota_sdk_types::Owner;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{base_types::ObjectRef, effects::TransactionEffectsAPI};
use iota_vm_sdk::{ExecuteOptions, LocalVm, TypeTag, grpc::GrpcStore};
use move_core_types::annotated_value::MoveValue;
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
    let mut store = GrpcStore::connect(test_cluster.grpc_url()).expect("connect gRPC store");
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

    // The staking call emits a `StakingRequestEvent`; use it to exercise the
    // SDK's introspection surface (`decode_events` / `decode_value`).
    let events = result
        .events
        .as_ref()
        .expect("staking run must emit events");
    let decoded = vm
        .decode_events(events)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("every emitted event must decode");
    let staking_event = decoded
        .iter()
        .find(|d| d.event.type_.name().as_str() == "StakingRequestEvent")
        .expect("a StakingRequestEvent must be present");

    // The raw BCS contents decode into a struct with the event's named fields.
    let MoveValue::Struct(decoded_struct) = &staking_event.value else {
        panic!(
            "event must decode to a struct, got {:?}",
            staking_event.value
        );
    };
    let amount = decoded_struct
        .fields
        .iter()
        .find(|(name, _)| name.as_str() == "amount")
        .map(|(_, value)| value)
        .expect("StakingRequestEvent has an `amount` field");
    assert!(
        matches!(amount, MoveValue::U64(v) if *v > 0),
        "staked amount must decode to a positive u64, got {amount:?}"
    );

    // `decode_value` on the same bytes + type reproduces the same value.
    let event = &staking_event.event;
    let via_value = vm
        .decode_value(
            &event.contents,
            &TypeTag::Struct(Box::new(event.type_.clone())),
        )
        .expect("decode_value must succeed on the event contents");
    assert_eq!(
        via_value, staking_event.value,
        "decode_value and decode_events must agree on the decoded payload"
    );
}
