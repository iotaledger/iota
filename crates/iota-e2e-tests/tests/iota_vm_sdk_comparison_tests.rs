// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end comparison between the `iota-vm-sdk` local VM and a live node.
//!
//! Runs transactions through the local [`LocalVm`] (objects resolved on
//! demand over gRPC from a [`GrpcStore`]) and against a live
//! [`test_cluster::TestCluster`]'s own dry-run, then asserts both agree — on
//! a staking transaction's full change set, and on receiving-object
//! semantics. This lives in `iota-e2e-tests` rather than alongside the SDK
//! because it needs a full cluster; the SDK's own suite stays offline and
//! cluster-free.

// The SDK's `GrpcStore` needs a multi-threaded Tokio runtime, which the `msim`
// simulator does not provide, so this test only runs under a real runtime.
#![cfg(not(msim))]

use std::{
    collections::{BTreeSet, HashSet},
    path::PathBuf,
};

use iota_json_rpc_types::{IotaExecutionStatus, IotaTransactionBlockEffectsAPI};
use iota_sdk_types::{
    Address, ExecutionError, ExecutionStatus, ObjectId, ObjectReference, Owner, StructTag,
};
use iota_test_transaction_builder::{TestTransactionBuilder, publish_package};
use iota_types::{
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, UserInputError},
    transaction::CallArg,
};
use iota_vm_sdk::{ExecuteOptions, ExecutionResult, LocalVm, TypeTag, VmSdkError, grpc::GrpcStore};
use move_core_types::annotated_value::MoveValue;
use test_cluster::{TestCluster, TestClusterBuilder};

/// Build a staking transaction, simulate it with both the node's dry-run and
/// the local VM, and assert the two produce the same object changes and events.
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

    // Reference object-change sets from the node's dry-run. The `ObjectRef`
    // carries each object's version and content digest, so the backends must
    // agree on resulting contents, not merely on which objects were touched.
    //
    // The transaction carries a real gas coin, so both backends meter gas the
    // same way and the gas object must match in full — id, owner, version, and
    // content digest (its post-execution balance). It is compared by its own
    // assertion below and kept out of the mutated set so it is not checked
    // twice.
    let node_gas: (ObjectReference, Owner) = {
        let gas = dry_run.effects.gas_object();
        (gas.reference, gas.owner)
    };
    let node_created: BTreeSet<(ObjectReference, Owner)> = dry_run
        .effects
        .created()
        .iter()
        .map(|o| (o.reference, o.owner))
        .collect();
    let node_mutated: BTreeSet<(ObjectReference, Owner)> = dry_run
        .effects
        .mutated()
        .iter()
        .filter(|o| o.object_id() != node_gas.0.object_id)
        .map(|o| (o.reference, o.owner))
        .collect();
    let node_deleted: BTreeSet<ObjectReference> =
        dry_run.effects.deleted().iter().copied().collect();

    // Reference events from the node's dry-run, compared by type, emitter, and
    // full BCS payload in emission order.
    let node_events: Vec<(StructTag, ObjectId, Address, Vec<u8>)> = dry_run
        .events
        .data
        .iter()
        .map(|e| {
            (
                e.type_.clone(),
                e.package_id,
                e.sender,
                e.bcs.bytes().to_vec(),
            )
        })
        .collect();

    // Local VM: every object the run reads — the transaction inputs and the
    // system-state dynamic fields staking walks — is resolved on demand over
    // gRPC during execution, against the same Move engine the node uses. Only
    // the chain context is fetched up front.
    let store = GrpcStore::connect(test_cluster.grpc_url()).expect("connect gRPC store");
    let ctx = store
        .fetch_chain_context()
        .await
        .expect("fetch chain context");

    let mut vm = LocalVm::new(ctx, store).expect("build LocalVm");

    // The node's object changes must match the local VM in both DevInspect
    // (relaxed checks, mock gas) and DryRun (full sign-time checks, real gas).
    let assert_changes_match = |result: &ExecutionResult, mode: &str| {
        let local_gas = result.effects.gas_object();
        assert_eq!(
            local_gas, node_gas,
            "{mode}: gas object must match the node in full (ref and owner)"
        );
        let local_created: BTreeSet<(ObjectReference, Owner)> =
            result.effects.created().into_iter().collect();
        let local_mutated: BTreeSet<(ObjectReference, Owner)> = result
            .effects
            .mutated()
            .into_iter()
            .filter(|(r, _)| r.object_id != node_gas.0.object_id)
            .collect();
        let local_deleted: BTreeSet<ObjectReference> =
            result.effects.deleted().into_iter().collect();
        assert_eq!(
            node_created, local_created,
            "{mode}: created objects must match"
        );
        assert_eq!(
            node_mutated, local_mutated,
            "{mode}: mutated objects must match"
        );
        assert_eq!(
            node_deleted, local_deleted,
            "{mode}: deleted objects must match"
        );
        let local_events: Vec<(StructTag, ObjectId, Address, Vec<u8>)> = result
            .events
            .as_ref()
            .map(|events| {
                events
                    .0
                    .iter()
                    .map(|e| (e.type_.clone(), e.package_id, e.sender, e.contents.clone()))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(
            node_events, local_events,
            "{mode}: emitted events must match"
        );
    };

    let dev_inspect = vm
        .execute(tx_data.clone(), ExecuteOptions::dev_inspect())
        .expect("local dev-inspect should succeed");
    assert!(
        dev_inspect.effects.status().is_success(),
        "local dev-inspect staking should succeed: {:?}",
        dev_inspect.effects.status()
    );
    assert_changes_match(&dev_inspect, "dev-inspect");

    let dry_run_local = vm
        .execute(tx_data.clone(), ExecuteOptions::dry_run())
        .expect("local dry-run should succeed");
    assert!(
        dry_run_local.effects.status().is_success(),
        "local dry-run staking should succeed: {:?}",
        dry_run_local.effects.status()
    );
    assert_changes_match(&dry_run_local, "dry-run");

    // The staking call emits a `StakingRequestEvent`; use it to exercise the
    // SDK's inspection surface (`decode_events` / `decode_value`).
    let events = dev_inspect
        .events
        .as_ref()
        .expect("staking run must emit events");
    let decoded = vm
        .decode_events(events)
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

/// Receiving-object semantics against a live node: a receiving reference whose
/// object was already received (and sent back to the same parent, so it is
/// receivable again at a newer version) must fail in the local VM exactly as
/// it fails on the node — not be silently resolved at the store's newer
/// version, which would flip the outcome to success.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compare_local_vm_receiving_against_test_cluster() {
    // `E_UNABLE_TO_RECEIVE_OBJECT` in the `iota::transfer` natives: the abort
    // the engine raises when the object cannot be received at the declared
    // version.
    const E_UNABLE_TO_RECEIVE_OBJECT: u64 = 3;

    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    // Publish the `tto` test package; `start` creates a parent object and a
    // child sent to the parent's object address, i.e. a receivable child.
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/move_test_code");
    let package_id = publish_package(&test_cluster.wallet, path).await.object_id;

    let start_fx = execute_tto_call(&test_cluster, package_id, "start", vec![]).await;
    let (parent, child) = parent_and_child(start_fx.created());

    // Receive the child once for real. `tto::receiver` sends it back to the
    // parent, so it is receivable again — but only at its new version; the
    // node's received-object marker blocks the old one.
    let receive_fx = execute_tto_call(
        &test_cluster,
        package_id,
        "receiver",
        vec![CallArg::ImmutableOrOwned(parent), CallArg::Receiving(child)],
    )
    .await;
    let current_parent = mutated_ref(&receive_fx, parent.object_id);
    let current_child = mutated_ref(&receive_fx, child.object_id);
    assert!(current_child.version > child.version);

    let outdated_tx = test_cluster
        .test_transaction_builder()
        .await
        .move_call(
            package_id,
            "tto",
            "receiver",
            vec![
                CallArg::ImmutableOrOwned(current_parent),
                CallArg::Receiving(child),
            ],
        )
        .build();
    let current_tx = test_cluster
        .test_transaction_builder()
        .await
        .move_call(
            package_id,
            "tto",
            "receiver",
            vec![
                CallArg::ImmutableOrOwned(current_parent),
                CallArg::Receiving(current_child),
            ],
        )
        .build();

    // Reference behavior: the node's dry-run lets the already-received
    // reference through signing (the marker makes it a previously-received
    // object) and the receive then aborts during execution.
    let node_outdated = test_cluster
        .iota_client()
        .read_api()
        .dry_run_transaction_block(outdated_tx.clone())
        .await
        .expect("node dry-run must produce effects");
    assert!(
        matches!(
            node_outdated.effects.status(),
            IotaExecutionStatus::Failure { .. }
        ),
        "node dry-run must fail the receive of an already-received object"
    );

    let store = GrpcStore::connect(test_cluster.grpc_url()).expect("connect gRPC store");
    let ctx = store
        .fetch_chain_context()
        .await
        .expect("fetch chain context");
    let mut vm = LocalVm::new(ctx, store).expect("build LocalVm");

    // Local dev-inspect matches the node: the receive resolves at the declared
    // (old) version, which the store no longer holds, and aborts.
    let local_outdated = vm
        .execute(outdated_tx.clone(), ExecuteOptions::dev_inspect())
        .expect("local dev-inspect must run to effects");
    assert!(
        matches!(
            local_outdated.effects.status(),
            ExecutionStatus::Failure {
                error: ExecutionError::MoveAbort {
                    code: E_UNABLE_TO_RECEIVE_OBJECT,
                    ..
                },
                ..
            }
        ),
        "local dev-inspect must abort the receive like the node, got {:?}",
        local_outdated.effects.status()
    );

    // Local dry-run applies the sign-time checks and rejects the outdated
    // reference up front. (The node's same-epoch dry-run instead defers the
    // failure to execution — its marker-based leniency exists only to consume
    // the owned-object locks, which the local VM does not model; the node
    // rejects with this same error once the marker's epoch is over.)
    let err = vm
        .execute(outdated_tx, ExecuteOptions::dry_run())
        .expect_err("local dry-run must reject an outdated receiving reference");
    assert!(
        matches!(
            &err,
            VmSdkError::Validation(v) if matches!(
                &v.source,
                IotaError::UserInput {
                    error: UserInputError::ObjectVersionUnavailableForConsumption { .. }
                }
            )
        ),
        "got {err:?}"
    );

    // The current reference receives successfully everywhere.
    let node_current = test_cluster
        .iota_client()
        .read_api()
        .dry_run_transaction_block(current_tx.clone())
        .await
        .expect("node dry-run must produce effects");
    assert!(
        matches!(node_current.effects.status(), IotaExecutionStatus::Success),
        "node dry-run must receive at the current version"
    );
    for opts in [ExecuteOptions::dev_inspect(), ExecuteOptions::dry_run()] {
        let mode = opts.mode;
        let result = vm
            .execute(current_tx.clone(), opts)
            .unwrap_or_else(|e| panic!("local {mode:?} must run to effects: {e}"));
        assert!(
            result.effects.status().is_success(),
            "local {mode:?} must receive at the current version, got {:?}",
            result.effects.status()
        );
    }
}

/// Sign and execute a `tto` move call on the cluster, asserting success.
async fn execute_tto_call(
    test_cluster: &TestCluster,
    package_id: ObjectId,
    function: &'static str,
    args: Vec<CallArg>,
) -> TransactionEffects {
    let tx = test_cluster
        .test_transaction_builder()
        .await
        .move_call(package_id, "tto", function, args)
        .build();
    let signed = test_cluster.wallet.sign_transaction(&tx);
    let (fx, _) = test_cluster
        .execute_transaction_return_raw_effects(signed)
        .await
        .expect("execute tto call");
    assert!(fx.status().is_success(), "tto::{function} must succeed");
    fx
}

/// Pick the (parent, child) pair out of `tto::start`'s created objects: the
/// child is the object owned by another created object's address.
fn parent_and_child(created: Vec<(ObjectReference, Owner)>) -> (ObjectReference, ObjectReference) {
    let created_ids: HashSet<_> = created.iter().map(|(oref, _)| oref.object_id).collect();
    let (child, parent_id) = created
        .iter()
        .find_map(|(oref, owner)| match owner {
            Owner::Address(a) if created_ids.contains(&ObjectId::from(*a)) => {
                Some((*oref, ObjectId::from(*a)))
            }
            _ => None,
        })
        .expect("start must create an object owned by another created object");
    let parent = created
        .iter()
        .find(|(oref, _)| oref.object_id == parent_id)
        .expect("the owning parent must be among the created objects");
    (parent.0, child)
}

/// The post-execution reference of the mutated object `id`.
fn mutated_ref(fx: &TransactionEffects, id: ObjectId) -> ObjectReference {
    fx.mutated_excluding_gas()
        .iter()
        .find_map(|(oref, _)| (oref.object_id == id).then_some(*oref))
        .unwrap_or_else(|| panic!("object {id} must be among the mutated objects"))
}
