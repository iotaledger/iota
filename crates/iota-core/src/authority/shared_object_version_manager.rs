// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    crypto::RandomnessRound,
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::IotaResult,
    executable_transaction::VerifiedExecutableTransaction,
    storage::{transaction_input_object_keys, transaction_receiving_object_keys, ObjectKey},
    transaction::{SenderSignedData, SharedInputObject, TransactionDataAPI, TransactionKey},
    IOTA_RANDOMNESS_STATE_OBJECT_ID,
};
use tracing::{debug, trace};

use crate::{
    authority::{epoch_start_configuration::EpochStartConfigTrait, AuthorityPerEpochStore},
    execution_cache::ExecutionCacheRead,
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
    pub async fn assign_versions_from_consensus(
        epoch_store: &AuthorityPerEpochStore,
        cache_reader: &dyn ExecutionCacheRead,
        certificates: &[VerifiedExecutableTransaction],
        randomness_round: Option<RandomnessRound>,
    ) -> IotaResult<ConsensusSharedObjVerAssignment> {
        let mut shared_input_next_versions = get_or_init_versions(
            certificates.iter().map(|cert| cert.data()),
            epoch_store,
            cache_reader,
            randomness_round.is_some(),
        )
        .await?;
        let mut assigned_versions = Vec::new();
        // We must update randomness object version first before processing any
        // transaction, so that all reads are using the next version.
        // TODO: Add a test that actually check this, i.e. if we change the order, some
        // test should fail.
        if let Some(round) = randomness_round {
            // If we're generating randomness, update the randomness state object version.
            let version = shared_input_next_versions
                .get_mut(&IOTA_RANDOMNESS_STATE_OBJECT_ID)
                .expect("randomness state object must have been added in get_or_init_versions()");
            debug!(
                "assigning shared object versions for randomness: epoch {}, round {round:?} -> version {version:?}",
                epoch_store.epoch()
            );
            assigned_versions.push((
                TransactionKey::RandomnessRound(epoch_store.epoch(), round),
                vec![(IOTA_RANDOMNESS_STATE_OBJECT_ID, *version)],
            ));
            version.increment();
        }
        for cert in certificates {
            if !cert.contains_shared_object() {
                continue;
            }
            let cert_assigned_versions =
                assign_versions_for_certificate(cert, &mut shared_input_next_versions);
            assigned_versions.push((cert.key(), cert_assigned_versions));
        }

        Ok(ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        })
    }

    pub async fn assign_versions_from_effects(
        certs_and_effects: &[(&VerifiedExecutableTransaction, &TransactionEffects)],
        epoch_store: &AuthorityPerEpochStore,
        cache_reader: &dyn ExecutionCacheRead,
    ) -> IotaResult<AssignedTxAndVersions> {
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
            false,
        )
        .await?;
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
        Ok(assigned_versions)
    }
}

async fn get_or_init_versions(
    transactions: impl Iterator<Item = &SenderSignedData>,
    epoch_store: &AuthorityPerEpochStore,
    cache_reader: &dyn ExecutionCacheRead,
    generate_randomness: bool,
) -> IotaResult<HashMap<ObjectID, SequenceNumber>> {
    let mut shared_input_objects: Vec<_> = transactions
        .flat_map(|tx| {
            tx.transaction_data()
                .shared_input_objects()
                .into_iter()
                .map(|so| so.into_id_and_version())
        })
        .collect();

    if generate_randomness {
        shared_input_objects.push((
            IOTA_RANDOMNESS_STATE_OBJECT_ID,
            epoch_store
                .epoch_start_config()
                .randomness_obj_initial_shared_version()
                .expect(
                    "randomness_obj_initial_shared_version should exist if randomness is enabled",
                ),
        ));
    }

    shared_input_objects.sort();
    shared_input_objects.dedup();

    epoch_store
        .get_or_init_next_object_versions(&shared_input_objects, cache_reader)
        .await
}

fn assign_versions_for_certificate(
    cert: &VerifiedExecutableTransaction,
    shared_input_next_versions: &mut HashMap<ObjectID, SequenceNumber>,
) -> Vec<(ObjectID, SequenceNumber)> {
    let tx_digest = cert.digest();

    // Make an iterator to update the locks of the transaction's shared objects.
    let shared_input_objects: Vec<_> = cert.shared_input_objects().collect();

    let mut input_object_keys =
        transaction_input_object_keys(cert).expect("Transaction input should have been verified");
    let mut assigned_versions = Vec::with_capacity(shared_input_objects.len());
    let mut is_mutable_input = Vec::with_capacity(shared_input_objects.len());
    // Record receiving object versions towards the shared version computation.
    let receiving_object_keys = transaction_receiving_object_keys(cert);
    input_object_keys.extend(receiving_object_keys);

    for (SharedInputObject { id, mutable, .. }, version) in shared_input_objects
        .iter()
        .map(|obj| (obj, *shared_input_next_versions.get(&obj.id()).unwrap()))
    {
        assigned_versions.push((*id, version));
        input_object_keys.push(ObjectKey(*id, version));
        is_mutable_input.push(*mutable);
    }

    let next_version = SequenceNumber::lamport_increment(input_object_keys.iter().map(|obj| obj.1));

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
            shared_input_next_versions.insert(id, version);
        });

    trace!(
        ?tx_digest,
        ?assigned_versions,
        ?next_version,
        "locking shared objects"
    );

    assigned_versions
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::{IotaAddress, ObjectID, SequenceNumber},
        crypto::RandomnessRound,
        digests::ObjectDigest,
        effects::TestEffectsBuilder,
        executable_transaction::{
            CertificateProof, ExecutableTransaction, VerifiedExecutableTransaction,
        },
        object::{Object, Owner},
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{ObjectArg, SenderSignedData, TransactionKey},
        IOTA_RANDOMNESS_STATE_OBJECT_ID,
    };
    use shared_crypto::intent::Intent;

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
            .with_starting_objects(&[shared_object.clone()])
            .build()
            .await;
        let certs = vec![
            generate_shared_obj_tx_with_gas_version(id, init_shared_version, true, 3),
            generate_shared_obj_tx_with_gas_version(id, init_shared_version, false, 5),
            generate_shared_obj_tx_with_gas_version(id, init_shared_version, true, 9),
            generate_shared_obj_tx_with_gas_version(id, init_shared_version, true, 11),
        ];
        let epoch_store = authority.epoch_store_for_testing();
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_cache_reader().as_ref(),
            &certs,
            None,
        )
        .await
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
            .randomness_obj_initial_shared_version()
            .unwrap();
        let certs = vec![
            generate_shared_obj_tx_with_gas_version(
                IOTA_RANDOMNESS_STATE_OBJECT_ID,
                randomness_obj_version,
                // This can only be false since it's not allowed to use randomness object with
                // mutable=true.
                false,
                3,
            ),
            generate_shared_obj_tx_with_gas_version(
                IOTA_RANDOMNESS_STATE_OBJECT_ID,
                randomness_obj_version,
                false,
                5,
            ),
        ];
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_cache_reader().as_ref(),
            &certs,
            Some(RandomnessRound::new(1)),
        )
        .await
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
                    TransactionKey::RandomnessRound(0, RandomnessRound::new(1)),
                    vec![(IOTA_RANDOMNESS_STATE_OBJECT_ID, randomness_obj_version),]
                ),
                (
                    certs[0].key(),
                    // It is critical that the randomness object version is updated before the
                    // assignment.
                    vec![(IOTA_RANDOMNESS_STATE_OBJECT_ID, next_randomness_obj_version)]
                ),
                (
                    certs[1].key(),
                    // It is critical that the randomness object version is updated before the
                    // assignment.
                    vec![(IOTA_RANDOMNESS_STATE_OBJECT_ID, next_randomness_obj_version)]
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
            .with_starting_objects(&[shared_object.clone()])
            .build()
            .await;
        let certs = vec![
            generate_shared_obj_tx_with_gas_version(id, init_shared_version, true, 3),
            generate_shared_obj_tx_with_gas_version(id, init_shared_version, false, 5),
            generate_shared_obj_tx_with_gas_version(id, init_shared_version, true, 9),
            generate_shared_obj_tx_with_gas_version(id, init_shared_version, true, 11),
        ];
        let effects = vec![
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
            authority.get_cache_reader().as_ref(),
        )
        .await
        .unwrap();
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

    /// Generate a transaction that uses a shared object as specified in the
    /// parameters. Also uses a gas object with specified version.
    /// The version of the gas object is used to manipulate the lamport version
    /// of this transaction.
    fn generate_shared_obj_tx_with_gas_version(
        shared_object_id: ObjectID,
        shared_object_init_version: SequenceNumber,
        shared_object_mutable: bool,
        gas_object_version: u64,
    ) -> VerifiedExecutableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .obj(ObjectArg::SharedObject {
                id: shared_object_id,
                initial_shared_version: shared_object_init_version,
                mutable: shared_object_mutable,
            })
            .unwrap();
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
        let tx = SenderSignedData::new(tx_data, Intent::iota_transaction(), vec![]);
        VerifiedExecutableTransaction::new_unchecked(ExecutableTransaction::new_from_data_and_sig(
            tx,
            CertificateProof::new_system(0),
        ))
    }
}
