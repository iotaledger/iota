// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, HashMap, HashSet};

use iota_sdk_types::{ObjectId, VersionAssignment};
use iota_types::{
    base_types::{SequenceNumber, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::IotaResult,
    executable_transaction::VerifiedExecutableTransaction,
    object::OBJECT_START_VERSION,
    storage::{
        ObjectKey, transaction_non_shared_input_object_keys, transaction_receiving_object_keys,
    },
    transaction::{SenderSignedData, SharedObjectRef, TransactionDataAPI, TransactionKey},
};
use tracing::trace;

use crate::{
    authority::{
        AuthorityPerEpochStore, authority_per_epoch_store::CancelConsensusTransactionReason,
    },
    execution_cache::ObjectCacheRead,
};

pub struct SharedObjVerManager {}

pub type AssignedTxAndVersions = Vec<(TransactionKey, Vec<VersionAssignment>)>;

#[must_use]
#[derive(Default)]
pub struct ConsensusSharedObjVerAssignment {
    pub shared_input_next_versions: HashMap<ObjectId, SequenceNumber>,
    pub assigned_versions: AssignedTxAndVersions,
}

impl SharedObjVerManager {
    pub fn assign_versions_from_consensus<'a>(
        epoch_store: &AuthorityPerEpochStore,
        cache_reader: &dyn ObjectCacheRead,
        transactions: impl Iterator<Item = &'a VerifiedExecutableTransaction> + Clone,
        cancelled_txns: &BTreeMap<TransactionDigest, CancelConsensusTransactionReason>,
    ) -> IotaResult<ConsensusSharedObjVerAssignment> {
        let mut shared_input_next_versions = get_or_init_versions(
            transactions.clone().map(|tx| tx.data()),
            epoch_store,
            cache_reader,
        )?;
        let mut assigned_versions = Vec::new();
        let assign_implicit_accounts_flag =
            epoch_store.protocol_config().enable_implicit_accounts();
        for transaction in transactions {
            // A transaction carrying no declared shared object but a implicit
            // account MUST still enter assignment so the account is pinned to
            // `OBJECT_START_VERSION`. Pinning the implicit account
            // keeps its read deterministic even when a claim of the same account
            // executes concurrently in a later commit.
            let has_account = assign_implicit_accounts_flag
                && !transaction.data().implicit_account_objects()?.is_empty();
            if !transaction.contains_shared_object() && !has_account {
                continue;
            }
            let tx_assigned_versions = Self::assign_versions_for_transaction(
                transaction,
                &mut shared_input_next_versions,
                cancelled_txns,
                epoch_store
                    .protocol_config()
                    .congestion_control_gas_price_feedback_mechanism(),
                assign_implicit_accounts_flag,
            );
            assigned_versions.push((transaction.key(), tx_assigned_versions));
        }

        Ok(ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        })
    }

    pub fn assign_versions_from_effects(
        transactions_and_effects: &[(&VerifiedExecutableTransaction, &TransactionEffects)],
        epoch_store: &AuthorityPerEpochStore,
        cache_reader: &dyn ObjectCacheRead,
    ) -> AssignedTxAndVersions {
        // We don't care about the results since we can use effects to assign versions.
        // But we must call it to make sure whenever a shared object is touched the
        // first time during an epoch, either through consensus or through
        // checkpoint executor, its next version must be initialized. This is
        // because we initialize the next version of a shared object in an epoch
        // by reading the current version from the object store. This must be
        // done before we mutate it the first time, otherwise we would be initializing
        // it with the wrong version.
        let _ = get_or_init_versions(
            transactions_and_effects.iter().map(|(tx, _)| tx.data()),
            epoch_store,
            cache_reader,
        );

        let mut assigned_versions = Vec::new();
        for (transaction, effects) in transactions_and_effects {
            let tx_assigned_versions: Vec<_> = effects
                .input_shared_objects()
                .into_iter()
                .map(|iso| {
                    let (object_id, version) = iso.id_and_version();
                    VersionAssignment::new(object_id, version)
                })
                .collect();
            let tx_key = transaction.key();
            trace!(
                ?tx_key,
                ?tx_assigned_versions,
                "locking shared objects from effects"
            );
            assigned_versions.push((tx_key, tx_assigned_versions));
        }
        assigned_versions
    }

    pub fn assign_versions_for_transaction(
        transaction: &VerifiedExecutableTransaction,
        shared_input_next_versions: &mut HashMap<ObjectId, SequenceNumber>,
        cancelled_txns: &BTreeMap<TransactionDigest, CancelConsensusTransactionReason>,
        enable_gas_price_feedback: bool,
        assign_implicit_accounts_flag: bool,
    ) -> Vec<VersionAssignment> {
        let tx_digest = transaction.digest();

        // Check if the transaction is cancelled due to congestion.
        let cancellation_info = cancelled_txns.get(tx_digest);
        let congested_objects_info: Option<HashSet<_>> =
            if let Some(CancelConsensusTransactionReason::CongestionOnObjects {
                congested_objects,
                suggested_gas_price: _,
            }) = &cancellation_info
            {
                Some(congested_objects.iter().cloned().collect())
            } else {
                None
            };
        let txn_cancelled = cancellation_info.is_some();

        // Make an iterator to update the locks of the transaction's shared objects.
        let shared_input_objects = transaction.shared_input_objects();

        let mut input_object_keys = transaction_non_shared_input_object_keys(transaction)
            .expect("Transaction input should have been verified");
        let mut assigned_versions = Vec::with_capacity(shared_input_objects.len());
        let mut is_mutable_input = Vec::with_capacity(shared_input_objects.len());
        // Record receiving object versions towards the shared version computation.
        let receiving_object_keys = transaction_receiving_object_keys(transaction);
        input_object_keys.extend(receiving_object_keys);

        if txn_cancelled {
            // For cancelled transaction due to congestion, assign special versions to all
            // shared objects. Note that new lamport version does not depend on
            // any shared objects.
            for SharedObjectRef { object_id: id, .. } in shared_input_objects.iter() {
                let assigned_version = match cancellation_info {
                    Some(CancelConsensusTransactionReason::CongestionOnObjects {
                        congested_objects: _,
                        suggested_gas_price,
                    }) => {
                        if congested_objects_info
                            .as_ref()
                            .is_some_and(|info| info.contains(id))
                        {
                            if enable_gas_price_feedback {
                                SequenceNumber::new_congested_with_suggested_gas_price(
                                    suggested_gas_price.expect(
                                        "Suggested gas price for transactions cancelled due \
                                            to congestion must not be None if the gas price \
                                            feedback is enabled.",
                                    ),
                                )
                                .unwrap()
                            } else {
                                // WARN: do not remove this `else` branch even after
                                // `congestion_control_gas_price_feedback_mechanism` is enabled
                                // on the mainnet. It must be kept to be able to replay old
                                // transaction data.
                                SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK
                            }
                        } else {
                            SequenceNumber::CANCELLED_READ
                        }
                    }
                    Some(CancelConsensusTransactionReason::DkgFailed) => {
                        if id == &ObjectId::RANDOMNESS_STATE {
                            SequenceNumber::RANDOMNESS_UNAVAILABLE
                        } else {
                            SequenceNumber::CANCELLED_READ
                        }
                    }
                    None => unreachable!("cancelled transaction should have cancellation info"),
                };
                assigned_versions.push(VersionAssignment::new(*id, assigned_version));
                is_mutable_input.push(false);
            }
        } else {
            for (
                SharedObjectRef {
                    object_id: id,
                    mutable,
                    ..
                },
                assigned_version,
            ) in shared_input_objects.iter().map(|obj| {
                (
                    obj,
                    *shared_input_next_versions.get(&obj.object_id).unwrap(),
                )
            }) {
                assigned_versions.push(VersionAssignment::new(*id, assigned_version));
                input_object_keys.push(ObjectKey(*id, assigned_version));
                is_mutable_input.push(*mutable);
            }

            // Pin each implicit account to `OBJECT_START_VERSION` as a read-only
            // input, so its execution read is deterministic.
            // This only covers implicit accounts: a claimed account is created at its
            // claim's Lamport version (>= 2), so reading `A@OBJECT_START_VERSION` always
            // misses → the account resolves to the implicit account, even if a
            // claim of it executes concurrently in a later commit. Accounts
            // already declared as shared inputs are skipped (handled above with
            // their real mutability). `mutable: false` keeps accounts out of
            // the Lamport bump below, and this pin is per-transaction only
            // (never written back to `shared_input_next_versions`).
            if assign_implicit_accounts_flag {
                let declared_shared_ids: HashSet<ObjectId> = shared_input_objects
                    .iter()
                    .map(|obj| obj.object_id)
                    .collect();
                for account_id in transaction
                    .data()
                    .implicit_account_objects()
                    .expect("implicit account ids derive for a verified transaction")
                {
                    if declared_shared_ids.contains(&account_id) {
                        continue;
                    }
                    assigned_versions
                        .push(VersionAssignment::new(account_id, OBJECT_START_VERSION));
                    input_object_keys.push(ObjectKey(account_id, OBJECT_START_VERSION));
                    is_mutable_input.push(false);
                }
            }
        }

        let next_version =
            SequenceNumber::lamport_increment(input_object_keys.iter().map(|obj| obj.1)).unwrap();
        assert!(
            next_version.is_valid(),
            "Assigned version must be valid. Got {next_version:?}"
        );

        if !txn_cancelled {
            // Update the next version for the shared objects.
            assigned_versions
                .iter()
                .zip(is_mutable_input)
                .filter_map(|(VersionAssignment { object_id, .. }, mutable)| {
                    if mutable {
                        Some(VersionAssignment::new(*object_id, next_version))
                    } else {
                        None
                    }
                })
                .for_each(|VersionAssignment { object_id, version }| {
                    assert!(
                        version.is_valid(),
                        "Assigned version must be a valid version."
                    );
                    shared_input_next_versions
                        .insert(object_id, version)
                        .expect("Object must exist in shared_input_next_versions.");
                });
        }

        // This is a special case for transactions that are claiming an account.
        // Record the freshly-claimed account in `shared_input_next_versions` so that,
        // for the rest of the epoch, it is a consensus-deterministic marker that the
        // account has been claimed. A plain-signature use of the account in a LATER
        // consensus commit reads this marker in post-consensus validation
        // (`get_existing_next_shared_object_versions`) and is rejected,
        // deterministically, without depending on whether the claim's effects
        // have executed yet. Only the presence of the entry matters; the value
        // (the claim's Lamport version) is not read. Same-commit transfers are
        // dropped by the claim-conflict whiteflag before reaching version
        // assignment. Claims cannot be sponsored, so only the sender's
        // account is recorded.
        if assign_implicit_accounts_flag
            && !txn_cancelled
            && transaction
                .data()
                .transaction_data()
                .calls_smart_account_build_v1()
        {
            let account_id = ObjectId::from(transaction.data().transaction_data().sender());
            shared_input_next_versions.insert(account_id, next_version);
        }

        trace!(
            ?tx_digest,
            ?assigned_versions,
            ?next_version,
            ?txn_cancelled,
            "locking shared objects"
        );

        assigned_versions
    }
}

fn get_or_init_versions<'a>(
    transactions: impl Iterator<Item = &'a SenderSignedData>,
    epoch_store: &AuthorityPerEpochStore,
    cache_reader: &dyn ObjectCacheRead,
) -> IotaResult<HashMap<ObjectId, SequenceNumber>> {
    let mut shared_input_objects: Vec<_> = transactions
        .flat_map(|tx| {
            tx.shared_input_objects()
                .into_iter()
                .map(|so| (so.object_id, so.initial_shared_version))
        })
        .collect();

    shared_input_objects.sort();
    shared_input_objects.dedup();

    epoch_store.get_or_init_next_object_versions(&shared_input_objects, cache_reader)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_types::{Address, Identifier, RandomnessRound};
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        IOTA_FRAMEWORK_PACKAGE_ID,
        base_types::{ObjectRef, SequenceNumber},
        digests::ObjectDigest,
        effects::TestEffectsBuilder,
        executable_transaction::{
            CertificateProof, ExecutableTransaction, VerifiedExecutableTransaction,
        },
        object::Object,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{CallArg, SenderSignedData, VerifiedTransaction},
    };

    use super::*;
    use crate::authority::{
        authority_per_epoch_store::CancelConsensusTransactionReason,
        epoch_start_configuration::EpochStartConfigTrait,
        shared_object_version_manager::{ConsensusSharedObjVerAssignment, SharedObjVerManager},
        test_authority_builder::TestAuthorityBuilder,
    };

    #[tokio::test]
    async fn test_assign_versions_from_consensus_basic() {
        let shared_object = Object::shared_for_testing();
        let id = shared_object.id();
        let init_shared_version = shared_object
            .owner
            .into_shared_opt()
            .expect("expected shared object");
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(std::slice::from_ref(&shared_object))
            .build()
            .await;
        let transactions = [
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, true)], 3),
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, false)], 5),
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, true)], 9),
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, true)], 11),
        ];
        let epoch_store = authority.epoch_store_for_testing();
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            transactions.iter(),
            &BTreeMap::new(),
        )
        .unwrap();
        // Check that the shared object's next version is always initialized in the
        // epoch store.
        assert_eq!(
            epoch_store.get_next_object_version(&id).unwrap(),
            init_shared_version
        );
        // Check that the final version of the shared object is the lamport version of
        // the last transaction.
        assert_eq!(
            shared_input_next_versions,
            HashMap::from([(id, SequenceNumber::from_u64(12))])
        );
        // Check that the version assignment for each transaction is correct.
        // For a transaction that uses the shared object with mutable=false, it won't
        // update the version using lamport version, hence the next transaction
        // will use the same version number. In the following case, transactions[2] has
        // the same assignment as transactions[1] for this reason.
        assert_eq!(
            assigned_versions,
            vec![
                (
                    transactions[0].key(),
                    vec![VersionAssignment::new(id, init_shared_version)]
                ),
                (
                    transactions[1].key(),
                    vec![VersionAssignment::new(id, SequenceNumber::from_u64(4))]
                ),
                (
                    transactions[2].key(),
                    vec![VersionAssignment::new(id, SequenceNumber::from_u64(4))]
                ),
                (
                    transactions[3].key(),
                    vec![VersionAssignment::new(id, SequenceNumber::from_u64(10))]
                ),
            ]
        );
    }

    #[tokio::test]
    async fn test_assign_versions_from_consensus_with_randomness() {
        let authority = TestAuthorityBuilder::new().build().await;
        let epoch_store = authority.epoch_store_for_testing();
        let randomness_obj_version = epoch_store
            .epoch_start_config()
            .randomness_obj_initial_shared_version();
        let transactions = [
            VerifiedExecutableTransaction::new_system(
                VerifiedTransaction::new_randomness_state_update(
                    epoch_store.epoch(),
                    RandomnessRound::new(1),
                    vec![],
                    randomness_obj_version,
                ),
                epoch_store.epoch(),
            ),
            generate_shared_objs_tx_with_gas_version(
                &[(
                    ObjectId::RANDOMNESS_STATE,
                    randomness_obj_version,
                    // This can only be false since it's not allowed to use randomness object with
                    // mutable=true.
                    false,
                )],
                3,
            ),
            generate_shared_objs_tx_with_gas_version(
                &[(ObjectId::RANDOMNESS_STATE, randomness_obj_version, false)],
                5,
            ),
        ];
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            transactions.iter(),
            &BTreeMap::new(),
        )
        .unwrap();
        // Check that the randomness object's next version is initialized.
        assert_eq!(
            epoch_store
                .get_next_object_version(&ObjectId::RANDOMNESS_STATE)
                .unwrap(),
            randomness_obj_version
        );
        let next_randomness_obj_version = randomness_obj_version.next().unwrap();
        assert_eq!(
            shared_input_next_versions,
            // Randomness object's version is only incremented by 1 regardless of lamport version.
            HashMap::from([(ObjectId::RANDOMNESS_STATE, next_randomness_obj_version)])
        );
        assert_eq!(
            assigned_versions,
            vec![
                (
                    transactions[0].key(),
                    vec![VersionAssignment::new(
                        ObjectId::RANDOMNESS_STATE,
                        randomness_obj_version
                    )]
                ),
                (
                    transactions[1].key(),
                    // It is critical that the randomness object version is updated before the
                    // assignment.
                    vec![VersionAssignment::new(
                        ObjectId::RANDOMNESS_STATE,
                        next_randomness_obj_version
                    )]
                ),
                (
                    transactions[2].key(),
                    // It is critical that the randomness object version is updated before the
                    // assignment.
                    vec![VersionAssignment::new(
                        ObjectId::RANDOMNESS_STATE,
                        next_randomness_obj_version
                    )]
                ),
            ]
        );
    }

    // Tests shared object version assignment for cancelled transaction.
    #[tokio::test]
    async fn test_assign_versions_from_consensus_with_cancellation() {
        let shared_object_1 = Object::shared_for_testing();
        let shared_object_2 = Object::shared_for_testing();
        let id1 = shared_object_1.id();
        let id2 = shared_object_2.id();
        let init_shared_version_1 = shared_object_1
            .owner
            .into_shared_opt()
            .expect("expected shared object");
        let init_shared_version_2 = shared_object_2
            .owner
            .into_shared_opt()
            .expect("expected shared object");
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(&[shared_object_1.clone(), shared_object_2.clone()])
            .build()
            .await;
        let randomness_obj_version = authority
            .epoch_store_for_testing()
            .epoch_start_config()
            .randomness_obj_initial_shared_version();

        // Generate 5 transactions for testing.
        //   tx1: shared_object_1, shared_object_2, owned_object_version = 3
        //   tx2: shared_object_1, shared_object_2, owned_object_version = 5
        //   tx3: shared_object_1, owned_object_version = 1
        //   tx4: shared_object_1, shared_object_2, owned_object_version = 9
        //   tx5: shared_object_1, shared_object_2, owned_object_version = 11
        //
        // Later, we cancel transaction 2 and 4 due to congestion, and 5 due to DKG
        // failure. Expected outcome:
        //   tx1: both shared objects assign version 1, lamport version = 4
        //   tx2: shared objects assign cancelled version, lamport version = 6 due to
        // gas object version = 5   tx3: shared object 1 assign version 4,
        // lamport version = 5   tx4: shared objects assign cancelled version,
        // lamport version = 10 due to gas object version = 9   tx5: shared
        // objects assign cancelled version, lamport version = 12 due to gas object
        // version = 11
        let transactions = [
            generate_shared_objs_tx_with_gas_version(
                &[
                    (id1, init_shared_version_1, true),
                    (id2, init_shared_version_2, true),
                ],
                3,
            ),
            generate_shared_objs_tx_with_gas_version(
                &[
                    (id1, init_shared_version_1, true),
                    (id2, init_shared_version_2, true),
                ],
                5,
            ),
            generate_shared_objs_tx_with_gas_version(&[(id1, init_shared_version_1, true)], 1),
            generate_shared_objs_tx_with_gas_version(
                &[
                    (id1, init_shared_version_1, true),
                    (id2, init_shared_version_2, true),
                ],
                9,
            ),
            generate_shared_objs_tx_with_gas_version(
                &[
                    (ObjectId::RANDOMNESS_STATE, randomness_obj_version, false),
                    (id2, init_shared_version_2, true),
                ],
                11,
            ),
        ];
        let epoch_store = authority.epoch_store_for_testing();

        // Cancel transactions 2 and 4 due to congestion.
        let suggested_gas_price = 1_000;
        let cancelled_txns: BTreeMap<TransactionDigest, CancelConsensusTransactionReason> = [
            (
                *transactions[1].digest(),
                CancelConsensusTransactionReason::CongestionOnObjects {
                    congested_objects: vec![id1],
                    suggested_gas_price: Some(suggested_gas_price),
                },
            ),
            (
                *transactions[3].digest(),
                CancelConsensusTransactionReason::CongestionOnObjects {
                    congested_objects: vec![id2],
                    suggested_gas_price: Some(suggested_gas_price),
                },
            ),
            (
                *transactions[4].digest(),
                CancelConsensusTransactionReason::DkgFailed,
            ),
        ]
        .into_iter()
        .collect();

        // Run version assignment logic.
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            transactions.iter(),
            &cancelled_txns,
        )
        .unwrap();

        // Check that the final version of the shared object is the lamport version of
        // the last transaction.
        assert_eq!(
            shared_input_next_versions,
            HashMap::from([
                (id1, SequenceNumber::from_u64(5)), // determined by tx3
                (id2, SequenceNumber::from_u64(4)), // determined by tx1
                (ObjectId::RANDOMNESS_STATE, SequenceNumber::from_u64(1)), // not mutable
            ])
        );

        // Check that the version assignment for each transaction is correct.
        assert_eq!(
            assigned_versions,
            vec![
                (
                    transactions[0].key(),
                    vec![
                        VersionAssignment::new(id1, init_shared_version_1),
                        VersionAssignment::new(id2, init_shared_version_2)
                    ]
                ),
                (
                    transactions[1].key(),
                    vec![
                        VersionAssignment::new(
                            id1,
                            SequenceNumber::new_congested_with_suggested_gas_price(
                                suggested_gas_price
                            )
                            .unwrap(),
                        ),
                        VersionAssignment::new(id2, SequenceNumber::CANCELLED_READ),
                    ]
                ),
                (
                    transactions[2].key(),
                    vec![VersionAssignment::new(id1, SequenceNumber::from_u64(4))]
                ),
                (
                    transactions[3].key(),
                    vec![
                        VersionAssignment::new(id1, SequenceNumber::CANCELLED_READ),
                        VersionAssignment::new(
                            id2,
                            SequenceNumber::new_congested_with_suggested_gas_price(
                                suggested_gas_price
                            )
                            .unwrap(),
                        )
                    ]
                ),
                (
                    transactions[4].key(),
                    vec![
                        VersionAssignment::new(
                            ObjectId::RANDOMNESS_STATE,
                            SequenceNumber::RANDOMNESS_UNAVAILABLE
                        ),
                        VersionAssignment::new(id2, SequenceNumber::CANCELLED_READ)
                    ]
                ),
            ]
        );
    }

    #[tokio::test]
    async fn test_assign_versions_from_effects() {
        let shared_object = Object::shared_for_testing();
        let id = shared_object.id();
        let init_shared_version = shared_object
            .owner
            .into_shared_opt()
            .expect("expected shared object");
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(std::slice::from_ref(&shared_object))
            .build()
            .await;
        let transactions = [
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, true)], 3),
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, false)], 5),
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, true)], 9),
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, true)], 11),
        ];
        let effects = [
            TestEffectsBuilder::new(transactions[0].data()).build(),
            TestEffectsBuilder::new(transactions[1].data())
                .with_shared_input_versions(BTreeMap::from([(id, SequenceNumber::from_u64(4))]))
                .build(),
            TestEffectsBuilder::new(transactions[2].data())
                .with_shared_input_versions(BTreeMap::from([(id, SequenceNumber::from_u64(4))]))
                .build(),
            TestEffectsBuilder::new(transactions[3].data())
                .with_shared_input_versions(BTreeMap::from([(id, SequenceNumber::from_u64(10))]))
                .build(),
        ];
        let epoch_store = authority.epoch_store_for_testing();
        let assigned_versions = SharedObjVerManager::assign_versions_from_effects(
            transactions
                .iter()
                .zip(effects.iter())
                .collect::<Vec<_>>()
                .as_slice(),
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
        );
        // Check that the shared object's next version is always initialized in the
        // epoch store.
        assert_eq!(
            epoch_store.get_next_object_version(&id).unwrap(),
            init_shared_version
        );
        assert_eq!(
            assigned_versions,
            vec![
                (
                    transactions[0].key(),
                    vec![VersionAssignment::new(id, init_shared_version),]
                ),
                (
                    transactions[1].key(),
                    vec![VersionAssignment::new(id, SequenceNumber::from_u64(4)),]
                ),
                (
                    transactions[2].key(),
                    vec![VersionAssignment::new(id, SequenceNumber::from_u64(4)),]
                ),
                (
                    transactions[3].key(),
                    vec![VersionAssignment::new(id, SequenceNumber::from_u64(10)),]
                ),
            ]
        );
    }

    /// Generate a transaction that uses shared objects as specified in the
    /// parameters. Also uses a gas object with specified version.
    /// The version of the gas object is used to manipulate the lamport version
    /// of this transaction.
    fn generate_shared_objs_tx_with_gas_version(
        shared_objects: &[(ObjectId, SequenceNumber, bool)],
        gas_object_version: u64,
    ) -> VerifiedExecutableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        for (shared_object_id, shared_object_init_version, shared_object_mutable) in shared_objects
        {
            builder
                .obj(CallArg::Shared(SharedObjectRef::new(
                    *shared_object_id,
                    *shared_object_init_version,
                    *shared_object_mutable,
                )))
                .unwrap();
        }
        let tx_data = TestTransactionBuilder::new(
            Address::ZERO,
            ObjectRef::new(
                ObjectId::random(),
                SequenceNumber::from_u64(gas_object_version),
                ObjectDigest::random(),
            ),
            0,
        )
        .programmable(builder.finish())
        .build();
        let tx = SenderSignedData::new(tx_data, vec![]);
        VerifiedExecutableTransaction::new_unchecked(ExecutableTransaction::new_from_data_and_sig(
            tx,
            CertificateProof::new_system(0),
        ))
    }

    fn generate_claim_tx(
        shared_objects: &[(ObjectId, SequenceNumber, bool)],
        sender: Address,
        gas_object_version: u64,
    ) -> VerifiedExecutableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        for (id, init, mutable) in shared_objects {
            builder
                .obj(CallArg::Shared(SharedObjectRef::new(*id, *init, *mutable)))
                .unwrap();
        }
        // A MoveCall to `0x2::smart_account::build_v1` is what marks the transaction
        // as a claim (matched by `calls_smart_account_build_v1`); the arguments are
        // irrelevant to version assignment.
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            Identifier::new("smart_account").unwrap(),
            Identifier::new("build_v1").unwrap(),
            vec![],
            vec![],
        );
        let tx_data = TestTransactionBuilder::new(
            sender,
            ObjectRef::new(
                ObjectId::random(),
                SequenceNumber::from_u64(gas_object_version),
                ObjectDigest::random(),
            ),
            0,
        )
        .programmable(builder.finish())
        .build();
        let tx = SenderSignedData::new(tx_data, vec![]);
        VerifiedExecutableTransaction::new_unchecked(ExecutableTransaction::new_from_data_and_sig(
            tx,
            CertificateProof::new_system(0),
        ))
    }

    /// A `build_v1` claim anchors its sender's account id at the claim's
    /// Lamport version into `shared_input_next_versions`, so that a plain
    /// transfer from that account in a later consensus commit resolves it
    /// deterministically (without an execution-lag-dependent store read).
    #[tokio::test]
    async fn test_assign_versions_anchors_claimed_account() {
        let registry = Object::shared_for_testing();
        let registry_id = registry.id();
        let registry_init_version = registry
            .owner
            .into_shared_opt()
            .expect("expected shared object");
        let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        protocol_config.set_enable_move_authentication_for_testing(true);
        protocol_config.set_enable_builtin_move_authenticators_for_testing(true);
        protocol_config.set_enable_implicit_accounts_for_testing(true);
        let authority = TestAuthorityBuilder::new()
            .with_protocol_config(protocol_config)
            .with_starting_objects(std::slice::from_ref(&registry))
            .build()
            .await;

        let gas_version = 7;
        let sender = Address::ZERO;
        let claim = generate_claim_tx(
            &[(registry_id, registry_init_version, true)],
            sender,
            gas_version,
        );

        let epoch_store = authority.epoch_store_for_testing();
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            ..
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            std::slice::from_ref(&claim).iter(),
            &BTreeMap::new(),
        )
        .unwrap();

        // The account is anchored at the claim's Lamport version, computed from the
        // claim's input object versions (the registry and the gas coin).
        let expected_account_version = SequenceNumber::lamport_increment(
            [registry_init_version, SequenceNumber::from_u64(gas_version)].into_iter(),
        )
        .unwrap();
        let account_id = ObjectId::from(sender);
        assert_eq!(
            shared_input_next_versions.get(&account_id),
            Some(&expected_account_version),
            "claim must anchor its account at the claim's Lamport version"
        );
    }
}
