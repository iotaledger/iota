// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Post-consensus validation of transactions with a transaction dependency
//! input, under deterministic validation. Validation at commit `C` reads the
//! state as of commit `C - K`, the horizon, where `K` is the protocol config's
//! horizon distance, 4 here. A dependency's outcome is read only when it is a
//! root of the horizon or of an earlier commit.

use std::sync::Arc;

use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_types::{Address, ObjectId, ObjectReference, TransactionDigest};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    attestation::AttestedTransaction,
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    error::{IotaError, UserInputError},
    executable_transaction::VerifiedExecutableTransaction,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::TransactionEnvelope,
};
use typed_store::Map;

use crate::{
    authority::{
        AuthorityState, ExecutionEnv,
        authority_per_epoch_store::{AuthorityPerEpochStore, ExecutionIndicesWithStats},
        authority_test_utils::init_state_with_objects,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_handler::{ConsensusCommitInfo, VerifiedSequencedConsensusTransaction},
    post_consensus_tx_reorder::PostConsensusTxReorder,
    post_consensus_validation::{self, PostConsensusDecision},
};

struct Setup {
    authority: Arc<AuthorityState>,
    epoch_store: Arc<AuthorityPerEpochStore>,
    sender: Address,
    key: AccountPrivateKey,
    gas: Vec<ObjectReference>,
    /// Enables transaction dependencies for as long as the setup lives.
    _config_guard: OverrideGuard,
}

async fn setup(gas_coins: usize) -> Setup {
    let config_guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_transaction_dependencies_for_testing(true);
        config
    });
    let (sender, key): (_, AccountPrivateKey) = get_key_pair();
    let objects: Vec<Object> = (0..gas_coins)
        .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
        .collect();
    let gas = objects.iter().map(|object| object.object_ref()).collect();
    let authority = init_state_with_objects(objects).await;
    let epoch_store = authority.epoch_store_for_testing().clone();
    Setup {
        authority,
        epoch_store,
        sender,
        key,
        gas,
        _config_guard: config_guard,
    }
}

impl Setup {
    /// A transfer of `amount` paid with gas coin `gas_index`, depending on
    /// `dependency` when given. `u64::MAX` fails at execution.
    fn transfer(
        &self,
        gas_index: usize,
        amount: u64,
        dependency: Option<TransactionDigest>,
    ) -> TransactionEnvelope {
        let rgp = self.authority.reference_gas_price_for_testing().unwrap();
        self.transfer_at_gas_price(gas_index, amount, dependency, rgp)
    }

    /// Like [`Self::transfer`], at `gas_price`.
    fn transfer_at_gas_price(
        &self,
        gas_index: usize,
        amount: u64,
        dependency: Option<TransactionDigest>,
        gas_price: u64,
    ) -> TransactionEnvelope {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(Address::random(), Some(amount));
        let mut transaction =
            TestTransactionBuilder::new(self.sender, self.gas[gas_index], gas_price)
                .programmable(builder.finish());
        if let Some(dependency) = dependency {
            transaction =
                transaction.with_transaction_dependency(dependency, self.epoch_store.epoch());
        }
        transaction.build_and_sign(&self.key)
    }

    /// Executes `transaction` as a root of commit `root_commit`, as the
    /// consensus handler and the execution watcher would leave it.
    fn execute_as_root_of(&self, transaction: TransactionEnvelope, root_commit: u64) {
        let digest = *transaction.digest();
        let verified = self.epoch_store.verify_transaction(transaction).unwrap();
        let executable = VerifiedExecutableTransaction::new_from_checkpoint(
            verified,
            self.epoch_store.epoch(),
            1,
        );
        self.epoch_store
            .assign_to_a_new_commit_for_testing(&[executable.key()]);
        self.authority
            .try_execute_immediately(&executable, ExecutionEnv::new(), &self.epoch_store)
            .unwrap();
        self.epoch_store
            .tables()
            .unwrap()
            .root_commits
            .insert(&digest, &root_commit)
            .unwrap();
    }

    /// Orders `transactions` for their dependencies and validates them at
    /// commit `commit_index`, as the consensus handler does, and returns the
    /// digests kept, in the order validated, and the drops.
    async fn validate(
        &self,
        commit_index: u64,
        transactions: Vec<TransactionEnvelope>,
    ) -> (
        Vec<TransactionDigest>,
        Vec<(TransactionDigest, IotaError, PostConsensusDecision)>,
    ) {
        let mut sequenced: Vec<_> = transactions
            .into_iter()
            .map(sequenced_user_transaction)
            .collect();
        PostConsensusTxReorder::order_dependents_after_dependencies(&mut sequenced);
        let (dropped, _locks, _digests) =
            post_consensus_validation::validate_and_resolve_conflicts(
                &self.authority,
                &self.epoch_store,
                commit_index,
                &Default::default(),
                &mut sequenced,
            )
            .await
            .unwrap();
        let kept = sequenced
            .iter()
            .filter_map(|transaction| transaction.0.transaction.executable_transaction_digest())
            .collect();
        (kept, dropped)
    }

    /// Processes `transactions` as the user transactions of commit
    /// `commit_index`, as the consensus handler does, and returns the ones
    /// scheduled, in order.
    async fn process_commit(
        &self,
        commit_index: u64,
        transactions: Vec<TransactionEnvelope>,
    ) -> Vec<TransactionDigest> {
        let digests: Vec<_> = transactions
            .iter()
            .map(|transaction| *transaction.digest())
            .collect();
        let (schedulables, ..) = self
            .epoch_store
            .process_consensus_transactions_and_commit_boundary(
                transactions
                    .into_iter()
                    .map(sequenced_user_transaction)
                    .collect(),
                &ExecutionIndicesWithStats::default(),
                &Arc::new(CheckpointServiceNoop {}),
                self.authority.get_object_cache_reader().as_ref(),
                &ConsensusCommitInfo::new_for_test(commit_index, commit_index, 0, true),
                &self.authority.metrics,
                &self.authority,
            )
            .await
            .unwrap();
        schedulables
            .iter()
            .filter_map(|schedulable| schedulable.as_tx())
            .map(|transaction| *transaction.digest())
            .filter(|digest| digests.contains(digest))
            .collect()
    }
}

/// `transaction` as consensus sequences a user transaction.
fn sequenced_user_transaction(
    transaction: TransactionEnvelope,
) -> VerifiedSequencedConsensusTransaction {
    VerifiedSequencedConsensusTransaction::new_test(ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
            transaction,
            None,
        ))),
        tracking_id: Default::default(),
    })
}

fn dependency_error(error: &IotaError) -> Option<&UserInputError> {
    match error {
        IotaError::UserInput { error } => Some(error),
        _ => None,
    }
}

#[tokio::test]
async fn dependency_sequenced_later_in_the_commit_is_validated_first() {
    let setup = setup(2).await;
    let dependency = setup.transfer(0, 1_000, None);
    let dependency_digest = *dependency.digest();
    let dependent = setup.transfer(1, 1_000, Some(dependency_digest));
    let dependent_digest = *dependent.digest();

    let (kept, dropped) = setup.validate(20, vec![dependent, dependency]).await;

    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(kept, vec![dependency_digest, dependent_digest]);
}

#[tokio::test]
async fn unknown_dependency_is_not_visible_yet() {
    let setup = setup(1).await;
    let missing = TransactionDigest::random();
    let dependent = setup.transfer(0, 1_000, Some(missing));
    let dependent_digest = *dependent.digest();

    let (kept, dropped) = setup.validate(20, vec![dependent]).await;

    assert!(kept.is_empty());
    let [(digest, error, decision)] = dropped.as_slice() else {
        panic!("expected one drop: {dropped:?}");
    };
    assert_eq!(*digest, dependent_digest);
    assert_eq!(*decision, PostConsensusDecision::DroppedInputNotVisibleYet);
    assert_eq!(
        dependency_error(error),
        Some(&UserInputError::TransactionDependencyNotFound { digest: missing })
    );
}

#[tokio::test]
async fn dependency_dropped_in_the_commit_drops_the_dependent_for_good() {
    let setup = setup(2).await;
    // Both spend gas coin 0: the first keeps it, the second loses the lock.
    let winner = setup.transfer(0, 1_000, None);
    let loser = setup.transfer(0, 2_000, None);
    let loser_digest = *loser.digest();
    let dependent = setup.transfer(1, 1_000, Some(loser_digest));
    let dependent_digest = *dependent.digest();

    let (kept, dropped) = setup.validate(20, vec![winner, loser, dependent]).await;

    assert!(!kept.contains(&dependent_digest));
    let (_, error, decision) = dropped
        .iter()
        .find(|(digest, _, _)| *digest == dependent_digest)
        .expect("the dependent is dropped");
    assert_eq!(*decision, PostConsensusDecision::Dropped);
    assert_eq!(
        dependency_error(error),
        Some(&UserInputError::TransactionDependencyFailed {
            digest: loser_digest
        })
    );
}

#[tokio::test]
async fn failed_dependency_at_the_horizon_drops_the_dependent_for_good() {
    let setup = setup(2).await;
    let failing = setup.transfer(0, u64::MAX, None);
    let failing_digest = *failing.digest();
    setup.execute_as_root_of(failing, 10);
    assert!(
        !setup
            .authority
            .get_transaction_cache_reader()
            .get_executed_effects(&failing_digest)
            .unwrap()
            .status()
            .is_success()
    );

    let dependent = setup.transfer(1, 1_000, Some(failing_digest));
    let dependent_digest = *dependent.digest();
    // The horizon of commit 14 is commit 10, the dependency's commit.
    let (kept, dropped) = setup.validate(14, vec![dependent]).await;

    assert!(kept.is_empty());
    let [(digest, error, decision)] = dropped.as_slice() else {
        panic!("expected one drop: {dropped:?}");
    };
    assert_eq!(*digest, dependent_digest);
    assert_eq!(*decision, PostConsensusDecision::Dropped);
    assert_eq!(
        dependency_error(error),
        Some(&UserInputError::TransactionDependencyFailed {
            digest: failing_digest
        })
    );
}

#[tokio::test]
async fn successful_dependency_at_the_horizon_keeps_the_dependent() {
    let setup = setup(2).await;
    let dependency = setup.transfer(0, 1_000, None);
    let dependency_digest = *dependency.digest();
    setup.execute_as_root_of(dependency, 10);

    let dependent = setup.transfer(1, 1_000, Some(dependency_digest));
    let dependent_digest = *dependent.digest();
    let (kept, dropped) = setup.validate(14, vec![dependent]).await;

    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(kept, vec![dependent_digest]);
}

/// A dependent whose dependency is a root of a commit later than the horizon
/// is kept, whatever the dependency's local outcome: another validator may
/// not have executed that commit yet.
#[tokio::test]
async fn dependency_later_than_the_horizon_keeps_the_dependent() {
    let setup = setup(2).await;
    let failing = setup.transfer(0, u64::MAX, None);
    let failing_digest = *failing.digest();
    setup.execute_as_root_of(failing, 11);

    let dependent = setup.transfer(1, 1_000, Some(failing_digest));
    let dependent_digest = *dependent.digest();
    // The horizon of commit 14 is commit 10, one before the dependency's
    // commit.
    let (kept, dropped) = setup.validate(14, vec![dependent]).await;

    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(kept, vec![dependent_digest]);
}

/// A dependent sequenced ahead of its dependency in one commit, at a higher
/// gas price so that the gas-price order puts it first again, is kept and
/// scheduled after its dependency.
#[tokio::test]
async fn dependent_ahead_of_its_dependency_is_scheduled_after_it() {
    let setup = setup(2).await;
    let rgp = setup.authority.reference_gas_price_for_testing().unwrap();
    let dependency = setup.transfer(0, 1_000, None);
    let dependent = setup.transfer_at_gas_price(1, 1_000, Some(*dependency.digest()), 2 * rgp);
    let expected = vec![*dependency.digest(), *dependent.digest()];

    let scheduled = setup.process_commit(1, vec![dependent, dependency]).await;

    assert_eq!(scheduled, expected);
}
