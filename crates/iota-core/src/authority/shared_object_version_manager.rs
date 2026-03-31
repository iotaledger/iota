// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, HashMap, HashSet};

use iota_types::{
    IOTA_RANDOMNESS_STATE_OBJECT_ID,
    base_types::{ObjectID, SequenceNumber, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::IotaResult,
    executable_transaction::VerifiedExecutableTransaction,
    storage::{
        ObjectKey, transaction_non_shared_input_object_keys, transaction_receiving_object_keys,
    },
    transaction::{SenderSignedData, SharedInputObject, TransactionKey},
};
use tracing::trace;

use crate::{
    authority::{
        AuthorityPerEpochStore, authority_per_epoch_store::CancelConsensusCertificateReason,
    },
    execution_cache::ObjectCacheRead,
};

pub struct SharedObjVerManager {}

pub type AssignedTxAndVersions = Vec<(TransactionKey, Vec<(ObjectID, SequenceNumber)>)>;

#[must_use]
#[derive(Default)]
pub struct ConsensusSharedObjVerAssignment {
    pub shared_input_next_versions: HashMap<ObjectID, SequenceNumber>,
    pub assigned_versions: AssignedTxAndVersions,
}

impl SharedObjVerManager {
    pub fn assign_versions_from_consensus<'a>(
        epoch_store: &AuthorityPerEpochStore,
        cache_reader: &dyn ObjectCacheRead,
        certificates: impl Iterator<Item = &'a VerifiedExecutableTransaction> + Clone,
        cancelled_txns: &BTreeMap<TransactionDigest, CancelConsensusCertificateReason>,
    ) -> IotaResult<ConsensusSharedObjVerAssignment> {
        let mut shared_input_next_versions = get_or_init_versions(
            certificates.clone().map(|cert| cert.data()),
            epoch_store,
            cache_reader,
        )?;
        let mut assigned_versions = Vec::new();
        for cert in certificates {
            if !cert.contains_shared_object() {
                continue;
            }
            let cert_assigned_versions = Self::assign_versions_for_certificate(
                cert,
                &mut shared_input_next_versions,
                cancelled_txns,
                epoch_store
                    .protocol_config()
                    .congestion_control_gas_price_feedback_mechanism(),
            );
            assigned_versions.push((cert.key(), cert_assigned_versions));
        }

        Ok(ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        })
    }

    pub fn assign_versions_from_effects(
        certs_and_effects: &[(&VerifiedExecutableTransaction, &TransactionEffects)],
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
            certs_and_effects.iter().map(|(cert, _)| cert.data()),
            epoch_store,
            cache_reader,
        );

        let mut assigned_versions = Vec::new();
        for (cert, effects) in certs_and_effects {
            let cert_assigned_versions: Vec<_> = effects
                .input_shared_objects()
                .into_iter()
                .map(|iso| iso.id_and_version())
                .collect();
            let tx_key = cert.key();
            trace!(
                ?tx_key,
                ?cert_assigned_versions,
                "locking shared objects from effects"
            );
            assigned_versions.push((tx_key, cert_assigned_versions));
        }
        assigned_versions
    }

    pub fn assign_versions_for_certificate(
        cert: &VerifiedExecutableTransaction,
        shared_input_next_versions: &mut HashMap<ObjectID, SequenceNumber>,
        cancelled_txns: &BTreeMap<TransactionDigest, CancelConsensusCertificateReason>,
        enable_gas_price_feedback: bool,
    ) -> Vec<(ObjectID, SequenceNumber)> {
        let tx_digest = cert.digest();

        // Check if the transaction is cancelled due to congestion.
        let cancellation_info = cancelled_txns.get(tx_digest);
        let congested_objects_info: Option<HashSet<_>> =
            if let Some(CancelConsensusCertificateReason::CongestionOnObjects {
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
        let shared_input_objects = cert.shared_input_objects();

        let mut input_object_keys = transaction_non_shared_input_object_keys(cert)
            .expect("Transaction input should have been verified");
        let mut assigned_versions = Vec::with_capacity(shared_input_objects.len());
        let mut is_mutable_input = Vec::with_capacity(shared_input_objects.len());
        // Record receiving object versions towards the shared version computation.
        let receiving_object_keys = transaction_receiving_object_keys(cert);
        input_object_keys.extend(receiving_object_keys);

        if txn_cancelled {
            // For cancelled transaction due to congestion, assign special versions to all
            // shared objects. Note that new lamport version does not depend on
            // any shared objects.
            for SharedInputObject { id, .. } in shared_input_objects.iter() {
                let assigned_version = match cancellation_info {
                    Some(CancelConsensusCertificateReason::CongestionOnObjects {
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
                    Some(CancelConsensusCertificateReason::DkgFailed) => {
                        if id == &IOTA_RANDOMNESS_STATE_OBJECT_ID {
                            SequenceNumber::RANDOMNESS_UNAVAILABLE
                        } else {
                            SequenceNumber::CANCELLED_READ
                        }
                    }
                    None => unreachable!("cancelled transaction should have cancellation info"),
                };
                assigned_versions.push((*id, assigned_version));
                is_mutable_input.push(false);
            }
        } else {
            for (SharedInputObject { id, mutable, .. }, assigned_version) in shared_input_objects
                .iter()
                .map(|obj| (obj, *shared_input_next_versions.get(&obj.id()).unwrap()))
            {
                assigned_versions.push((*id, assigned_version));
                input_object_keys.push(ObjectKey(*id, assigned_version));
                is_mutable_input.push(*mutable);
            }
        }

        let next_version =
            SequenceNumber::lamport_increment(input_object_keys.iter().map(|obj| obj.1));
        assert!(
            next_version.is_valid(),
            "Assigned version must be valid. Got {next_version:?}"
        );

        if !txn_cancelled {
            // Update the next version for the shared objects.
            assigned_versions
                .iter()
                .zip(is_mutable_input)
                .filter_map(|((id, _), mutable)| {
                    if mutable {
                        Some((*id, next_version))
                    } else {
                        None
                    }
                })
                .for_each(|(id, version)| {
                    assert!(
                        version.is_valid(),
                        "Assigned version must be a valid version."
                    );
                    shared_input_next_versions
                        .insert(id, version)
                        .expect("Object must exist in shared_input_next_versions.");
                });
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
) -> IotaResult<HashMap<ObjectID, SequenceNumber>> {
    let mut shared_input_objects: Vec<_> = transactions
        .flat_map(|tx| {
            tx.shared_input_objects()
                .into_iter()
                .map(|so| so.into_id_and_version())
        })
        .collect();

    shared_input_objects.sort();
    shared_input_objects.dedup();

    epoch_store.get_or_init_next_object_versions(&shared_input_objects, cache_reader)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        IOTA_RANDOMNESS_STATE_OBJECT_ID,
        base_types::{IotaAddress, ObjectID, SequenceNumber},
        crypto::RandomnessRound,
        digests::ObjectDigest,
        effects::TestEffectsBuilder,
        executable_transaction::{
            CertificateProof, ExecutableTransaction, VerifiedExecutableTransaction,
        },
        object::{Object, Owner},
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{ObjectArg, SenderSignedData, VerifiedTransaction},
    };

    use super::*;
    use crate::authority::{
        epoch_start_configuration::EpochStartConfigTrait,
        shared_object_version_manager::{ConsensusSharedObjVerAssignment, SharedObjVerManager},
        test_authority_builder::TestAuthorityBuilder,
    };

    #[tokio::test]
    async fn test_assign_versions_from_consensus_basic() {
        let shared_object = Object::shared_for_testing();
        let id = shared_object.id();
        let init_shared_version = match shared_object.owner {
            Owner::Shared {
                initial_shared_version,
                ..
            } => initial_shared_version,
            _ => panic!("expected shared object"),
        };
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(std::slice::from_ref(&shared_object))
            .build()
            .await;
        let certs = [
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
            certs.iter(),
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
        // will use the same version number. In the following case, certs[2] has
        // the same assignment as certs[1] for this reason.
        assert_eq!(
            assigned_versions,
            vec![
                (certs[0].key(), vec![(id, init_shared_version),]),
                (certs[1].key(), vec![(id, SequenceNumber::from_u64(4)),]),
                (certs[2].key(), vec![(id, SequenceNumber::from_u64(4)),]),
                (certs[3].key(), vec![(id, SequenceNumber::from_u64(10)),]),
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
        let certs = [
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
                    IOTA_RANDOMNESS_STATE_OBJECT_ID,
                    randomness_obj_version,
                    // This can only be false since it's not allowed to use randomness object with
                    // mutable=true.
                    false,
                )],
                3,
            ),
            generate_shared_objs_tx_with_gas_version(
                &[(
                    IOTA_RANDOMNESS_STATE_OBJECT_ID,
                    randomness_obj_version,
                    false,
                )],
                5,
            ),
        ];
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            certs.iter(),
            &BTreeMap::new(),
        )
        .unwrap();
        // Check that the randomness object's next version is initialized.
        assert_eq!(
            epoch_store
                .get_next_object_version(&IOTA_RANDOMNESS_STATE_OBJECT_ID)
                .unwrap(),
            randomness_obj_version
        );
        let next_randomness_obj_version = randomness_obj_version.next();
        assert_eq!(
            shared_input_next_versions,
            // Randomness object's version is only incremented by 1 regardless of lamport version.
            HashMap::from([(IOTA_RANDOMNESS_STATE_OBJECT_ID, next_randomness_obj_version)])
        );
        assert_eq!(
            assigned_versions,
            vec![
                (
                    certs[0].key(),
                    vec![(IOTA_RANDOMNESS_STATE_OBJECT_ID, randomness_obj_version),]
                ),
                (
                    certs[1].key(),
                    // It is critical that the randomness object version is updated before the
                    // assignment.
                    vec![(IOTA_RANDOMNESS_STATE_OBJECT_ID, next_randomness_obj_version)]
                ),
                (
                    certs[2].key(),
                    // It is critical that the randomness object version is updated before the
                    // assignment.
                    vec![(IOTA_RANDOMNESS_STATE_OBJECT_ID, next_randomness_obj_version)]
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
        let init_shared_version_1 = match shared_object_1.owner {
            Owner::Shared {
                initial_shared_version,
                ..
            } => initial_shared_version,
            _ => panic!("expected shared object"),
        };
        let init_shared_version_2 = match shared_object_2.owner {
            Owner::Shared {
                initial_shared_version,
                ..
            } => initial_shared_version,
            _ => panic!("expected shared object"),
        };
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
        let certs = [
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
                    (
                        IOTA_RANDOMNESS_STATE_OBJECT_ID,
                        randomness_obj_version,
                        false,
                    ),
                    (id2, init_shared_version_2, true),
                ],
                11,
            ),
        ];
        let epoch_store = authority.epoch_store_for_testing();

        // Cancel transactions 2 and 4 due to congestion.
        let suggested_gas_price = 1_000;
        let cancelled_txns: BTreeMap<TransactionDigest, CancelConsensusCertificateReason> = [
            (
                *certs[1].digest(),
                CancelConsensusCertificateReason::CongestionOnObjects {
                    congested_objects: vec![id1],
                    suggested_gas_price: Some(suggested_gas_price),
                },
            ),
            (
                *certs[3].digest(),
                CancelConsensusCertificateReason::CongestionOnObjects {
                    congested_objects: vec![id2],
                    suggested_gas_price: Some(suggested_gas_price),
                },
            ),
            (
                *certs[4].digest(),
                CancelConsensusCertificateReason::DkgFailed,
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
            certs.iter(),
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
                (IOTA_RANDOMNESS_STATE_OBJECT_ID, SequenceNumber::from_u64(1)), // not mutable
            ])
        );

        // Check that the version assignment for each transaction is correct.
        assert_eq!(
            assigned_versions,
            vec![
                (
                    certs[0].key(),
                    vec![(id1, init_shared_version_1), (id2, init_shared_version_2)]
                ),
                (
                    certs[1].key(),
                    vec![
                        (
                            id1,
                            SequenceNumber::new_congested_with_suggested_gas_price(
                                suggested_gas_price
                            )
                        ),
                        (id2, SequenceNumber::CANCELLED_READ),
                    ]
                ),
                (certs[2].key(), vec![(id1, SequenceNumber::from_u64(4)),]),
                (
                    certs[3].key(),
                    vec![
                        (id1, SequenceNumber::CANCELLED_READ),
                        (
                            id2,
                            SequenceNumber::new_congested_with_suggested_gas_price(
                                suggested_gas_price
                            )
                        )
                    ]
                ),
                (
                    certs[4].key(),
                    vec![
                        (
                            IOTA_RANDOMNESS_STATE_OBJECT_ID,
                            SequenceNumber::RANDOMNESS_UNAVAILABLE
                        ),
                        (id2, SequenceNumber::CANCELLED_READ)
                    ]
                ),
            ]
        );
    }

    #[tokio::test]
    async fn test_assign_versions_from_effects() {
        let shared_object = Object::shared_for_testing();
        let id = shared_object.id();
        let init_shared_version = match shared_object.owner {
            Owner::Shared {
                initial_shared_version,
                ..
            } => initial_shared_version,
            _ => panic!("expected shared object"),
        };
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(std::slice::from_ref(&shared_object))
            .build()
            .await;
        let certs = [
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, true)], 3),
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, false)], 5),
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, true)], 9),
            generate_shared_objs_tx_with_gas_version(&[(id, init_shared_version, true)], 11),
        ];
        let effects = [
            TestEffectsBuilder::new(certs[0].data()).build(),
            TestEffectsBuilder::new(certs[1].data())
                .with_shared_input_versions(BTreeMap::from([(id, SequenceNumber::from_u64(4))]))
                .build(),
            TestEffectsBuilder::new(certs[2].data())
                .with_shared_input_versions(BTreeMap::from([(id, SequenceNumber::from_u64(4))]))
                .build(),
            TestEffectsBuilder::new(certs[3].data())
                .with_shared_input_versions(BTreeMap::from([(id, SequenceNumber::from_u64(10))]))
                .build(),
        ];
        let epoch_store = authority.epoch_store_for_testing();
        let assigned_versions = SharedObjVerManager::assign_versions_from_effects(
            certs
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
                (certs[0].key(), vec![(id, init_shared_version),]),
                (certs[1].key(), vec![(id, SequenceNumber::from_u64(4)),]),
                (certs[2].key(), vec![(id, SequenceNumber::from_u64(4)),]),
                (certs[3].key(), vec![(id, SequenceNumber::from_u64(10)),]),
            ]
        );
    }

    /// Generate a transaction that uses shared objects as specified in the
    /// parameters. Also uses a gas object with specified version.
    /// The version of the gas object is used to manipulate the lamport version
    /// of this transaction.
    fn generate_shared_objs_tx_with_gas_version(
        shared_objects: &[(ObjectID, SequenceNumber, bool)],
        gas_object_version: u64,
    ) -> VerifiedExecutableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        for (shared_object_id, shared_object_init_version, shared_object_mutable) in shared_objects
        {
            builder
                .obj(ObjectArg::SharedObject {
                    id: *shared_object_id,
                    initial_shared_version: *shared_object_init_version,
                    mutable: *shared_object_mutable,
                })
                .unwrap();
        }
        let tx_data = TestTransactionBuilder::new(
            IotaAddress::ZERO,
            (
                ObjectID::random(),
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
}
