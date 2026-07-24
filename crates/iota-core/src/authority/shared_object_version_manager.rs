// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, HashMap, HashSet};

use iota_sdk_types::{
    ExecutionError, ExecutionStatus, ObjectId, RandomnessRound, SharedObjectReference,
    TransactionDigest, TransactionEffects, Version, VersionAssignment,
};
use iota_types::{
    base_types::EpochId,
    effects::TransactionEffectsAPI,
    error::IotaResult,
    executable_transaction::VerifiedExecutableTransaction,
    storage::{
        ObjectKey, transaction_non_shared_input_object_keys, transaction_receiving_object_keys,
    },
    transaction::{SenderSignedTransactionAPI, TransactionAPI, TransactionKey},
};
use tracing::trace;

use crate::{
    authority::{
        AuthorityPerEpochStore, authority_per_epoch_store::CancelConsensusTransactionReason,
        epoch_start_configuration::EpochStartConfigTrait,
    },
    execution_cache::ObjectCacheRead,
};

pub struct SharedObjVerManager {}

/// The assigned version of each shared object of a single transaction.
pub type AssignedVersions = Vec<VersionAssignment>;

#[derive(Default, Debug)]
pub struct AssignedTxAndVersions(pub Vec<(TransactionKey, AssignedVersions)>);

impl AssignedTxAndVersions {
    pub fn new(assigned_versions: Vec<(TransactionKey, AssignedVersions)>) -> Self {
        Self(assigned_versions)
    }

    pub fn into_map(self) -> HashMap<TransactionKey, AssignedVersions> {
        self.0.into_iter().collect()
    }
}

/// A wrapper around things that can be scheduled for execution by the
/// assigning of shared object versions.
#[derive(Clone)]
pub enum Schedulable<T = VerifiedExecutableTransaction> {
    Transaction(T),
    RandomnessStateUpdate(EpochId, RandomnessRound),
}

impl From<VerifiedExecutableTransaction> for Schedulable<VerifiedExecutableTransaction> {
    fn from(tx: VerifiedExecutableTransaction) -> Self {
        Schedulable::Transaction(tx)
    }
}

// AsTx is like Deref, in that it allows us to use either refs or values in
// Schedulable. Deref does not work because it conflicts with the impl of Deref
// for VerifiedExecutableTransaction.
pub trait AsTx {
    fn as_tx(&self) -> &VerifiedExecutableTransaction;
}

impl AsTx for VerifiedExecutableTransaction {
    fn as_tx(&self) -> &VerifiedExecutableTransaction {
        self
    }
}

impl AsTx for &'_ VerifiedExecutableTransaction {
    fn as_tx(&self) -> &VerifiedExecutableTransaction {
        self
    }
}

impl Schedulable<&'_ VerifiedExecutableTransaction> {
    // Cannot use the blanket ToOwned trait impl because it just calls clone.
    pub fn to_owned_schedulable(&self) -> Schedulable<VerifiedExecutableTransaction> {
        match self {
            Schedulable::Transaction(tx) => Schedulable::Transaction((*tx).clone()),
            Schedulable::RandomnessStateUpdate(epoch, round) => {
                Schedulable::RandomnessStateUpdate(*epoch, *round)
            }
        }
    }
}

impl<T> Schedulable<T> {
    pub fn as_tx(&self) -> Option<&VerifiedExecutableTransaction>
    where
        T: AsTx,
    {
        match self {
            Schedulable::Transaction(tx) => Some(tx.as_tx()),
            Schedulable::RandomnessStateUpdate(_, _) => None,
        }
    }

    pub fn shared_input_objects(
        &self,
        epoch_store: &AuthorityPerEpochStore,
    ) -> Vec<SharedObjectReference>
    where
        T: AsTx,
    {
        match self {
            Schedulable::Transaction(tx) => tx.as_tx().shared_input_objects(),
            Schedulable::RandomnessStateUpdate(_, _) => vec![SharedObjectReference::new(
                ObjectId::RANDOMNESS_STATE,
                epoch_store
                    .epoch_start_config()
                    .randomness_obj_initial_shared_version(),
                true,
            )],
        }
    }

    pub fn contains_shared_object(&self) -> bool
    where
        T: AsTx,
    {
        match self {
            Schedulable::Transaction(tx) => tx.as_tx().contains_shared_object(),
            Schedulable::RandomnessStateUpdate(_, _) => true,
        }
    }

    pub fn non_shared_input_object_keys(&self) -> Vec<ObjectKey>
    where
        T: AsTx,
    {
        match self {
            Schedulable::Transaction(tx) => transaction_non_shared_input_object_keys(tx.as_tx())
                .expect("Transaction input should have been verified"),
            Schedulable::RandomnessStateUpdate(_, _) => vec![],
        }
    }

    pub fn receiving_object_keys(&self) -> Vec<ObjectKey>
    where
        T: AsTx,
    {
        match self {
            Schedulable::Transaction(tx) => transaction_receiving_object_keys(tx.as_tx()),
            Schedulable::RandomnessStateUpdate(_, _) => vec![],
        }
    }

    pub fn key(&self) -> TransactionKey
    where
        T: AsTx,
    {
        match self {
            Schedulable::Transaction(tx) => tx.as_tx().key(),
            Schedulable::RandomnessStateUpdate(epoch, round) => {
                TransactionKey::RandomnessRound(*epoch, *round)
            }
        }
    }
}

#[must_use]
#[derive(Default)]
pub struct ConsensusSharedObjVerAssignment {
    pub shared_input_next_versions: HashMap<ObjectId, Version>,
    pub assigned_versions: AssignedTxAndVersions,
}

impl SharedObjVerManager {
    pub fn assign_versions_from_consensus<'a, T>(
        epoch_store: &AuthorityPerEpochStore,
        cache_reader: &dyn ObjectCacheRead,
        assignables: impl Iterator<Item = &'a Schedulable<T>> + Clone,
        cancelled_txns: &BTreeMap<TransactionDigest, CancelConsensusTransactionReason>,
    ) -> IotaResult<ConsensusSharedObjVerAssignment>
    where
        T: AsTx + 'a,
    {
        let mut shared_input_next_versions = get_or_init_versions(
            assignables
                .clone()
                .flat_map(|a| a.shared_input_objects(epoch_store)),
            epoch_store,
            cache_reader,
        )?;
        let mut assigned_versions = Vec::new();
        for assignable in assignables {
            if !assignable.contains_shared_object() {
                // A transaction without shared inputs has no version
                // assignments, except when it is cancelled (execution-worker
                // congestion): the cancellation version is then carried on
                // its gas object.
                let cancelled = assignable
                    .key()
                    .as_digest()
                    .is_some_and(|digest| cancelled_txns.contains_key(digest));
                if !cancelled {
                    continue;
                }
            }
            let tx_assigned_versions = Self::assign_versions_for_transaction(
                epoch_store,
                assignable,
                &mut shared_input_next_versions,
                cancelled_txns,
                epoch_store
                    .protocol_config()
                    .congestion_control_gas_price_feedback_mechanism(),
            );
            assigned_versions.push((assignable.key(), tx_assigned_versions));
        }

        Ok(ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions: AssignedTxAndVersions::new(assigned_versions),
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
            transactions_and_effects
                .iter()
                .flat_map(|(tx, _)| tx.shared_input_objects()),
            epoch_store,
            cache_reader,
        );

        let mut assigned_versions = Vec::new();
        for (transaction, effects) in transactions_and_effects {
            let mut tx_assigned_versions: Vec<_> = effects
                .input_shared_objects()
                .into_iter()
                .map(|shared| {
                    let reference = shared.object_reference();
                    VersionAssignment::new(reference.object_id, reference.version)
                })
                .collect();
            // A cancelled transaction without shared inputs carries the
            // cancellation version on its gas object, which is not part of
            // the effects' shared object entries. Reconstruct it from the
            // execution status so re-execution reproduces the cancellation.
            if !transaction.contains_shared_object() {
                if let Some(version) = gas_object_cancellation_version_from_effects(effects) {
                    let gas_object_id = transaction
                        .transaction()
                        .gas()
                        .first()
                        .expect("user transactions have at least one gas object")
                        .object_id;
                    tx_assigned_versions.push(VersionAssignment::new(gas_object_id, version));
                }
            }
            let tx_key = transaction.key();
            trace!(
                ?tx_key,
                ?tx_assigned_versions,
                "assigned shared object versions from effects"
            );
            assigned_versions.push((tx_key, tx_assigned_versions));
        }
        AssignedTxAndVersions::new(assigned_versions)
    }

    pub fn assign_versions_for_transaction(
        epoch_store: &AuthorityPerEpochStore,
        assignable: &Schedulable<impl AsTx>,
        shared_input_next_versions: &mut HashMap<ObjectId, Version>,
        cancelled_txns: &BTreeMap<TransactionDigest, CancelConsensusTransactionReason>,
        enable_gas_price_feedback: bool,
    ) -> AssignedVersions {
        let tx_key = assignable.key();

        // Check if the transaction is cancelled due to congestion.
        let cancellation_info = tx_key
            .as_digest()
            .and_then(|tx_digest| cancelled_txns.get(tx_digest));
        let congested_objects_info: Option<HashSet<_>> =
            if let Some(CancelConsensusTransactionReason::Congested {
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
        let shared_input_objects = assignable.shared_input_objects(epoch_store);

        let mut input_object_keys = assignable.non_shared_input_object_keys();
        let mut assigned_versions = Vec::with_capacity(shared_input_objects.len());
        let mut is_mutable_input = Vec::with_capacity(shared_input_objects.len());
        // Record receiving object versions towards the shared version computation.
        let receiving_object_keys = assignable.receiving_object_keys();
        input_object_keys.extend(receiving_object_keys);

        if txn_cancelled {
            // A cancelled transaction gets cancellation versions instead of
            // real ones: one per shared input, or one on the gas object when
            // it has no shared inputs (below). Note that the new lamport
            // version does not depend on any of these assignments.
            for SharedObjectReference { object_id: id, .. } in shared_input_objects.iter() {
                let assigned_version = match cancellation_info {
                    Some(CancelConsensusTransactionReason::Congested {
                        congested_objects: _,
                        suggested_gas_price,
                    }) => {
                        if congested_objects_info
                            .as_ref()
                            .is_some_and(|info| info.contains(id))
                        {
                            if enable_gas_price_feedback {
                                Version::new_congested_with_suggested_gas_price(
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
                                Version::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK
                            }
                        } else {
                            Version::CANCELED_READ
                        }
                    }
                    Some(CancelConsensusTransactionReason::DkgFailed) => {
                        if id == &ObjectId::RANDOMNESS_STATE {
                            Version::RANDOMNESS_UNAVAILABLE
                        } else {
                            Version::CANCELED_READ
                        }
                    }
                    None => unreachable!("cancelled transaction should have cancellation info"),
                };
                assigned_versions.push(VersionAssignment::new(*id, assigned_version));
                is_mutable_input.push(false);
            }

            // A cancelled transaction without shared inputs carries the
            // cancellation version on its gas object instead. The gas object
            // is still read and charged normally during the cancelled
            // execution; this assignment only transports the cancellation.
            if shared_input_objects.is_empty() {
                let suggested_gas_price = match cancellation_info {
                    Some(CancelConsensusTransactionReason::Congested {
                        suggested_gas_price,
                        ..
                    }) => suggested_gas_price,
                    Some(CancelConsensusTransactionReason::DkgFailed) => unreachable!(
                        "only congestion can cancel a transaction without shared inputs"
                    ),
                    None => unreachable!("cancelled transaction should have cancellation info"),
                };
                let assigned_version =
                    Version::new_congested_with_suggested_gas_price(suggested_gas_price.expect(
                        "execution-worker congestion control requires the gas price feedback \
                            mechanism, so a suggested gas price must be present",
                    ))
                    .unwrap();
                let gas_object_id = assignable
                    .as_tx()
                    .expect("only a transaction can be cancelled without shared inputs")
                    .transaction()
                    .gas()
                    .first()
                    .expect("user transactions have at least one gas object")
                    .object_id;
                assigned_versions.push(VersionAssignment::new(gas_object_id, assigned_version));
            }
        } else {
            for (
                SharedObjectReference {
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
        }

        let next_version =
            Version::lamport_increment(input_object_keys.iter().map(|obj| obj.1)).unwrap();
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

        trace!(
            ?tx_key,
            ?assigned_versions,
            ?next_version,
            ?txn_cancelled,
            "locking shared objects"
        );

        assigned_versions
    }
}

/// Returns the cancellation version to assign to the gas object of a
/// cancelled transaction without shared inputs, reconstructed from the
/// effects' execution status. Returns `None` when the effects are not an
/// execution-worker congestion cancellation, which is the only cancellation a
/// transaction without shared inputs can carry on its gas object.
pub(crate) fn gas_object_cancellation_version_from_effects(
    effects: &TransactionEffects,
) -> Option<Version> {
    let ExecutionStatus::Failure { error, .. } = effects.status() else {
        return None;
    };
    match error {
        ExecutionError::ExecutionCanceledDueToExecutionWorkerCongestion {
            suggested_gas_price,
        } => Some(
            Version::new_congested_with_suggested_gas_price(*suggested_gas_price)
                .expect("suggested gas price came from an assigned congestion version"),
        ),
        _ => None,
    }
}

fn get_or_init_versions(
    shared_input_objects: impl Iterator<Item = SharedObjectReference>,
    epoch_store: &AuthorityPerEpochStore,
    cache_reader: &dyn ObjectCacheRead,
) -> IotaResult<HashMap<ObjectId, Version>> {
    let mut shared_input_objects: Vec<_> = shared_input_objects
        .map(|so| (so.object_id, so.initial_shared_version))
        .collect();

    shared_input_objects.sort();
    shared_input_objects.dedup();

    epoch_store.get_or_init_next_object_versions(&shared_input_objects, cache_reader)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use iota_sdk_types::{
        Address, ObjectDigest, ObjectReference, RandomnessRound, SenderSignedTransaction,
    };
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        effects::TestEffectsBuilder,
        executable_transaction::{
            CertificateProof, ExecutableTransaction, VerifiedExecutableTransaction,
        },
        object::Object,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{CallArg, VerifiedTransaction},
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
            .into_opt_shared()
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
        let assignables = transactions
            .iter()
            .map(Schedulable::Transaction)
            .collect::<Vec<_>>();
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            assignables.iter(),
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
            HashMap::from([(id, Version::from_u64(12))])
        );
        // Check that the version assignment for each transaction is correct.
        // For a transaction that uses the shared object with mutable=false, it won't
        // update the version using lamport version, hence the next transaction
        // will use the same version number. In the following case, transactions[2] has
        // the same assignment as transactions[1] for this reason.
        assert_eq!(
            assigned_versions.0,
            vec![
                (
                    transactions[0].key(),
                    vec![VersionAssignment::new(id, init_shared_version)]
                ),
                (
                    transactions[1].key(),
                    vec![VersionAssignment::new(id, Version::from_u64(4))]
                ),
                (
                    transactions[2].key(),
                    vec![VersionAssignment::new(id, Version::from_u64(4))]
                ),
                (
                    transactions[3].key(),
                    vec![VersionAssignment::new(id, Version::from_u64(10))]
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
        let assignables = transactions
            .iter()
            .map(Schedulable::Transaction)
            .collect::<Vec<_>>();
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            assignables.iter(),
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
            assigned_versions.0,
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

    /// The virtual `Schedulable::RandomnessStateUpdate` (no transaction exists
    /// yet) must be assigned the current randomness object version under its
    /// `TransactionKey::RandomnessRound`, and — because its shared input is
    /// mutable — bump the version that subsequent randomness-using
    /// transactions are assigned.
    #[tokio::test]
    async fn test_assign_versions_from_consensus_with_virtual_randomness_schedulable() {
        let authority = TestAuthorityBuilder::new().build().await;
        let epoch_store = authority.epoch_store_for_testing();
        let randomness_obj_version = epoch_store
            .epoch_start_config()
            .randomness_obj_initial_shared_version();
        let round = RandomnessRound::new(1);
        let transactions = [
            generate_shared_objs_tx_with_gas_version(
                &[(ObjectId::RANDOMNESS_STATE, randomness_obj_version, false)],
                3,
            ),
            generate_shared_objs_tx_with_gas_version(
                &[(ObjectId::RANDOMNESS_STATE, randomness_obj_version, false)],
                5,
            ),
        ];
        let mut assignables: Vec<Schedulable<&VerifiedExecutableTransaction>> =
            vec![Schedulable::RandomnessStateUpdate(
                epoch_store.epoch(),
                round,
            )];
        assignables.extend(transactions.iter().map(Schedulable::Transaction));

        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            assignables.iter(),
            &BTreeMap::new(),
        )
        .unwrap();

        let next_randomness_obj_version = randomness_obj_version.next().unwrap();
        assert_eq!(
            shared_input_next_versions,
            HashMap::from([(ObjectId::RANDOMNESS_STATE, next_randomness_obj_version)])
        );
        assert_eq!(
            assigned_versions.0,
            vec![
                (
                    TransactionKey::RandomnessRound(epoch_store.epoch(), round),
                    vec![VersionAssignment::new(
                        ObjectId::RANDOMNESS_STATE,
                        randomness_obj_version
                    )]
                ),
                (
                    transactions[0].key(),
                    vec![VersionAssignment::new(
                        ObjectId::RANDOMNESS_STATE,
                        next_randomness_obj_version
                    )]
                ),
                (
                    transactions[1].key(),
                    vec![VersionAssignment::new(
                        ObjectId::RANDOMNESS_STATE,
                        next_randomness_obj_version
                    )]
                ),
            ]
        );
    }

    /// Cancellation is looked up by digest, but a randomness state update
    /// transaction is keyed by `TransactionKey::RandomnessRound`, whose
    /// `as_digest()` is `None` — so an entry in `cancelled_txns` holding the
    /// update's REAL digest must be ignored: the update is assigned the
    /// current randomness object version (not `RANDOMNESS_UNAVAILABLE`) and
    /// still bumps it for subsequent readers. A refactor that reverts to a raw
    /// digest lookup would start silently cancelling randomness state updates.
    #[tokio::test]
    async fn test_assign_versions_ignores_cancellation_for_randomness_round_keyed_transaction() {
        let authority = TestAuthorityBuilder::new().build().await;
        let epoch_store = authority.epoch_store_for_testing();
        let randomness_obj_version = epoch_store
            .epoch_start_config()
            .randomness_obj_initial_shared_version();
        let round = RandomnessRound::new(1);
        let randomness_state_update = VerifiedExecutableTransaction::new_system(
            VerifiedTransaction::new_randomness_state_update(
                epoch_store.epoch(),
                round,
                vec![],
                randomness_obj_version,
            ),
            epoch_store.epoch(),
        );
        let reader = generate_shared_objs_tx_with_gas_version(
            &[(ObjectId::RANDOMNESS_STATE, randomness_obj_version, false)],
            3,
        );
        let cancelled_txns: BTreeMap<TransactionDigest, CancelConsensusTransactionReason> = [(
            *randomness_state_update.digest(),
            CancelConsensusTransactionReason::DkgFailed,
        )]
        .into_iter()
        .collect();
        let assignables = [
            Schedulable::Transaction(&randomness_state_update),
            Schedulable::Transaction(&reader),
        ];

        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            assignables.iter(),
            &cancelled_txns,
        )
        .unwrap();

        let next_randomness_obj_version = randomness_obj_version.next().unwrap();
        assert_eq!(
            shared_input_next_versions,
            HashMap::from([(ObjectId::RANDOMNESS_STATE, next_randomness_obj_version)])
        );
        assert_eq!(
            assigned_versions.0,
            vec![
                (
                    TransactionKey::RandomnessRound(epoch_store.epoch(), round),
                    vec![VersionAssignment::new(
                        ObjectId::RANDOMNESS_STATE,
                        randomness_obj_version
                    )]
                ),
                (
                    reader.key(),
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
            .into_opt_shared()
            .expect("expected shared object");
        let init_shared_version_2 = shared_object_2
            .owner
            .into_opt_shared()
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
                CancelConsensusTransactionReason::Congested {
                    congested_objects: vec![id1],
                    suggested_gas_price: Some(suggested_gas_price),
                },
            ),
            (
                *transactions[3].digest(),
                CancelConsensusTransactionReason::Congested {
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
        let assignables = transactions
            .iter()
            .map(Schedulable::Transaction)
            .collect::<Vec<_>>();
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            assignables.iter(),
            &cancelled_txns,
        )
        .unwrap();

        // Check that the final version of the shared object is the lamport version of
        // the last transaction.
        assert_eq!(
            shared_input_next_versions,
            HashMap::from([
                (id1, Version::from_u64(5)),                        // determined by tx3
                (id2, Version::from_u64(4)),                        // determined by tx1
                (ObjectId::RANDOMNESS_STATE, Version::from_u64(1)), // not mutable
            ])
        );

        // Check that the version assignment for each transaction is correct.
        assert_eq!(
            assigned_versions.0,
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
                            Version::new_congested_with_suggested_gas_price(suggested_gas_price)
                                .unwrap(),
                        ),
                        VersionAssignment::new(id2, Version::CANCELED_READ),
                    ]
                ),
                (
                    transactions[2].key(),
                    vec![VersionAssignment::new(id1, Version::from_u64(4))]
                ),
                (
                    transactions[3].key(),
                    vec![
                        VersionAssignment::new(id1, Version::CANCELED_READ),
                        VersionAssignment::new(
                            id2,
                            Version::new_congested_with_suggested_gas_price(suggested_gas_price)
                                .unwrap(),
                        )
                    ]
                ),
                (
                    transactions[4].key(),
                    vec![
                        VersionAssignment::new(
                            ObjectId::RANDOMNESS_STATE,
                            Version::RANDOMNESS_UNAVAILABLE
                        ),
                        VersionAssignment::new(id2, Version::CANCELED_READ)
                    ]
                ),
            ]
        );
    }

    /// A virtual `Schedulable::RandomnessStateUpdate` and a
    /// congestion-cancelled transaction in the same batch — the shape of a
    /// real commit — must not disturb each other's version accounting: the
    /// virtual schedulable takes the current randomness object version and
    /// bumps it once; the cancelled transaction gets its special versions
    /// without bumping anything (neither the congested object nor the
    /// randomness object it reads); a trailing reader sees the bumped
    /// randomness version. The existing tests cover each half only in
    /// isolation.
    #[tokio::test]
    async fn test_assign_versions_from_consensus_with_virtual_schedulable_and_cancellation() {
        let shared_object = Object::shared_for_testing();
        let id = shared_object.id();
        let init_shared_version = shared_object
            .owner
            .into_opt_shared()
            .expect("expected shared object");
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(std::slice::from_ref(&shared_object))
            .build()
            .await;
        let epoch_store = authority.epoch_store_for_testing();
        let randomness_obj_version = epoch_store
            .epoch_start_config()
            .randomness_obj_initial_shared_version();
        let round = RandomnessRound::new(1);

        let cancelled_tx = generate_shared_objs_tx_with_gas_version(
            &[
                (id, init_shared_version, true),
                (ObjectId::RANDOMNESS_STATE, randomness_obj_version, false),
            ],
            5,
        );
        let reader = generate_shared_objs_tx_with_gas_version(
            &[(ObjectId::RANDOMNESS_STATE, randomness_obj_version, false)],
            3,
        );
        let suggested_gas_price = 1_000;
        let cancelled_txns: BTreeMap<TransactionDigest, CancelConsensusTransactionReason> = [(
            *cancelled_tx.digest(),
            CancelConsensusTransactionReason::CongestionOnObjects {
                congested_objects: vec![id],
                suggested_gas_price: Some(suggested_gas_price),
            },
        )]
        .into_iter()
        .collect();

        let mut assignables: Vec<Schedulable<&VerifiedExecutableTransaction>> =
            vec![Schedulable::RandomnessStateUpdate(
                epoch_store.epoch(),
                round,
            )];
        assignables.push(Schedulable::Transaction(&cancelled_tx));
        assignables.push(Schedulable::Transaction(&reader));

        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            assignables.iter(),
            &cancelled_txns,
        )
        .unwrap();

        let next_randomness_obj_version = randomness_obj_version.next().unwrap();
        assert_eq!(
            shared_input_next_versions,
            HashMap::from([
                // The cancelled transaction must not have bumped it.
                (id, init_shared_version),
                (ObjectId::RANDOMNESS_STATE, next_randomness_obj_version),
            ])
        );
        assert_eq!(
            assigned_versions.0,
            vec![
                (
                    TransactionKey::RandomnessRound(epoch_store.epoch(), round),
                    vec![VersionAssignment::new(
                        ObjectId::RANDOMNESS_STATE,
                        randomness_obj_version
                    )]
                ),
                (
                    cancelled_tx.key(),
                    vec![
                        VersionAssignment::new(
                            id,
                            Version::new_congested_with_suggested_gas_price(suggested_gas_price)
                                .unwrap(),
                        ),
                        VersionAssignment::new(ObjectId::RANDOMNESS_STATE, Version::CANCELED_READ),
                    ]
                ),
                (
                    reader.key(),
                    vec![VersionAssignment::new(
                        ObjectId::RANDOMNESS_STATE,
                        next_randomness_obj_version
                    )]
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
            .into_opt_shared()
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
                .with_shared_input_versions(BTreeMap::from([(id, Version::from_u64(4))]))
                .build(),
            TestEffectsBuilder::new(transactions[2].data())
                .with_shared_input_versions(BTreeMap::from([(id, Version::from_u64(4))]))
                .build(),
            TestEffectsBuilder::new(transactions[3].data())
                .with_shared_input_versions(BTreeMap::from([(id, Version::from_u64(10))]))
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
            assigned_versions.0,
            vec![
                (
                    transactions[0].key(),
                    vec![VersionAssignment::new(id, init_shared_version),]
                ),
                (
                    transactions[1].key(),
                    vec![VersionAssignment::new(id, Version::from_u64(4)),]
                ),
                (
                    transactions[2].key(),
                    vec![VersionAssignment::new(id, Version::from_u64(4)),]
                ),
                (
                    transactions[3].key(),
                    vec![VersionAssignment::new(id, Version::from_u64(10)),]
                ),
            ]
        );
    }

    // A transaction without shared inputs that is cancelled for congestion
    // carries its cancellation version on the gas object. All three assignment
    // paths must agree on that assignment: consensus processing (the
    // assigned-versions table), the per-transaction assignment used for the
    // consensus commit prologue, and the reconstruction from the effects'
    // execution status used by checkpoint replay.
    #[tokio::test]
    async fn test_assign_versions_for_cancelled_tx_without_shared_inputs() {
        let authority = TestAuthorityBuilder::new().build().await;
        let epoch_store = authority.epoch_store_for_testing();
        assert!(
            epoch_store
                .protocol_config()
                .congestion_control_gas_price_feedback_mechanism()
        );

        let transaction = generate_shared_objs_tx_with_gas_version(&[], 3);
        let gas_object_id = transaction.transaction().gas()[0].object_id;
        let suggested_gas_price = 1_000;
        let cancelled_txns: BTreeMap<TransactionDigest, CancelConsensusTransactionReason> = [(
            *transaction.digest(),
            CancelConsensusTransactionReason::Congested {
                congested_objects: vec![],
                suggested_gas_price: Some(suggested_gas_price),
            },
        )]
        .into_iter()
        .collect();

        let expected_assignment = vec![VersionAssignment::new(
            gas_object_id,
            Version::new_congested_with_suggested_gas_price(suggested_gas_price).unwrap(),
        )];

        // Consensus processing.
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            [&transaction].into_iter(),
            &cancelled_txns,
        )
        .unwrap();
        assert_eq!(
            assigned_versions,
            vec![(transaction.key(), expected_assignment.clone())]
        );
        // The gas object must not leak into the shared object version
        // tracking.
        assert!(shared_input_next_versions.is_empty());

        // The per-transaction assignment recorded in the consensus commit
        // prologue.
        let prologue_assignment = SharedObjVerManager::assign_versions_for_transaction(
            &transaction,
            &mut HashMap::new(),
            &cancelled_txns,
            true,
        );
        assert_eq!(prologue_assignment, expected_assignment);

        // Reconstruction from the effects' execution status (checkpoint
        // replay).
        let effects = TestEffectsBuilder::new(transaction.data())
            .with_status(ExecutionStatus::Failure {
                error: ExecutionError::ExecutionCanceledDueToExecutionWorkerCongestion {
                    suggested_gas_price,
                },
                command: None,
            })
            .build();
        let replay_assignment = SharedObjVerManager::assign_versions_from_effects(
            &[(&transaction, &effects)],
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
        );
        assert_eq!(
            replay_assignment,
            vec![(transaction.key(), expected_assignment)]
        );

        // A transaction that is not cancelled gets no assignment at all.
        let not_cancelled = generate_shared_objs_tx_with_gas_version(&[], 5);
        let ConsensusSharedObjVerAssignment {
            assigned_versions, ..
        } = SharedObjVerManager::assign_versions_from_consensus(
            &epoch_store,
            authority.get_object_cache_reader().as_ref(),
            [&not_cancelled].into_iter(),
            &cancelled_txns,
        )
        .unwrap();
        assert!(assigned_versions.is_empty());
    }

    /// Generate a transaction that uses shared objects as specified in the
    /// parameters. Also uses a gas object with specified version.
    /// The version of the gas object is used to manipulate the lamport version
    /// of this transaction.
    fn generate_shared_objs_tx_with_gas_version(
        shared_objects: &[(ObjectId, Version, bool)],
        gas_object_version: u64,
    ) -> VerifiedExecutableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        for (shared_object_id, shared_object_init_version, shared_object_mutable) in shared_objects
        {
            builder
                .obj(CallArg::Shared(SharedObjectReference::new(
                    *shared_object_id,
                    *shared_object_init_version,
                    *shared_object_mutable,
                )))
                .unwrap();
        }
        let tx_data = TestTransactionBuilder::new(
            Address::ZERO,
            ObjectReference::new(
                ObjectId::random(),
                Version::from_u64(gas_object_version),
                ObjectDigest::random(),
            ),
            0,
        )
        .programmable(builder.finish())
        .build();
        let tx = SenderSignedTransaction::new(tx_data, vec![]);
        VerifiedExecutableTransaction::new_unchecked(ExecutableTransaction::new_from_data_and_sig(
            tx,
            CertificateProof::new_system(0),
        ))
    }
}
