// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for white flag owned object conflict resolution.

#[cfg(test)]
mod tests {
    use iota_macros::sim_test;
    use iota_protocol_config::ProtocolConfig;
    use iota_types::{
        base_types::ObjectID,
        crypto::{AccountKeyPair, get_key_pair},
        digests::TransactionDigest,
        error::IotaError,
        messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
        object::Object,
        transaction::VerifiedTransaction,
    };

    use crate::{
        authority::authority_tests::init_state_with_objects_and_object_basics,
        consensus_handler::VerifiedSequencedConsensusTransaction,
        test_utils::make_transfer_object_transaction, white_flag,
    };

    /// Helper to create a UserTransactionV1 consensus transaction
    fn make_user_transaction_v1(tx: VerifiedTransaction) -> VerifiedSequencedConsensusTransaction {
        let consensus_tx = ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(Box::new(tx.into())),
            tracking_id: Default::default(),
        };
        VerifiedSequencedConsensusTransaction::new_test(consensus_tx)
    }

    #[sim_test]
    async fn test_white_flag_simple_conflict() {
        telemetry_subscribers::init_for_testing();

        // Enable white flag flow
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        });

        // Setup: Two transactions touching the same owned object
        let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
        let recipient1 = get_key_pair::<AccountKeyPair>().0;
        let recipient2 = get_key_pair::<AccountKeyPair>().0;

        let object_id = ObjectID::random();
        let gas1_id = ObjectID::random();
        let gas2_id = ObjectID::random();

        let objects = vec![
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas1_id, sender),
            Object::with_id_owner_for_testing(gas2_id, sender),
        ];

        let (authority, _) = init_state_with_objects_and_object_basics(objects).await;
        let epoch_store = authority.epoch_store_for_testing();
        let rgp = authority.reference_gas_price_for_testing().unwrap();

        // Get object refs
        let object = authority.get_object(&object_id).await.unwrap();
        let gas1 = authority.get_object(&gas1_id).await.unwrap();
        let gas2 = authority.get_object(&gas2_id).await.unwrap();

        // Create two conflicting transactions (both transfer the same object)
        let tx1 = make_transfer_object_transaction(
            object.compute_object_reference(),
            gas1.compute_object_reference(),
            sender,
            &sender_key,
            recipient1,
            rgp,
        );
        let tx2 = make_transfer_object_transaction(
            object.compute_object_reference(),
            gas2.compute_object_reference(),
            sender,
            &sender_key,
            recipient2,
            rgp,
        );

        // Validate transactions
        let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
        let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();

        // Create consensus transactions in order: tx1, tx2
        let mut consensus_txs = vec![
            make_user_transaction_v1(verified_tx1.clone()),
            make_user_transaction_v1(verified_tx2.clone()),
        ];

        // Run white flag conflict resolution
        let (dropped, locks) =
            white_flag::resolve_owned_object_conflicts(&epoch_store, &mut consensus_txs).unwrap();
        let (dropped_tx, _dropped_error): (Vec<TransactionDigest>, Vec<IotaError>) =
            dropped.into_iter().unzip();

        // Assert: tx1 should succeed (kept), tx2 should be dropped
        assert_eq!(consensus_txs.len(), 1, "Only one transaction should remain");
        assert_eq!(
            dropped_tx.len(),
            1,
            "Exactly one transaction should be dropped"
        );
        assert_eq!(dropped_tx[0], *verified_tx2.digest());

        // Verify locks were acquired for tx1
        assert!(
            locks.contains_key(&object.compute_object_reference()),
            "Lock should be acquired for the contested object"
        );
        assert_eq!(
            locks.get(&object.compute_object_reference()),
            Some(verified_tx1.digest())
        );
    }

    #[sim_test]
    async fn test_white_flag_no_conflict() {
        telemetry_subscribers::init_for_testing();

        // Enable white flag flow
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        });

        // Setup: Two transactions touching different owned objects
        let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
        let recipient1 = get_key_pair::<AccountKeyPair>().0;
        let recipient2 = get_key_pair::<AccountKeyPair>().0;

        let object1_id = ObjectID::random();
        let object2_id = ObjectID::random();
        let gas1_id = ObjectID::random();
        let gas2_id = ObjectID::random();

        let objects = vec![
            Object::with_id_owner_for_testing(object1_id, sender),
            Object::with_id_owner_for_testing(object2_id, sender),
            Object::with_id_owner_for_testing(gas1_id, sender),
            Object::with_id_owner_for_testing(gas2_id, sender),
        ];

        let (authority, _) = init_state_with_objects_and_object_basics(objects).await;
        let epoch_store = authority.epoch_store_for_testing();
        let rgp = authority.reference_gas_price_for_testing().unwrap();

        // Get object refs
        let object1 = authority.get_object(&object1_id).await.unwrap();
        let object2 = authority.get_object(&object2_id).await.unwrap();
        let gas1 = authority.get_object(&gas1_id).await.unwrap();
        let gas2 = authority.get_object(&gas2_id).await.unwrap();

        // Create two non-conflicting transactions
        let tx1 = make_transfer_object_transaction(
            object1.compute_object_reference(),
            gas1.compute_object_reference(),
            sender,
            &sender_key,
            recipient1,
            rgp,
        );
        let tx2 = make_transfer_object_transaction(
            object2.compute_object_reference(),
            gas2.compute_object_reference(),
            sender,
            &sender_key,
            recipient2,
            rgp,
        );

        // Validate transactions
        let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
        let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();

        // Create consensus transactions
        let mut consensus_txs = vec![
            make_user_transaction_v1(verified_tx1.clone()),
            make_user_transaction_v1(verified_tx2.clone()),
        ];

        // Run white flag conflict resolution
        let (dropped, locks) =
            white_flag::resolve_owned_object_conflicts(&epoch_store, &mut consensus_txs).unwrap();

        // Assert: Both transactions should succeed
        assert_eq!(consensus_txs.len(), 2, "Both transactions should remain");
        assert_eq!(dropped.len(), 0, "No transactions should be dropped");

        // Verify locks were acquired for both transactions
        assert_eq!(
            locks.len(),
            4,
            "Four locks should be acquired (2 objects + 2 gas)"
        );
        assert_eq!(
            locks.get(&object1.compute_object_reference()),
            Some(verified_tx1.digest())
        );
        assert_eq!(
            locks.get(&object2.compute_object_reference()),
            Some(verified_tx2.digest())
        );
    }

    #[sim_test]
    async fn test_white_flag_chain_conflict() {
        telemetry_subscribers::init_for_testing();

        // Enable white flag flow
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        });

        // Setup: Three transactions with chain conflict via shared gas:
        // - Tx1 locks object A (with gas1)
        // - Tx2 locks object B (with shared_gas) - conflicts with Tx3 on gas
        // - Tx3 locks object C (with shared_gas) - conflicts with Tx2 on gas
        // Result: Tx1 wins, Tx2 wins (first to use shared_gas), Tx3 is dropped
        let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
        let recipient1 = get_key_pair::<AccountKeyPair>().0;
        let recipient2 = get_key_pair::<AccountKeyPair>().0;
        let recipient3 = get_key_pair::<AccountKeyPair>().0;

        let object_a_id = ObjectID::random();
        let object_b_id = ObjectID::random();
        let object_c_id = ObjectID::random();
        let gas1_id = ObjectID::random();
        let shared_gas_id = ObjectID::random();

        let objects = vec![
            Object::with_id_owner_for_testing(object_a_id, sender),
            Object::with_id_owner_for_testing(object_b_id, sender),
            Object::with_id_owner_for_testing(object_c_id, sender),
            Object::with_id_owner_for_testing(gas1_id, sender),
            Object::with_id_owner_for_testing(shared_gas_id, sender),
        ];

        let (authority, _) = init_state_with_objects_and_object_basics(objects).await;
        let epoch_store = authority.epoch_store_for_testing();
        let rgp = authority.reference_gas_price_for_testing().unwrap();

        // Get object refs
        let object_a = authority.get_object(&object_a_id).await.unwrap();
        let object_b = authority.get_object(&object_b_id).await.unwrap();
        let object_c = authority.get_object(&object_c_id).await.unwrap();
        let gas1 = authority.get_object(&gas1_id).await.unwrap();
        let shared_gas = authority.get_object(&shared_gas_id).await.unwrap();

        // Tx1: Transfer object A (uses gas1)
        let tx1 = make_transfer_object_transaction(
            object_a.compute_object_reference(),
            gas1.compute_object_reference(),
            sender,
            &sender_key,
            recipient1,
            rgp,
        );

        // Tx2: Transfer object B (uses shared_gas)
        let tx2 = make_transfer_object_transaction(
            object_b.compute_object_reference(),
            shared_gas.compute_object_reference(),
            sender,
            &sender_key,
            recipient2,
            rgp,
        );

        // Tx3: Transfer object C (also uses shared_gas - conflicts with Tx2)
        let tx3 = make_transfer_object_transaction(
            object_c.compute_object_reference(),
            shared_gas.compute_object_reference(),
            sender,
            &sender_key,
            recipient3,
            rgp,
        );

        // Validate transactions
        let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
        let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();
        let verified_tx3 = epoch_store.verify_transaction(tx3).unwrap();

        // Create consensus transactions in order: tx1, tx2, tx3
        let mut consensus_txs = vec![
            make_user_transaction_v1(verified_tx1.clone()),
            make_user_transaction_v1(verified_tx2.clone()),
            make_user_transaction_v1(verified_tx3.clone()),
        ];

        // Run white flag conflict resolution
        let (dropped, locks) =
            white_flag::resolve_owned_object_conflicts(&epoch_store, &mut consensus_txs).unwrap();
        let (dropped_tx, _dropped_error): (Vec<TransactionDigest>, Vec<IotaError>) =
            dropped.into_iter().unzip();

        // Assert: Tx1 and Tx2 should succeed, Tx3 should be dropped due to gas conflict
        assert_eq!(consensus_txs.len(), 2, "Two transactions should remain");
        assert_eq!(
            dropped_tx.len(),
            1,
            "Exactly one transaction should be dropped"
        );
        assert_eq!(dropped_tx[0], *verified_tx3.digest());

        // Verify locks
        assert_eq!(
            locks.get(&object_a.compute_object_reference()),
            Some(verified_tx1.digest())
        );
        assert_eq!(
            locks.get(&object_b.compute_object_reference()),
            Some(verified_tx2.digest())
        );
        assert_eq!(
            locks.get(&shared_gas.compute_object_reference()),
            Some(verified_tx2.digest()),
            "Tx2 should have locked the shared gas"
        );
    }

    #[sim_test]
    async fn test_white_flag_multiple_conflicts_in_batch() {
        telemetry_subscribers::init_for_testing();

        // Enable white flag flow
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        });

        // Setup: Multiple transactions with multiple conflict sets
        // Conflict set 1: tx1 and tx2 both use object A
        // Conflict set 2: tx3 and tx4 both use object B
        let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();

        let object_a_id = ObjectID::random();
        let object_b_id = ObjectID::random();
        let gas1_id = ObjectID::random();
        let gas2_id = ObjectID::random();
        let gas3_id = ObjectID::random();
        let gas4_id = ObjectID::random();

        let objects = vec![
            Object::with_id_owner_for_testing(object_a_id, sender),
            Object::with_id_owner_for_testing(object_b_id, sender),
            Object::with_id_owner_for_testing(gas1_id, sender),
            Object::with_id_owner_for_testing(gas2_id, sender),
            Object::with_id_owner_for_testing(gas3_id, sender),
            Object::with_id_owner_for_testing(gas4_id, sender),
        ];

        let (authority, _) = init_state_with_objects_and_object_basics(objects).await;
        let epoch_store = authority.epoch_store_for_testing();
        let rgp = authority.reference_gas_price_for_testing().unwrap();

        let object_a = authority.get_object(&object_a_id).await.unwrap();
        let object_b = authority.get_object(&object_b_id).await.unwrap();
        let gas1 = authority.get_object(&gas1_id).await.unwrap();
        let gas2 = authority.get_object(&gas2_id).await.unwrap();
        let gas3 = authority.get_object(&gas3_id).await.unwrap();
        let gas4 = authority.get_object(&gas4_id).await.unwrap();

        let recipient = get_key_pair::<AccountKeyPair>().0;

        // Create transactions
        let tx1 = make_transfer_object_transaction(
            object_a.compute_object_reference(),
            gas1.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );
        let tx2 = make_transfer_object_transaction(
            object_a.compute_object_reference(),
            gas2.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );
        let tx3 = make_transfer_object_transaction(
            object_b.compute_object_reference(),
            gas3.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );
        let tx4 = make_transfer_object_transaction(
            object_b.compute_object_reference(),
            gas4.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );

        let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
        let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();
        let verified_tx3 = epoch_store.verify_transaction(tx3).unwrap();
        let verified_tx4 = epoch_store.verify_transaction(tx4).unwrap();

        let mut consensus_txs = vec![
            make_user_transaction_v1(verified_tx1.clone()),
            make_user_transaction_v1(verified_tx2.clone()),
            make_user_transaction_v1(verified_tx3.clone()),
            make_user_transaction_v1(verified_tx4.clone()),
        ];

        // Run white flag conflict resolution
        let (dropped, locks) =
            white_flag::resolve_owned_object_conflicts(&epoch_store, &mut consensus_txs).unwrap();
        let (dropped_tx, _dropped_error): (Vec<TransactionDigest>, Vec<IotaError>) =
            dropped.into_iter().unzip();

        // Assert: tx1 and tx3 succeed (first in each conflict set), tx2 and tx4 dropped
        assert_eq!(consensus_txs.len(), 2, "Two transactions should remain");
        assert_eq!(dropped_tx.len(), 2, "Two transactions should be dropped");
        assert!(dropped_tx.contains(verified_tx2.digest()));
        assert!(dropped_tx.contains(verified_tx4.digest()));

        // Verify locks for winners
        assert_eq!(
            locks.get(&object_a.compute_object_reference()),
            Some(verified_tx1.digest())
        );
        assert_eq!(
            locks.get(&object_b.compute_object_reference()),
            Some(verified_tx3.digest())
        );
    }

    #[sim_test]
    async fn test_white_flag_gas_object_conflict() {
        telemetry_subscribers::init_for_testing();

        // Enable white flag flow
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        });

        // Setup: Two transactions using the same gas object
        let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
        let recipient1 = get_key_pair::<AccountKeyPair>().0;
        let recipient2 = get_key_pair::<AccountKeyPair>().0;

        let object1_id = ObjectID::random();
        let object2_id = ObjectID::random();
        let shared_gas_id = ObjectID::random();

        let objects = vec![
            Object::with_id_owner_for_testing(object1_id, sender),
            Object::with_id_owner_for_testing(object2_id, sender),
            Object::with_id_owner_for_testing(shared_gas_id, sender),
        ];

        let (authority, _) = init_state_with_objects_and_object_basics(objects).await;
        let epoch_store = authority.epoch_store_for_testing();
        let rgp = authority.reference_gas_price_for_testing().unwrap();

        let object1 = authority.get_object(&object1_id).await.unwrap();
        let object2 = authority.get_object(&object2_id).await.unwrap();
        let shared_gas = authority.get_object(&shared_gas_id).await.unwrap();

        // Both transactions use the same gas object
        let tx1 = make_transfer_object_transaction(
            object1.compute_object_reference(),
            shared_gas.compute_object_reference(),
            sender,
            &sender_key,
            recipient1,
            rgp,
        );
        let tx2 = make_transfer_object_transaction(
            object2.compute_object_reference(),
            shared_gas.compute_object_reference(),
            sender,
            &sender_key,
            recipient2,
            rgp,
        );

        let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
        let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();

        let mut consensus_txs = vec![
            make_user_transaction_v1(verified_tx1.clone()),
            make_user_transaction_v1(verified_tx2.clone()),
        ];

        // Run white flag conflict resolution
        let (dropped, locks) =
            white_flag::resolve_owned_object_conflicts(&epoch_store, &mut consensus_txs).unwrap();
        let (dropped_tx, _dropped_error): (Vec<TransactionDigest>, Vec<IotaError>) =
            dropped.into_iter().unzip();

        // Assert: tx1 succeeds, tx2 is dropped due to gas conflict
        assert_eq!(consensus_txs.len(), 1, "Only one transaction should remain");
        assert_eq!(dropped_tx.len(), 1, "One transaction should be dropped");
        assert_eq!(dropped_tx[0], *verified_tx2.digest());

        // Verify locks for gas and object
        assert_eq!(
            locks.get(&shared_gas.compute_object_reference()),
            Some(verified_tx1.digest())
        );
        assert_eq!(
            locks.get(&object1.compute_object_reference()),
            Some(verified_tx1.digest())
        );
    }

    #[sim_test]
    async fn test_white_flag_winner_blocks_multiple_losers() {
        telemetry_subscribers::init_for_testing();

        // Enable white flag flow
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        });

        // Setup: Three transactions where tx1 uses both A and B, blocking tx2 and tx3
        // - Tx1 uses objects A and B (wins, locks both)
        // - Tx2 uses object A (dropped, conflicts with Tx1)
        // - Tx3 uses object B (dropped, conflicts with Tx1)
        let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
        let recipient = get_key_pair::<AccountKeyPair>().0;

        let object_a_id = ObjectID::random();
        let object_b_id = ObjectID::random();
        let gas1_id = ObjectID::random();
        let gas2_id = ObjectID::random();
        let gas3_id = ObjectID::random();

        let objects = vec![
            Object::with_id_owner_for_testing(object_a_id, sender),
            Object::with_id_owner_for_testing(object_b_id, sender),
            Object::with_id_owner_for_testing(gas1_id, sender),
            Object::with_id_owner_for_testing(gas2_id, sender),
            Object::with_id_owner_for_testing(gas3_id, sender),
        ];

        let (authority, package_ref) = init_state_with_objects_and_object_basics(objects).await;
        let epoch_store = authority.epoch_store_for_testing();
        let rgp = authority.reference_gas_price_for_testing().unwrap();

        let object_a = authority.get_object(&object_a_id).await.unwrap();
        let object_b = authority.get_object(&object_b_id).await.unwrap();
        let gas1 = authority.get_object(&gas1_id).await.unwrap();
        let gas2 = authority.get_object(&gas2_id).await.unwrap();
        let gas3 = authority.get_object(&gas3_id).await.unwrap();

        // Tx1: Move call that uses both object A and B
        use iota_types::transaction::{CallArg, ObjectArg, TransactionData};
        use move_core_types::ident_str;

        let tx1_data = TransactionData::new_move_call(
            sender,
            package_ref.0,
            ident_str!("object_basics").to_owned(),
            ident_str!("update").to_owned(),
            vec![],
            gas1.compute_object_reference(),
            vec![
                CallArg::Object(ObjectArg::ImmOrOwnedObject(
                    object_a.compute_object_reference(),
                )),
                CallArg::Object(ObjectArg::ImmOrOwnedObject(
                    object_b.compute_object_reference(),
                )),
            ],
            rgp * 1000,
            rgp,
        )
        .unwrap();
        let tx1 = iota_types::utils::to_sender_signed_transaction(tx1_data, &sender_key);

        // Tx2: Transfer object A (will conflict with tx1)
        let tx2 = make_transfer_object_transaction(
            object_a.compute_object_reference(),
            gas2.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );

        // Tx3: Transfer object B (will conflict with tx1)
        let tx3 = make_transfer_object_transaction(
            object_b.compute_object_reference(),
            gas3.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );

        let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
        let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();
        let verified_tx3 = epoch_store.verify_transaction(tx3).unwrap();

        let mut consensus_txs = vec![
            make_user_transaction_v1(verified_tx1.clone()),
            make_user_transaction_v1(verified_tx2.clone()),
            make_user_transaction_v1(verified_tx3.clone()),
        ];

        // Run white flag conflict resolution
        let (dropped, locks) =
            white_flag::resolve_owned_object_conflicts(&epoch_store, &mut consensus_txs).unwrap();
        let (dropped_tx, _dropped_error): (Vec<TransactionDigest>, Vec<IotaError>) =
            dropped.into_iter().unzip();

        // Assert: Only tx1 succeeds, tx2 and tx3 are both dropped
        assert_eq!(consensus_txs.len(), 1, "Only one transaction should remain");
        assert_eq!(dropped_tx.len(), 2, "Two transactions should be dropped");
        assert!(dropped_tx.contains(verified_tx2.digest()));
        assert!(dropped_tx.contains(verified_tx3.digest()));

        // Verify tx1 locked both objects
        assert_eq!(
            locks.get(&object_a.compute_object_reference()),
            Some(verified_tx1.digest())
        );
        assert_eq!(
            locks.get(&object_b.compute_object_reference()),
            Some(verified_tx1.digest())
        );
    }

    #[sim_test]
    async fn test_white_flag_complex_chain_with_gas_conflicts() {
        telemetry_subscribers::init_for_testing();

        // Enable white flag flow
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        });

        // Setup: Tests that dropped transactions don't lock objects
        // Tx1: object A with shared_gas (wins, locks both)
        // Tx2: object A with gas1 (drops - object A conflict with tx1)
        // Tx3: object B with shared_gas (drops - shared_gas conflict with tx1)
        // Tx4: object B with gas2 (wins - object B is free since tx3 was dropped!)
        // This verifies that tx4 succeeds even though tx3 tried to use object B earlier
        let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
        let recipient = get_key_pair::<AccountKeyPair>().0;

        let object_a_id = ObjectID::random();
        let object_b_id = ObjectID::random();
        let gas1_id = ObjectID::random();
        let gas2_id = ObjectID::random();
        let shared_gas_id = ObjectID::random();

        let objects = vec![
            Object::with_id_owner_for_testing(object_a_id, sender),
            Object::with_id_owner_for_testing(object_b_id, sender),
            Object::with_id_owner_for_testing(gas1_id, sender),
            Object::with_id_owner_for_testing(gas2_id, sender),
            Object::with_id_owner_for_testing(shared_gas_id, sender),
        ];

        let (authority, _) = init_state_with_objects_and_object_basics(objects).await;
        let epoch_store = authority.epoch_store_for_testing();
        let rgp = authority.reference_gas_price_for_testing().unwrap();

        let object_a = authority.get_object(&object_a_id).await.unwrap();
        let object_b = authority.get_object(&object_b_id).await.unwrap();
        let gas1 = authority.get_object(&gas1_id).await.unwrap();
        let gas2 = authority.get_object(&gas2_id).await.unwrap();
        let shared_gas = authority.get_object(&shared_gas_id).await.unwrap();

        // Tx1: object A with shared_gas (first, will win and lock shared_gas)
        let tx1 = make_transfer_object_transaction(
            object_a.compute_object_reference(),
            shared_gas.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );

        // Tx2: object A with gas1 (conflicts with tx1 on object A)
        let tx2 = make_transfer_object_transaction(
            object_a.compute_object_reference(),
            gas1.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );

        // Tx3: object B with shared_gas (conflicts with tx1 on shared_gas)
        let tx3 = make_transfer_object_transaction(
            object_b.compute_object_reference(),
            shared_gas.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );

        // Tx4: object B with gas2 (should succeed - object B is free since tx3 was
        // dropped)
        let tx4 = make_transfer_object_transaction(
            object_b.compute_object_reference(),
            gas2.compute_object_reference(),
            sender,
            &sender_key,
            recipient,
            rgp,
        );

        let verified_tx1 = epoch_store.verify_transaction(tx1).unwrap();
        let verified_tx2 = epoch_store.verify_transaction(tx2).unwrap();
        let verified_tx3 = epoch_store.verify_transaction(tx3).unwrap();
        let verified_tx4 = epoch_store.verify_transaction(tx4).unwrap();

        let mut consensus_txs = vec![
            make_user_transaction_v1(verified_tx1.clone()),
            make_user_transaction_v1(verified_tx2.clone()),
            make_user_transaction_v1(verified_tx3.clone()),
            make_user_transaction_v1(verified_tx4.clone()),
        ];

        // Run white flag conflict resolution
        let (dropped, locks) =
            white_flag::resolve_owned_object_conflicts(&epoch_store, &mut consensus_txs).unwrap();
        let (dropped_tx, _dropped_error): (Vec<TransactionDigest>, Vec<IotaError>) =
            dropped.into_iter().unzip();

        // Assert: tx1 and tx4 win, tx2 and tx3 dropped
        // tx2 drops due to object A conflict with tx1
        // tx3 drops due to shared_gas conflict with tx1
        // tx4 succeeds because object B is free (tx3 didn't lock it)
        assert_eq!(consensus_txs.len(), 2, "Two transactions should remain");
        assert_eq!(dropped_tx.len(), 2, "Two transactions should be dropped");
        assert!(dropped_tx.contains(verified_tx2.digest()));
        assert!(dropped_tx.contains(verified_tx3.digest()));

        // Verify locks for winners
        assert_eq!(
            locks.get(&object_a.compute_object_reference()),
            Some(verified_tx1.digest()),
            "tx1 should lock object A"
        );
        assert_eq!(
            locks.get(&shared_gas.compute_object_reference()),
            Some(verified_tx1.digest()),
            "tx1 should lock shared_gas"
        );
        assert_eq!(
            locks.get(&object_b.compute_object_reference()),
            Some(verified_tx4.digest()),
            "tx4 should lock object B (tx3 was dropped before locking)"
        );
        assert_eq!(
            locks.get(&gas2.compute_object_reference()),
            Some(verified_tx4.digest()),
            "tx4 should lock gas2"
        );
        // gas1 should not be locked since tx2 was dropped
        assert!(!locks.contains_key(&gas1.compute_object_reference()));
    }
}
