// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(msim)]
mod sim_only_tests {
    use std::{path::PathBuf, sync::Arc, time::Duration};

    use iota_core::authority::AuthorityState;
    use iota_json_rpc_api::{GovernanceReadApiClient, ReadApiClient};
    use iota_json_rpc_types::{IotaPastObjectResponse, StakeStatus};
    use iota_macros::sim_test;
    use iota_node::IotaNode;
    use iota_sdk_types::{Identifier, ObjectId, TransactionDigest, TransactionEffects, Version};
    use iota_test_transaction_builder::publish_package;
    use iota_types::{
        effects::TransactionEffectsAPI, governance::StakedIota,
        messages_checkpoint::CheckpointSequenceNumber, storage::ObjectKey, transaction::CallArg,
    };
    use test_cluster::{TestCluster, TestClusterBuilder};
    use tokio::time::timeout;

    /// How many versions of an object the execution cache keeps once they are
    /// committed, mirroring `WritebackCache`'s own limit.
    const CACHED_OBJECT_VERSIONS: usize = 3;

    // Tests that relocation moves superseded object versions into the historic
    // bucket, and that expiring that bucket removes the lineages' tombstones
    // from the live table. Specifically, we first wrap a child object into a
    // root object (tests wrap tombstone), then unwrap and delete the child
    // object (tests unwrap and delete), and last delete the root object (tests
    // object deletion).
    //
    // Pruning is disabled on this node's own schedule so it cannot race the
    // relocation checks below; expiry is instead driven once, explicitly, at
    // the end.
    #[sim_test]
    async fn object_pruning_test() {
        let test_cluster = TestClusterBuilder::new()
            .disable_fullnode_pruning()
            .build()
            .await;
        let fullnode = &test_cluster.fullnode_handle.iota_node;

        // Create a root object and a child object. Wrap the child object inside the
        // root object.
        let (package_id, object_id) = publish_package_and_create_parent_object(&test_cluster).await;
        let child_id = create_owned_child(&test_cluster, package_id).await;
        let wrap_effects = wrap_child(&test_cluster, package_id, object_id, child_id).await;
        let wrap_child_txn_digest = *wrap_effects.transaction_digest();
        let child_pre_wrap_version = superseded_version(&wrap_effects, child_id);

        fullnode
            .with_async(|node| async {
                // Wait until the wrapping transaction's checkpoint has executed: that
                // is when relocation moves the versions it superseded out of the
                // live table and into the epoch's historic bucket.
                timeout(
                    Duration::from_secs(60),
                    wait_until_txn_in_checkpoint(node, &wrap_child_txn_digest),
                )
                .await
                .unwrap();

                let state = node.state();

                // The child's pre-wrap version has left the live table and moved into
                // the historic bucket; the live table keeps only the wrap's tombstone
                // head, so `child_id` still has exactly one version there.
                assert_relocated(&state, child_id, child_pre_wrap_version);
                assert_eq!(
                    state.database_for_testing().count_object_versions(child_id),
                    1
                );
                assert!(
                    state
                        .database_for_testing()
                        .count_object_versions(object_id)
                        > 0
                );
            })
            .await;

        // Next, we unwrap and delete the child object, as well as delete the root
        // object.
        let unwrap_delete_effects =
            unwrap_and_delete_child(&test_cluster, package_id, object_id).await;
        let unwrap_delete_txn_digest = *unwrap_delete_effects.transaction_digest();
        // Unwrapping takes only the root as an explicit input; the wrapped child
        // never appears in this transaction's `modified_at_versions`, so relocation
        // has nothing to move for it here. Its wrap tombstone stays live until the
        // bucket that recorded it expires at the end of this test.
        let object_pre_unwrap_version = superseded_version(&unwrap_delete_effects, object_id);

        let delete_root_effects = delete_object(&test_cluster, package_id, object_id).await;
        let delete_root_obj_txn_digest = *delete_root_effects.transaction_digest();
        let object_pre_delete_version = superseded_version(&delete_root_effects, object_id);

        fullnode
            .with_async(|node| async {
                // Wait for both transactions' checkpoints to execute and relocate the
                // versions they superseded.
                timeout(
                    Duration::from_secs(60),
                    wait_until_txn_in_checkpoint(node, &unwrap_delete_txn_digest),
                )
                .await
                .unwrap();
                timeout(
                    Duration::from_secs(60),
                    wait_until_txn_in_checkpoint(node, &delete_root_obj_txn_digest),
                )
                .await
                .unwrap();

                let state = node.state();

                // The root's version from just before unwrapping, and again its
                // version from just before the final delete, both moved into the
                // historic bucket as each transaction's checkpoint executed.
                assert_relocated(&state, object_id, object_pre_unwrap_version);
                assert_relocated(&state, object_id, object_pre_delete_version);
            })
            .await;

        // Both lineages' tombstone heads are recorded in the bucket of the epoch
        // that wrote them, so that epoch has to end before its bucket can fall
        // out of the retention.
        test_cluster.force_new_epoch().await;

        fullnode
            .with_async(|node| async {
                let state = node.state();

                // Expiring the earlier epoch's bucket deletes the tombstone heads
                // it recorded from the live table, so both lineages leave the
                // objects table entirely.
                state
                    .database_for_testing()
                    .expire_historic_objects_and_compact_for_testing();

                // Check that both root and child objects are gone from object store.
                assert_eq!(
                    state.database_for_testing().count_object_versions(child_id),
                    0
                );
                assert_eq!(
                    state
                        .database_for_testing()
                        .count_object_versions(object_id),
                    0
                );
            })
            .await;
    }

    // `iota_tryGetObjectBeforeVersion` bounded below an object's live version
    // has to answer from the historic bucket: relocation took every earlier
    // version out of the live table as each mutation's checkpoint executed.
    //
    // Pruning is disabled so that no bucket expiry can run between the
    // relocation checked below and the read that follows it.
    #[sim_test]
    async fn try_get_object_before_version_reads_a_relocated_version() {
        let test_cluster = TestClusterBuilder::new()
            .disable_fullnode_pruning()
            .build()
            .await;
        let fullnode = &test_cluster.fullnode_handle.iota_node;

        let (package_id, object_id) = publish_package_and_create_parent_object(&test_cluster).await;
        let created_version = test_cluster.get_latest_object_ref(&object_id).await.version;

        // The execution cache keeps the most recent versions of an object and
        // answers a bounded read from them. Mutate often enough that the
        // created version has left that window and the read has to go to
        // storage, where relocation has moved it.
        let mut last_effects = None;
        for _ in 0..CACHED_OBJECT_VERSIONS {
            last_effects = Some(create_and_wrap_child(&test_cluster, package_id, object_id).await);
        }
        let last_effects = last_effects.unwrap();
        let last_txn_digest = *last_effects.transaction_digest();

        fullnode
            .with_async(|node| async {
                timeout(
                    Duration::from_secs(60),
                    wait_until_txn_in_checkpoint(node, &last_txn_digest),
                )
                .await
                .unwrap();

                assert_relocated(&node.state(), object_id, created_version);
            })
            .await;

        let response = test_cluster
            .rpc_client()
            .try_get_object_before_version(object_id, created_version)
            .await
            .unwrap();

        let IotaPastObjectResponse::VersionFound(object_data) = response else {
            panic!("expected version {created_version} of {object_id}, got {response:?}");
        };
        assert_eq!(object_data.version, created_version);
    }

    // Withdrawing a stake deletes the `StakedIota`, so `iotax_getStakesByIds`
    // has to read the version underneath the tombstone to report the stake at
    // all: it reaches the withdrawal's tombstone head in the live table and
    // then the version below it, which relocation moved into the historic
    // bucket when the withdrawal's checkpoint executed.
    //
    // Pruning is disabled so that no bucket expiry can run between the
    // relocation checked below and the read that follows it.
    #[sim_test]
    async fn a_withdrawn_stake_still_reports_its_status() {
        let test_cluster = TestClusterBuilder::new()
            .disable_fullnode_pruning()
            .build()
            .await;
        let fullnode = &test_cluster.fullnode_handle.iota_node;

        let staked_iota_id = add_stake(&test_cluster).await;
        let withdraw_effects = withdraw_stake(&test_cluster, staked_iota_id).await;
        let withdraw_txn_digest = *withdraw_effects.transaction_digest();
        let version_before_withdrawal = superseded_version(&withdraw_effects, staked_iota_id);

        fullnode
            .with_async(|node| async {
                timeout(
                    Duration::from_secs(60),
                    wait_until_txn_in_checkpoint(node, &withdraw_txn_digest),
                )
                .await
                .unwrap();

                assert_relocated(&node.state(), staked_iota_id, version_before_withdrawal);
            })
            .await;

        let stakes = test_cluster
            .rpc_client()
            .get_stakes_by_ids(vec![staked_iota_id])
            .await
            .unwrap()
            .into_iter()
            .flat_map(|delegated| delegated.stakes)
            .collect::<Vec<_>>();

        assert_eq!(stakes.len(), 1);
        assert_eq!(stakes[0].staked_iota_id, staked_iota_id);
        assert!(matches!(stakes[0].status, StakeStatus::Unstaked));
    }

    /// The version of `id` that `effects`' transaction superseded — its
    /// pre-image is what relocation must have moved into the historic bucket.
    fn superseded_version(effects: &TransactionEffects, id: ObjectId) -> Version {
        effects
            .modified_at_versions()
            .into_iter()
            .find_map(|modified| (modified.object_id == id).then_some(modified.version))
            .unwrap_or_else(|| {
                panic!(
                    "{id} was not superseded by {:?}",
                    effects.transaction_digest()
                )
            })
    }

    /// Asserts that `version` of `id` has left the live `objects` table and
    /// arrived in the historic bucket — the two halves relocation must keep
    /// true together, since a version is always readable from one of the two.
    fn assert_relocated(state: &Arc<AuthorityState>, id: ObjectId, version: Version) {
        assert!(
            !state
                .database_for_testing()
                .object_exists_by_key(&id, version)
                .unwrap(),
            "expected version {version} of {id} to have left the live objects table"
        );
        assert!(
            state
                .get_historic_objects()
                .get(&ObjectKey(id, version))
                .unwrap()
                .is_some(),
            "expected version {version} of {id} to be in the historic bucket"
        );
    }

    async fn publish_package_and_create_parent_object(
        test_cluster: &TestCluster,
    ) -> (ObjectId, ObjectId) {
        let package_id = publish_package(
            &test_cluster.wallet,
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/move_building_blocks"),
        )
        .await
        .object_id;

        let object_id = test_cluster
            .sign_and_execute_transaction(
                &test_cluster
                    .test_transaction_builder()
                    .await
                    .move_call(package_id, "objects", "create_owned_object", vec![])
                    .build(),
            )
            .await
            .created()[0]
            .reference
            .object_id;

        (package_id, object_id)
    }

    async fn create_owned_child(test_cluster: &TestCluster, package_id: ObjectId) -> ObjectId {
        test_cluster
            .sign_and_execute_transaction(
                &test_cluster
                    .test_transaction_builder()
                    .await
                    .move_call(package_id, "objects", "create_owned_child", vec![])
                    .build(),
            )
            .await
            .created()[0]
            .reference
            .object_id
    }

    async fn create_and_wrap_child(
        test_cluster: &TestCluster,
        package_id: ObjectId,
        object_id: ObjectId,
    ) -> TransactionEffects {
        let object = test_cluster.wallet.get_object_ref(object_id).await.unwrap();
        test_cluster
            .sign_and_execute_transaction(
                &test_cluster
                    .test_transaction_builder()
                    .await
                    .move_call(
                        package_id,
                        "objects",
                        "create_and_wrap_child",
                        vec![CallArg::ImmutableOrOwned(object), CallArg::pure(&false)],
                    )
                    .build(),
            )
            .await
    }

    /// Stakes a whole gas coin with the first active validator and returns the
    /// `StakedIota` it created.
    async fn add_stake(test_cluster: &TestCluster) -> ObjectId {
        let sender = test_cluster.get_address_0();
        let mut coins = test_cluster
            .wallet
            .get_gas_objects_owned_by_address(sender, 2)
            .await
            .unwrap();
        let stake_coin = coins.pop().expect("the sender needs two gas coins");
        let gas = coins.pop().expect("the sender needs two gas coins");

        let validator_address = test_cluster
            .swarm
            .active_validators()
            .next()
            .unwrap()
            .config()
            .iota_address();

        let effects = test_cluster
            .sign_and_execute_transaction(
                &test_cluster
                    .test_transaction_builder_with_gas_object(sender, gas)
                    .await
                    .call_staking(stake_coin, validator_address)
                    .build(),
            )
            .await;

        let mut staked_iota_ids = vec![];
        for created in effects.created() {
            let object = test_cluster
                .get_object_from_fullnode_store(&created.reference.object_id)
                .await
                .unwrap();
            if StakedIota::try_from(&object).is_ok() {
                staked_iota_ids.push(created.reference.object_id);
            }
        }
        assert_eq!(staked_iota_ids.len(), 1);
        staked_iota_ids[0]
    }

    async fn withdraw_stake(
        test_cluster: &TestCluster,
        staked_iota_id: ObjectId,
    ) -> TransactionEffects {
        let staked_iota = test_cluster
            .wallet
            .get_object_ref(staked_iota_id)
            .await
            .unwrap();
        let effects = test_cluster
            .sign_and_execute_transaction(
                &test_cluster
                    .test_transaction_builder()
                    .await
                    .move_call(
                        ObjectId::SYSTEM,
                        Identifier::IOTA_SYSTEM_MODULE.as_str(),
                        "request_withdraw_stake",
                        vec![
                            CallArg::IOTA_SYSTEM_MUTABLE,
                            CallArg::ImmutableOrOwned(staked_iota),
                        ],
                    )
                    .build(),
            )
            .await;
        assert_eq!(effects.deleted().len(), 1);
        effects
    }

    async fn wrap_child(
        test_cluster: &TestCluster,
        package_id: ObjectId,
        object_id: ObjectId,
        child_id: ObjectId,
    ) -> TransactionEffects {
        let object = test_cluster.wallet.get_object_ref(object_id).await.unwrap();
        let child = test_cluster.wallet.get_object_ref(child_id).await.unwrap();
        let effects = test_cluster
            .sign_and_execute_transaction(
                &test_cluster
                    .test_transaction_builder()
                    .await
                    .move_call(
                        package_id,
                        "objects",
                        "wrap_child",
                        vec![
                            CallArg::ImmutableOrOwned(object),
                            CallArg::ImmutableOrOwned(child),
                            CallArg::pure(&true),
                        ],
                    )
                    .build(),
            )
            .await;
        assert_eq!(effects.wrapped().len(), 1);
        assert!(
            test_cluster
                .get_object_or_tombstone_from_fullnode_store(child_id)
                .await
                .digest
                .is_wrapped()
        );
        effects
    }

    async fn unwrap_and_delete_child(
        test_cluster: &TestCluster,
        package_id: ObjectId,
        object_id: ObjectId,
    ) -> TransactionEffects {
        let object = test_cluster.wallet.get_object_ref(object_id).await.unwrap();
        let effects = test_cluster
            .sign_and_execute_transaction(
                &test_cluster
                    .test_transaction_builder()
                    .await
                    .move_call(
                        package_id,
                        "objects",
                        "unwrap_and_delete_child",
                        vec![CallArg::ImmutableOrOwned(object)],
                    )
                    .build(),
            )
            .await;
        assert!(effects.deleted().is_empty());
        effects
    }

    async fn delete_object(
        test_cluster: &TestCluster,
        package_id: ObjectId,
        object_id: ObjectId,
    ) -> TransactionEffects {
        let object = test_cluster.wallet.get_object_ref(object_id).await.unwrap();
        let effects = test_cluster
            .sign_and_execute_transaction(
                &test_cluster
                    .test_transaction_builder()
                    .await
                    .move_call(
                        package_id,
                        "objects",
                        "delete",
                        vec![CallArg::ImmutableOrOwned(object)],
                    )
                    .build(),
            )
            .await;
        assert_eq!(effects.deleted().len(), 1);
        effects
    }

    async fn wait_until_txn_in_checkpoint(
        node: &IotaNode,
        digest: &TransactionDigest,
    ) -> CheckpointSequenceNumber {
        loop {
            if let Some(seq) = node
                .state()
                .epoch_store_for_testing()
                .get_transaction_checkpoint(digest)
                .unwrap()
            {
                return seq;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
