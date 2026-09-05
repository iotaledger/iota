// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(msim)]
mod sim_only_tests {
    use std::{path::PathBuf, sync::Arc, time::Duration};

    use iota_core::authority::AuthorityState;
    use iota_macros::sim_test;
    use iota_node::IotaNode;
    use iota_sdk_types::{ObjectId, TransactionDigest, TransactionEffects, Version};
    use iota_test_transaction_builder::publish_package;
    use iota_types::{
        effects::TransactionEffectsAPI, messages_checkpoint::CheckpointSequenceNumber,
        storage::ObjectKey, transaction::CallArg,
    };
    use test_cluster::{TestCluster, TestClusterBuilder};
    use tokio::time::timeout;

    // Tests that relocation moves superseded object versions into the historic
    // bucket, and that the still-unchanged pruner goes on to remove the
    // lineages' tombstones the same way it always did. Specifically, we first
    // wrap a child object into a root object (tests wrap tombstone), then
    // unwrap and delete the child object (tests unwrap and delete), and last
    // delete the root object (tests object deletion).
    //
    // Pruning is disabled on this node's own schedule so it cannot race the
    // relocation checks below; the pruner is instead driven once, explicitly,
    // at the end.
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
        // pruner's tombstone sweep catches it at the end of this test.
        let object_pre_unwrap_version = superseded_version(&unwrap_delete_effects, object_id);

        let delete_root_effects = delete_object(&test_cluster, package_id, object_id).await;
        let delete_root_obj_txn_digest = *delete_root_effects.transaction_digest();
        let object_pre_delete_version = superseded_version(&delete_root_effects, object_id);

        let delete_root_checkpoint = fullnode
            .with_async(|node| async {
                // Wait for both transactions' checkpoints to execute and relocate the
                // versions they superseded.
                timeout(
                    Duration::from_secs(60),
                    wait_until_txn_in_checkpoint(node, &unwrap_delete_txn_digest),
                )
                .await
                .unwrap();
                let delete_root_checkpoint = timeout(
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

                delete_root_checkpoint
            })
            .await;

        fullnode
            .with_async(|node| async {
                let state = node.state();
                let checkpoint_store = state.get_checkpoint_store();

                // The pruner's eligibility window leaves the newest executed
                // checkpoint alone, so wait for at least one checkpoint past the
                // final delete's before driving it — otherwise the root's last
                // tombstone would not be eligible yet.
                timeout(Duration::from_secs(60), async {
                    loop {
                        if checkpoint_store
                            .get_highest_executed_checkpoint()
                            .unwrap()
                            .is_some_and(|c| c.sequence_number() > delete_root_checkpoint)
                        {
                            return;
                        }
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                })
                .await
                .unwrap();

                // The historic buckets are never pruned and the pruner itself is
                // unchanged: manually driving it here still finds and removes both
                // lineages' tombstone heads from the live table, exactly as it did
                // before relocation existed.
                state
                    .database_for_testing()
                    .prune_objects_and_compact_for_testing(checkpoint_store)
                    .await;

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
