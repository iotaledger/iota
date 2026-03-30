// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A `Simulacrum` of IOTA.
//!
//! The word simulacrum is latin for "likeness, semblance", it is also a spell
//! in D&D which creates a copy of a creature which then follows the player's
//! commands and wishes. As such this crate provides the [`Simulacrum`] type
//! which is a implementation or instantiation of a iota blockchain, one which
//! doesn't do anything unless acted upon.
//!
//! [`Simulacrum`]: crate::Simulacrum

mod epoch_state;
pub mod state_reader;
pub mod store;
pub mod transaction_executor;

use std::{
    num::NonZeroUsize,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use anyhow::{Result, anyhow};
use fastcrypto::traits::Signer;
use iota_config::{
    genesis, transaction_deny_config::TransactionDenyConfig,
    verifier_signing_config::VerifierSigningConfig,
};
use iota_protocol_config::ProtocolVersion;
use iota_storage::blob::{Blob, BlobEncoding};
use iota_swarm_config::{
    genesis_config::AccountConfig, network_config::NetworkConfig,
    network_config_builder::ConfigBuilder,
};
use iota_types::{
    base_types::{AuthorityName, IotaAddress, ObjectID, VersionNumber},
    committee::Committee,
    crypto::{AuthoritySignature, KeypairTraits},
    digests::ConsensusCommitDigest,
    effects::TransactionEffects,
    error::ExecutionError,
    gas_coin::{GasCoin, NANOS_PER_IOTA},
    inner_temporary_store::InnerTemporaryStore,
    iota_system_state::epoch_start_iota_system_state::EpochStartSystemState,
    messages_checkpoint::{
        CheckpointContents, CheckpointSequenceNumber, EndOfEpochData, VerifiedCheckpoint,
    },
    mock_checkpoint_builder::{MockCheckpointBuilder, ValidatorKeypairProvider},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::VerifyParams,
    storage::{ObjectStore, ReadStore, RestStateReader},
    transaction::{
        EndOfEpochTransactionKind, GasData, Transaction, TransactionData, TransactionKind,
        VerifiedTransaction,
    },
};
use rand::rngs::OsRng;

pub use self::store::{SimulatorStore, in_mem_store::InMemoryStore};
use self::{epoch_state::EpochState, store::in_mem_store::KeyStore};

/// A `Simulacrum` of IOTA.
///
/// This type represents a simulated instantiation of an IOTA blockchain that
/// needs to be driven manually, that is time doesn't advance and checkpoints
/// are not formed unless explicitly requested.
///
/// See [module level][mod] documentation for more details.
///
/// [mod]: index.html
pub struct Simulacrum<R = OsRng, Store: SimulatorStore = InMemoryStore> {
    // Mutable state protected by RwLock for thread-safe interior mutability
    inner: RwLock<SimulacrumInner<R, Store>>,
    // Immutable config - can be accessed directly
    deny_config: TransactionDenyConfig,
    verifier_signing_config: VerifierSigningConfig,
}

struct SimulacrumInner<R, Store: SimulatorStore> {
    rng: R,
    keystore: KeyStore,
    #[expect(unused)]
    genesis: genesis::Genesis,
    store: Store,
    checkpoint_builder: MockCheckpointBuilder,

    // Epoch specific data
    epoch_state: EpochState,
    data_ingestion_path: Option<PathBuf>,
}

impl Simulacrum {
    /// Create a new, random Simulacrum instance using an `OsRng` as the source
    /// of randomness.
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::new_with_rng(OsRng)
    }
}

impl<R> Simulacrum<R>
where
    R: rand::RngCore + rand::CryptoRng,
{
    /// Create a new Simulacrum instance using the provided `rng`.
    ///
    /// This allows you to create a fully deterministic initial chainstate when
    /// a seeded rng is used.
    ///
    /// ```
    /// use rand::{SeedableRng, rngs::StdRng};
    /// use simulacrum::Simulacrum;
    ///
    /// # fn main() {
    /// let mut rng = StdRng::seed_from_u64(1);
    /// let simulacrum = Simulacrum::new_with_rng(rng);
    /// # }
    /// ```
    pub fn new_with_rng(mut rng: R) -> Self {
        let config = ConfigBuilder::new_with_temp_dir()
            .rng(&mut rng)
            .with_chain_start_timestamp_ms(1)
            .deterministic_committee_size(NonZeroUsize::new(1).unwrap())
            .build();
        Self::new_with_network_config_in_mem(&config, rng)
    }

    pub fn new_with_protocol_version_and_accounts(
        mut rng: R,
        chain_start_timestamp_ms: u64,
        protocol_version: ProtocolVersion,
        account_configs: Vec<AccountConfig>,
    ) -> Self {
        let config = ConfigBuilder::new_with_temp_dir()
            .rng(&mut rng)
            .with_chain_start_timestamp_ms(chain_start_timestamp_ms)
            .deterministic_committee_size(NonZeroUsize::new(1).unwrap())
            .with_protocol_version(protocol_version)
            .with_accounts(account_configs)
            .build();
        Self::new_with_network_config_in_mem(&config, rng)
    }

    fn new_with_network_config_in_mem(config: &NetworkConfig, rng: R) -> Self {
        let store = InMemoryStore::new(&config.genesis);
        Self::new_with_network_config_store(config, rng, store)
    }
}

impl<R, S: store::SimulatorStore> Simulacrum<R, S> {
    pub fn new_with_network_config_store(config: &NetworkConfig, rng: R, store: S) -> Self {
        let keystore = KeyStore::from_network_config(config);
        let checkpoint_builder = MockCheckpointBuilder::new(config.genesis.checkpoint());

        let genesis = &config.genesis;
        let epoch_state = EpochState::new(genesis.iota_system_object());

        Self {
            deny_config: TransactionDenyConfig::default(),
            verifier_signing_config: VerifierSigningConfig::default(),
            inner: RwLock::new(SimulacrumInner {
                rng,
                keystore,
                genesis: genesis.clone(),
                store,
                checkpoint_builder,
                epoch_state,
                data_ingestion_path: None,
            }),
        }
    }

    /// Attempts to execute the provided Transaction.
    ///
    /// The provided Transaction undergoes the same types of checks that a
    /// Validator does prior to signing and executing in the production
    /// system. Some of these checks are as follows:
    /// - User signature is valid
    /// - Sender owns all OwnedObject inputs
    /// - etc
    ///
    /// If the above checks are successful then the transaction is immediately
    /// executed, enqueued to be included in the next checkpoint (the next
    /// time `create_checkpoint` is called) and the corresponding
    /// TransactionEffects are returned.
    pub fn execute_transaction(
        &self,
        transaction: Transaction,
    ) -> anyhow::Result<(TransactionEffects, Option<ExecutionError>)> {
        let mut inner = self.inner.write().unwrap();
        let transaction = transaction
            .try_into_verified_for_testing(inner.epoch_state.epoch(), &VerifyParams::default())?;

        let (inner_temporary_store, _, effects, execution_error_opt) =
            inner.epoch_state.execute_transaction(
                &inner.store,
                &self.deny_config,
                &self.verifier_signing_config,
                &transaction,
            )?;

        let InnerTemporaryStore {
            written, events, ..
        } = inner_temporary_store;

        inner.store.insert_executed_transaction(
            transaction.clone(),
            effects.clone(),
            events,
            written,
        );

        // Insert into checkpoint builder
        inner
            .checkpoint_builder
            .push_transaction(transaction, effects.clone());
        Ok((effects, execution_error_opt.err()))
    }

    /// Simulate a transaction without committing changes.
    /// This is useful for testing transaction behavior without modifying state.
    pub fn simulate_transaction(
        &self,
        transaction: TransactionData,
        checks: iota_types::transaction_executor::VmChecks,
    ) -> iota_types::error::IotaResult<iota_types::transaction_executor::SimulateTransactionResult>
    {
        let inner = self.inner.read().unwrap();
        inner.epoch_state.simulate_transaction(
            &inner.store,
            &self.deny_config,
            &self.verifier_signing_config,
            transaction,
            checks,
        )
    }

    /// Creates the next Checkpoint using the Transactions enqueued since the
    /// last checkpoint was created.
    pub fn create_checkpoint(&self) -> VerifiedCheckpoint {
        let (checkpoint, contents) = {
            let mut inner = self.inner.write().unwrap();
            let committee = CommitteeWithKeys::new(&inner.keystore, inner.epoch_state.committee());
            let timestamp_ms = inner.store.get_clock().timestamp_ms();
            let (checkpoint, contents, _) =
                inner.checkpoint_builder.build(&committee, timestamp_ms);
            inner.store.insert_checkpoint(checkpoint.clone());
            inner.store.insert_checkpoint_contents(contents.clone());
            (checkpoint, contents)
        };
        // Release lock before expensive data ingestion operation
        self.process_data_ingestion(checkpoint.clone(), contents)
            .unwrap();
        checkpoint
    }

    /// Advances the clock by `duration`.
    ///
    /// This creates and executes a ConsensusCommitPrologue transaction which
    /// advances the chain Clock by the provided duration.
    pub fn advance_clock(&self, duration: std::time::Duration) -> TransactionEffects {
        let mut inner = self.inner.write().unwrap();
        let epoch = inner.epoch_state.epoch();
        let round = inner.epoch_state.next_consensus_round();
        let timestamp_ms = inner.store.get_clock().timestamp_ms() + duration.as_millis() as u64;
        drop(inner);

        let consensus_commit_prologue_transaction =
            VerifiedTransaction::new_consensus_commit_prologue_v1(
                epoch,
                round,
                timestamp_ms,
                ConsensusCommitDigest::default(),
                Vec::new(),
            );

        self.execute_transaction(consensus_commit_prologue_transaction.into())
            .expect("advancing the clock cannot fail")
            .0
    }

    /// Advances the epoch.
    ///
    /// This creates and executes an EndOfEpoch transaction which advances the
    /// chain into the next epoch. Since it is required to be the final
    /// transaction in an epoch, the final checkpoint in the epoch is also
    /// created.
    ///
    /// NOTE: This function does not currently support updating the protocol
    /// version or the system packages
    pub fn advance_epoch(&self) {
        let inner = self.inner.read().unwrap();
        let current_epoch = inner.epoch_state.epoch();
        let next_epoch = current_epoch + 1;
        let next_epoch_protocol_version = inner.epoch_state.protocol_version();
        let gas_cost_summary = inner
            .checkpoint_builder
            .epoch_rolling_gas_cost_summary()
            .clone();
        let epoch_start_timestamp_ms = inner.store.get_clock().timestamp_ms();
        drop(inner);

        let next_epoch_system_package_bytes = vec![];
        let kinds = vec![EndOfEpochTransactionKind::new_change_epoch_v3(
            next_epoch,
            next_epoch_protocol_version,
            gas_cost_summary.storage_cost,
            gas_cost_summary.computation_cost,
            gas_cost_summary.computation_cost_burned,
            gas_cost_summary.storage_rebate,
            gas_cost_summary.non_refundable_storage_fee,
            epoch_start_timestamp_ms,
            next_epoch_system_package_bytes,
            vec![],
        )];

        let tx = VerifiedTransaction::new_end_of_epoch_transaction(kinds);
        self.execute_transaction(tx.into())
            .expect("advancing the epoch cannot fail");

        let (checkpoint, contents, new_epoch_state) = {
            let mut inner = self.inner.write().unwrap();
            let new_epoch_state = EpochState::new(inner.store.get_system_state());
            let end_of_epoch_data = EndOfEpochData {
                next_epoch_committee: new_epoch_state.committee().voting_rights.clone(),
                next_epoch_protocol_version,
                epoch_commitments: vec![],
                // Do not simulate supply changes for now.
                epoch_supply_change: 0,
            };
            let (checkpoint, contents, _) = {
                let committee =
                    CommitteeWithKeys::new(&inner.keystore, inner.epoch_state.committee());
                let timestamp_ms = inner.store.get_clock().timestamp_ms();
                inner.checkpoint_builder.build_end_of_epoch(
                    &committee,
                    timestamp_ms,
                    next_epoch,
                    end_of_epoch_data,
                )
            };

            inner.store.insert_checkpoint(checkpoint.clone());
            inner.store.insert_checkpoint_contents(contents.clone());
            inner
                .store
                .update_last_checkpoint_of_epoch(current_epoch, *checkpoint.sequence_number());
            (checkpoint, contents, new_epoch_state)
        };

        // Process data ingestion without holding the lock
        self.process_data_ingestion(checkpoint, contents).unwrap();

        // Finally, update the epoch state
        let mut inner = self.inner.write().unwrap();
        inner.epoch_state = new_epoch_state;
    }

    /// Execute a function with read access to the store.
    ///
    /// This provides thread-safe access to the underlying store by locking it
    /// for the duration of the closure execution.
    pub fn with_store<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&S) -> T,
    {
        let inner = self.inner.read().unwrap();
        f(&inner.store)
    }

    /// Execute a function with read access to the keystore.
    ///
    /// This provides thread-safe access to the keystore by locking it
    /// for the duration of the closure execution.
    pub fn with_keystore<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&KeyStore) -> T,
    {
        let inner = self.inner.read().unwrap();
        f(&inner.keystore)
    }

    pub fn epoch_start_state(&self) -> EpochStartSystemState {
        let inner = self.inner.read().unwrap();
        inner.epoch_state.epoch_start_state()
    }

    /// Execute a function with mutable access to the internally held RNG.
    ///
    /// Provides mutable access to the RNG used to create this Simulacrum for
    /// use as a source of randomness. Using a seeded RNG to build a
    /// Simulacrum and then utilizing the stored RNG as a source of
    /// randomness can lead to a fully deterministic chain evolution.
    pub fn with_rng<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut R) -> T,
    {
        let mut inner = self.inner.write().unwrap();
        f(&mut inner.rng)
    }

    /// Return the reference gas price for the current epoch
    pub fn reference_gas_price(&self) -> u64 {
        self.inner.read().unwrap().epoch_state.reference_gas_price()
    }

    /// Request that `amount` Nanos be sent to `address` from a faucet account.
    ///
    /// ```
    /// use iota_types::{base_types::IotaAddress, gas_coin::NANOS_PER_IOTA};
    /// use simulacrum::Simulacrum;
    ///
    /// # fn main() {
    /// let mut simulacrum = Simulacrum::new();
    /// let address = simulacrum.with_rng(|rng| IotaAddress::generate(rng));
    /// simulacrum.request_gas(address, NANOS_PER_IOTA).unwrap();
    ///
    /// // `account` now has a Coin<IOTA> object with single IOTA in it.
    /// // ...
    /// # }
    /// ```
    pub fn request_gas(&self, address: IotaAddress, amount: u64) -> Result<TransactionEffects> {
        // For right now we'll just use the first account as the `faucet` account. We
        // may want to explicitly cordon off the faucet account from the rest of
        // the accounts though.
        let (sender, key) = self.with_keystore(|keystore| -> Result<(IotaAddress, _)> {
            let (s, k) = keystore
                .accounts()
                .next()
                .ok_or_else(|| anyhow!("no accounts available in keystore"))?;
            Ok((*s, k.copy()))
        })?;

        let object = self
            .with_store(|store| {
                store.owned_objects(sender).find(|object| {
                    object.is_gas_coin() && object.get_coin_value_unsafe() > amount + NANOS_PER_IOTA
                })
            })
            .ok_or_else(|| {
                anyhow!("unable to find a coin with enough to satisfy request for {amount} Nanos")
            })?;

        let gas_data = iota_types::transaction::GasData {
            payment: vec![object.compute_object_reference()],
            owner: sender,
            price: self.reference_gas_price(),
            budget: NANOS_PER_IOTA,
        };

        let pt = {
            let mut builder =
                iota_types::programmable_transaction_builder::ProgrammableTransactionBuilder::new();
            builder.transfer_iota(address, Some(amount));
            builder.finish()
        };

        let kind = iota_types::transaction::TransactionKind::ProgrammableTransaction(pt);
        let tx_data =
            iota_types::transaction::TransactionData::new_with_gas_data(kind, sender, gas_data);
        let tx = Transaction::from_data_and_signer(tx_data, vec![&key]);

        self.execute_transaction(tx).map(|x| x.0)
    }

    pub fn set_data_ingestion_path(&self, data_ingestion_path: PathBuf) {
        let checkpoint = {
            let mut inner = self.inner.write().unwrap();
            inner.data_ingestion_path = Some(data_ingestion_path);
            let checkpoint = inner.store.get_checkpoint_by_sequence_number(0).unwrap();
            let contents = inner
                .store
                .get_checkpoint_contents_by_digest(&checkpoint.content_digest);
            (checkpoint, contents)
        };
        // Release lock before expensive data ingestion operation
        if let (checkpoint, Some(contents)) = checkpoint {
            self.process_data_ingestion(checkpoint, contents).unwrap();
        }
    }

    /// Overrides the next checkpoint number indirectly by setting the previous
    /// checkpoint's number to checkpoint_number - 1. This ensures the next
    /// generated checkpoint has the exact sequence number provided. This
    /// can be useful to generate checkpoints with specific sequence
    /// numbers. Monotonicity of checkpoint numbers is enforced strictly.
    pub fn override_next_checkpoint_number(&self, number: CheckpointSequenceNumber) {
        let mut inner = self.inner.write().unwrap();
        let committee = CommitteeWithKeys::new(&inner.keystore, inner.epoch_state.committee());
        inner
            .checkpoint_builder
            .override_next_checkpoint_number(number, &committee);
    }

    /// Process data ingestion without holding the inner lock.
    /// This version should be used when you don't already hold the lock.
    fn process_data_ingestion(
        &self,
        checkpoint: VerifiedCheckpoint,
        checkpoint_contents: CheckpointContents,
    ) -> anyhow::Result<()> {
        let path = self.inner.read().unwrap().data_ingestion_path.clone();
        if let Some(data_path) = path {
            let file_name = format!("{}.chk", checkpoint.sequence_number);
            let checkpoint_data = self.try_get_checkpoint_data(checkpoint, checkpoint_contents)?;
            std::fs::create_dir_all(&data_path)?;
            let blob = Blob::encode(&checkpoint_data, BlobEncoding::Bcs)?;
            std::fs::write(data_path.join(file_name), blob.to_bytes())?;
        }
        Ok(())
    }
}

pub struct CommitteeWithKeys {
    keystore: KeyStore,
    committee: Committee,
}

impl CommitteeWithKeys {
    fn new(keystore: &KeyStore, committee: &Committee) -> Self {
        Self {
            keystore: keystore.clone(),
            committee: committee.clone(),
        }
    }

    pub fn keystore(&self) -> &KeyStore {
        &self.keystore
    }
}

impl ValidatorKeypairProvider for CommitteeWithKeys {
    fn get_validator_key(&self, name: &AuthorityName) -> &dyn Signer<AuthoritySignature> {
        self.keystore.validator(name).unwrap()
    }

    fn get_committee(&self) -> &Committee {
        &self.committee
    }
}

impl<T, V: store::SimulatorStore> ObjectStore for Simulacrum<T, V> {
    fn try_get_object(
        &self,
        object_id: &ObjectID,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        self.with_store(|store| store.try_get_object(object_id))
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        self.with_store(|store| store.try_get_object_by_key(object_id, version))
    }
}

impl<T, V: store::SimulatorStore> ReadStore for Simulacrum<T, V> {
    fn try_get_committee(
        &self,
        _epoch: iota_types::committee::EpochId,
    ) -> iota_types::storage::error::Result<Option<std::sync::Arc<Committee>>> {
        todo!()
    }

    fn try_get_latest_checkpoint(&self) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        Ok(self.with_store(|store| store.get_highest_checkpoint().unwrap()))
    }

    fn try_get_highest_verified_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        Ok(self.with_store(|store| store.get_highest_checkpoint().unwrap()))
    }

    fn try_get_highest_synced_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        Ok(self.with_store(|store| store.get_highest_checkpoint().unwrap()))
    }

    fn try_get_lowest_available_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<iota_types::messages_checkpoint::CheckpointSequenceNumber>
    {
        // TODO wire this up to the underlying sim store, for now this will work since
        // we never prune the sim store
        Ok(0)
    }

    fn try_get_checkpoint_by_digest(
        &self,
        digest: &iota_types::messages_checkpoint::CheckpointDigest,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        Ok(self.with_store(|store| store.get_checkpoint_by_digest(digest)))
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        sequence_number: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        Ok(self.with_store(|store| store.get_checkpoint_by_sequence_number(sequence_number)))
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        Ok(self.with_store(|store| store.get_checkpoint_contents_by_digest(digest)))
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        Ok(self.with_store(|store| {
            store
                .get_checkpoint_by_sequence_number(sequence_number)
                .and_then(|checkpoint| {
                    store.get_checkpoint_contents_by_digest(&checkpoint.content_digest)
                })
        }))
    }

    fn try_get_transaction(
        &self,
        tx_digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<Arc<VerifiedTransaction>>> {
        Ok(self.with_store(|store| store.get_transaction(tx_digest)))
    }

    fn try_get_transaction_effects(
        &self,
        tx_digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<TransactionEffects>> {
        Ok(self.with_store(|store| store.get_transaction_effects(tx_digest)))
    }

    fn try_get_events(
        &self,
        digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<iota_types::effects::TransactionEvents>> {
        Ok(self.with_store(|store| store.get_events(digest)))
    }

    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        self.with_store(|store| {
            store
                .try_get_checkpoint_by_sequence_number(sequence_number)?
                .and_then(|chk| store.get_checkpoint_contents_by_digest(&chk.content_digest))
                .map_or(Ok(None), |contents| {
                    iota_types::messages_checkpoint::FullCheckpointContents::try_from_checkpoint_contents(
                        store,
                        contents,
                    )
                })
        })
    }

    fn try_get_full_checkpoint_contents(
        &self,
        digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        self.with_store(|store| {
            store.get_checkpoint_contents_by_digest(digest)
            .map_or(Ok(None), |contents| {
                iota_types::messages_checkpoint::FullCheckpointContents::try_from_checkpoint_contents(
                    store,
                    contents,
                )
            })
        })
    }
}

impl<T: Send + Sync, V: store::SimulatorStore + Send + Sync> RestStateReader for Simulacrum<T, V> {
    fn get_lowest_available_checkpoint_objects(
        &self,
    ) -> iota_types::storage::error::Result<CheckpointSequenceNumber> {
        Ok(0)
    }

    fn get_chain_identifier(
        &self,
    ) -> iota_types::storage::error::Result<iota_types::digests::ChainIdentifier> {
        Ok(self
            .with_store(|store| store.get_checkpoint_by_sequence_number(0))
            .expect("lowest available checkpoint should exist")
            .digest()
            .to_owned()
            .into())
    }

    fn get_epoch_last_checkpoint(
        &self,
        epoch_id: iota_types::committee::EpochId,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        Ok(self.with_store(|store| {
            store
                .get_last_checkpoint_of_epoch(epoch_id)
                .and_then(|seq| store.get_checkpoint_by_sequence_number(seq))
        }))
    }

    fn indexes(&self) -> Option<&dyn iota_types::storage::RestIndexes> {
        None
    }

    fn get_struct_layout(
        &self,
        _: &move_core_types::language_storage::StructTag,
    ) -> iota_types::storage::error::Result<Option<move_core_types::annotated_value::MoveTypeLayout>>
    {
        Ok(None)
    }
}

impl Simulacrum {
    /// Generate a random transfer transaction.
    /// TODO: This is here today to make it easier to write tests. But we should
    /// utilize all the existing code for generating transactions in
    /// iota-test-transaction-builder by defining a trait
    /// that both WalletContext and Simulacrum implement. Then we can remove
    /// this function.
    pub fn transfer_txn(&self, recipient: IotaAddress) -> (Transaction, u64) {
        let (sender, key) = self.with_keystore(|keystore| {
            let (s, k) = keystore.accounts().next().unwrap();
            (*s, k.copy())
        });

        let (object, gas_coin_value) = self.with_store(|store| {
            let object = store
                .owned_objects(sender)
                .find(|object| object.is_gas_coin())
                .unwrap();
            let gas_coin = GasCoin::try_from(object).unwrap();
            (object.clone(), gas_coin.value())
        });
        let transfer_amount = gas_coin_value / 2;

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            builder.transfer_iota(recipient, Some(transfer_amount));
            builder.finish()
        };

        let kind = TransactionKind::ProgrammableTransaction(pt);
        let gas_data = GasData {
            payment: vec![object.compute_object_reference()],
            owner: sender,
            price: self.reference_gas_price(),
            budget: 1_000_000_000,
        };
        let tx_data = TransactionData::new_with_gas_data(kind, sender, gas_data);
        let tx = Transaction::from_data_and_signer(tx_data, vec![&key]);
        (tx, transfer_amount)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iota_types::{
        base_types::IotaAddress, effects::TransactionEffectsAPI, gas_coin::GasCoin,
        transaction::TransactionDataAPI,
    };
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;

    #[test]
    fn deterministic_genesis() {
        let rng = StdRng::from_seed([9; 32]);
        let chain1 = Simulacrum::new_with_rng(rng);
        let genesis_checkpoint_digest1 = chain1
            .with_store(|store| *store.get_checkpoint_by_sequence_number(0).unwrap().digest());

        let rng = StdRng::from_seed([9; 32]);
        let chain2 = Simulacrum::new_with_rng(rng);
        let genesis_checkpoint_digest2 = chain2
            .with_store(|store| *store.get_checkpoint_by_sequence_number(0).unwrap().digest());

        assert_eq!(genesis_checkpoint_digest1, genesis_checkpoint_digest2);

        // Ensure the committees are different when using different seeds
        let rng = StdRng::from_seed([0; 32]);
        let chain3 = Simulacrum::new_with_rng(rng);

        let committee1 = chain1.with_store(|store| store.get_committee_by_epoch(0).cloned());
        let committee3 = chain3.with_store(|store| store.get_committee_by_epoch(0).cloned());
        assert_ne!(committee1, committee3);
    }

    #[test]
    fn simple() {
        let steps = 10;
        let sim = Simulacrum::new();

        let start_time_ms = sim.with_store(|store| {
            let clock = store.get_clock();
            println!("clock: {clock:#?}");
            clock.timestamp_ms()
        });

        for _ in 0..steps {
            sim.advance_clock(Duration::from_millis(1));
            sim.create_checkpoint();
            sim.with_store(|store| {
                let clock = store.get_clock();
                println!("clock: {clock:#?}");
            });
        }
        let end_time_ms = sim.with_store(|store| store.get_clock().timestamp_ms());
        assert_eq!(end_time_ms - start_time_ms, steps);
        sim.with_store(|store| {
            dbg!(store.get_highest_checkpoint());
        });
    }

    #[test]
    fn simple_epoch() {
        let steps = 10;
        let sim = Simulacrum::new();

        let start_epoch = sim.with_store(|store| store.get_highest_checkpoint().unwrap().epoch);
        for i in 0..steps {
            sim.advance_epoch();
            sim.advance_clock(Duration::from_millis(1));
            sim.create_checkpoint();
            println!("{i}");
        }
        let end_epoch = sim.with_store(|store| store.get_highest_checkpoint().unwrap().epoch);
        assert_eq!(end_epoch - start_epoch, steps);
        sim.with_store(|store| {
            dbg!(store.get_highest_checkpoint());
        });
    }

    #[test]
    fn transfer() {
        let sim = Simulacrum::new();
        let recipient = IotaAddress::random_for_testing_only();
        let (tx, transfer_amount) = sim.transfer_txn(recipient);

        let gas_id = tx.data().transaction_data().gas_data().payment[0].0;
        let effects = sim.execute_transaction(tx).unwrap().0;
        let gas_summary = effects.gas_cost_summary();
        let gas_paid = gas_summary.net_gas_usage();

        sim.with_store(|store| {
            assert_eq!(
                (transfer_amount as i64 - gas_paid) as u64,
                store::SimulatorStore::get_object(store, &gas_id)
                    .and_then(|object| GasCoin::try_from(&object).ok())
                    .unwrap()
                    .value()
            );

            assert_eq!(
                transfer_amount,
                store
                    .owned_objects(recipient)
                    .next()
                    .and_then(|object| GasCoin::try_from(object).ok())
                    .unwrap()
                    .value()
            );
        });

        let checkpoint = sim.create_checkpoint();

        assert_eq!(&checkpoint.epoch_rolling_gas_cost_summary, gas_summary);
        assert_eq!(checkpoint.network_total_transactions, 2); // genesis + 1 txn
    }
}
