// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use iota_config::genesis;
use iota_types::{
    base_types::{AuthorityName, IotaAddress, ObjectID, SequenceNumber},
    committee::{Committee, EpochId},
    crypto::{AccountKeyPair, AuthorityKeyPair},
    digests::{ObjectDigest, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    error::IotaError,
    messages_checkpoint::{
        CheckpointContents, CheckpointContentsDigest, CheckpointDigest, CheckpointSequenceNumber,
        VerifiedCheckpoint,
    },
    object::{Object, Owner},
    storage::{
        BackingPackageStore, ChildObjectResolver, ObjectStore, PackageObject, ReadStore,
        get_module, load_package_object_from_object_store,
    },
    transaction::VerifiedTransaction,
};
use move_binary_format::CompiledModule;
use move_bytecode_utils::module_cache::GetModule;
use move_core_types::{language_storage::ModuleId, resolver::ModuleResolver};

use super::SimulatorStore;

#[derive(Debug, Default)]
pub struct InMemoryStore {
    // Checkpoint data
    checkpoints: BTreeMap<CheckpointSequenceNumber, VerifiedCheckpoint>,
    checkpoint_digest_to_sequence_number: HashMap<CheckpointDigest, CheckpointSequenceNumber>,
    checkpoint_contents: HashMap<CheckpointContentsDigest, CheckpointContents>,

    // Transaction data
    transactions: HashMap<TransactionDigest, VerifiedTransaction>,
    effects: HashMap<TransactionDigest, TransactionEffects>,
    events: HashMap<TransactionDigest, TransactionEvents>,

    // Committee data
    epoch_to_committee: Vec<Committee>,

    // Epoch data
    last_checkpoints_per_epoch: HashMap<EpochId, CheckpointSequenceNumber>,

    // Historical system states by epoch
    historical_system_states: HashMap<EpochId, iota_types::iota_system_state::IotaSystemState>,

    // Object data
    live_objects: HashMap<ObjectID, SequenceNumber>,
    objects: HashMap<ObjectID, BTreeMap<SequenceNumber, Object>>,
}

impl InMemoryStore {
    pub fn new(genesis: &genesis::Genesis) -> Self {
        let mut store = Self::default();
        store.init_with_genesis(genesis);

        // Store the initial system state for epoch 0
        let initial_system_state = store.get_system_state();
        store.store_system_state_for_epoch(0, initial_system_state);

        store
    }

    pub fn get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Option<&VerifiedCheckpoint> {
        self.checkpoints.get(&sequence_number)
    }

    pub fn get_checkpoint_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> Option<&VerifiedCheckpoint> {
        self.checkpoint_digest_to_sequence_number
            .get(digest)
            .and_then(|sequence_number| self.get_checkpoint_by_sequence_number(*sequence_number))
    }

    pub fn get_highest_checkpoint(&self) -> Option<&VerifiedCheckpoint> {
        self.checkpoints
            .last_key_value()
            .map(|(_, checkpoint)| checkpoint)
    }

    pub fn get_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Option<&CheckpointContents> {
        self.checkpoint_contents.get(digest)
    }

    pub fn get_committee_by_epoch(&self, epoch: EpochId) -> Option<&Committee> {
        self.epoch_to_committee.get(epoch as usize)
    }

    pub fn get_transaction(&self, digest: &TransactionDigest) -> Option<&VerifiedTransaction> {
        self.transactions.get(digest)
    }

    pub fn get_transaction_effects(
        &self,
        digest: &TransactionDigest,
    ) -> Option<&TransactionEffects> {
        self.effects.get(digest)
    }

    pub fn get_transaction_events(&self, digest: &TransactionDigest) -> Option<&TransactionEvents> {
        self.events.get(digest)
    }

    pub fn get_object(&self, id: &ObjectID) -> Option<&Object> {
        let version = self.live_objects.get(id)?;
        self.get_object_at_version(id, *version)
    }

    pub fn get_object_at_version(&self, id: &ObjectID, version: SequenceNumber) -> Option<&Object> {
        self.objects
            .get(id)
            .and_then(|versions| versions.get(&version))
    }

    pub fn get_system_state(&self) -> iota_types::iota_system_state::IotaSystemState {
        iota_types::iota_system_state::get_iota_system_state(self).expect("system state must exist")
    }

    pub fn get_clock(&self) -> iota_types::clock::Clock {
        self.get_object(&iota_types::IOTA_CLOCK_OBJECT_ID)
            .expect("clock should exist")
            .to_rust()
            .expect("clock object should deserialize")
    }

    pub fn get_last_checkpoint_of_epoch(&self, epoch: EpochId) -> Option<CheckpointSequenceNumber> {
        self.last_checkpoints_per_epoch.get(&epoch).cloned()
    }

    /// Get the system state for a specific epoch.
    /// Returns None if the system state for that epoch was not stored.
    pub fn get_system_state_by_epoch(
        &self,
        epoch: EpochId,
    ) -> Option<&iota_types::iota_system_state::IotaSystemState> {
        self.historical_system_states.get(&epoch)
    }

    /// Store the system state for a specific epoch.
    /// This should be called when an epoch ends to preserve the final system
    /// state.
    pub fn store_system_state_for_epoch(
        &mut self,
        epoch: EpochId,
        system_state: iota_types::iota_system_state::IotaSystemState,
    ) {
        self.historical_system_states.insert(epoch, system_state);
    }

    pub fn owned_objects(&self, owner: IotaAddress) -> impl Iterator<Item = &Object> {
        self.live_objects
            .iter()
            .flat_map(|(id, version)| self.get_object_at_version(id, *version))
            .filter(
                move |object| matches!(object.owner, Owner::AddressOwner(addr) if addr == owner),
            )
    }

    pub fn update_last_checkpoint_by_epoch(
        &mut self,
        epoch: EpochId,
        last_checkpoint: CheckpointSequenceNumber,
    ) {
        self.last_checkpoints_per_epoch
            .entry(epoch)
            .and_modify(|last| {
                *last = last_checkpoint;
            })
            .or_insert(last_checkpoint);
    }
}

impl InMemoryStore {
    pub fn insert_checkpoint(&mut self, checkpoint: VerifiedCheckpoint) {
        if let Some(end_of_epoch_data) = &checkpoint.data().end_of_epoch_data {
            // Store the current system state for the ending epoch before transitioning
            let current_epoch = checkpoint.epoch();
            let current_system_state = self.get_system_state();
            self.store_system_state_for_epoch(current_epoch, current_system_state);

            let next_committee = end_of_epoch_data
                .next_epoch_committee
                .iter()
                .cloned()
                .collect();
            let committee =
                Committee::new(checkpoint.epoch().checked_add(1).unwrap(), next_committee);
            self.insert_committee(committee);
        }

        self.checkpoint_digest_to_sequence_number
            .insert(*checkpoint.digest(), *checkpoint.sequence_number());
        self.checkpoints
            .insert(*checkpoint.sequence_number(), checkpoint);
    }

    pub fn insert_checkpoint_contents(&mut self, contents: CheckpointContents) {
        self.checkpoint_contents
            .insert(*contents.digest(), contents);
    }

    pub fn insert_committee(&mut self, committee: Committee) {
        let epoch = committee.epoch as usize;

        if self.epoch_to_committee.get(epoch).is_some() {
            return;
        }

        if self.epoch_to_committee.len() == epoch {
            self.epoch_to_committee.push(committee);
        } else {
            panic!("committee was inserted into EpochCommitteeMap out of order");
        }
    }

    pub fn insert_executed_transaction(
        &mut self,
        transaction: VerifiedTransaction,
        effects: TransactionEffects,
        events: TransactionEvents,
        written_objects: BTreeMap<ObjectID, Object>,
    ) {
        let deleted_objects = effects.deleted();
        let tx_digest = *effects.transaction_digest();
        self.insert_transaction(transaction);
        self.insert_transaction_effects(effects);
        self.insert_events(&tx_digest, events);
        self.update_objects(written_objects, deleted_objects);
    }

    pub fn insert_transaction(&mut self, transaction: VerifiedTransaction) {
        self.transactions.insert(*transaction.digest(), transaction);
    }

    pub fn insert_transaction_effects(&mut self, effects: TransactionEffects) {
        self.effects.insert(*effects.transaction_digest(), effects);
    }

    pub fn insert_events(&mut self, tx_digest: &TransactionDigest, events: TransactionEvents) {
        self.events.insert(*tx_digest, events);
    }

    pub fn update_objects(
        &mut self,
        written_objects: BTreeMap<ObjectID, Object>,
        deleted_objects: Vec<(ObjectID, SequenceNumber, ObjectDigest)>,
    ) {
        for (object_id, _, _) in deleted_objects {
            self.live_objects.remove(&object_id);
        }

        for (object_id, object) in written_objects {
            let version = object.version();
            self.live_objects.insert(object_id, version);
            self.objects
                .entry(object_id)
                .or_default()
                .insert(version, object);
        }
    }
}

impl BackingPackageStore for InMemoryStore {
    fn get_package_object(
        &self,
        package_id: &ObjectID,
    ) -> iota_types::error::IotaResult<Option<PackageObject>> {
        load_package_object_from_object_store(self, package_id)
    }
}

impl ChildObjectResolver for InMemoryStore {
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> iota_types::error::IotaResult<Option<Object>> {
        let child_object = match crate::store::SimulatorStore::get_object(self, child) {
            None => return Ok(None),
            Some(obj) => obj,
        };

        let parent = *parent;
        if child_object.owner != Owner::ObjectOwner(parent.into()) {
            return Err(IotaError::InvalidChildObjectAccess {
                object: *child,
                given_parent: parent,
                actual_owner: child_object.owner,
            });
        }

        if child_object.version() > child_version_upper_bound {
            return Err(IotaError::UnsupportedFeature {
                error: "TODO InMemoryStorage::read_child_object does not yet support bounded reads"
                    .to_owned(),
            });
        }

        Ok(Some(child_object))
    }

    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        _epoch_id: EpochId,
    ) -> iota_types::error::IotaResult<Option<Object>> {
        let recv_object = match crate::store::SimulatorStore::get_object(self, receiving_object_id)
        {
            None => return Ok(None),
            Some(obj) => obj,
        };
        if recv_object.owner != Owner::AddressOwner((*owner).into()) {
            return Ok(None);
        }

        if recv_object.version() != receive_object_at_version {
            return Ok(None);
        }
        Ok(Some(recv_object))
    }
}

impl GetModule for InMemoryStore {
    type Error = IotaError;
    type Item = CompiledModule;

    fn get_module_by_id(&self, id: &ModuleId) -> Result<Option<Self::Item>, Self::Error> {
        Ok(self
            .get_module(id)?
            .map(|bytes| CompiledModule::deserialize_with_defaults(&bytes).unwrap()))
    }
}

impl ModuleResolver for InMemoryStore {
    type Error = IotaError;

    fn get_module(&self, module_id: &ModuleId) -> Result<Option<Vec<u8>>, Self::Error> {
        get_module(self, module_id)
    }
}

impl ObjectStore for InMemoryStore {
    fn try_get_object(
        &self,
        object_id: &ObjectID,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        Ok(self.get_object(object_id).cloned())
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: iota_types::base_types::VersionNumber,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        Ok(self.get_object_at_version(object_id, version).cloned())
    }
}

impl ReadStore for InMemoryStore {
    fn try_get_committee(
        &self,
        epoch: iota_types::committee::EpochId,
    ) -> iota_types::storage::error::Result<Option<std::sync::Arc<Committee>>> {
        Ok(self
            .get_committee_by_epoch(epoch)
            .cloned()
            .map(std::sync::Arc::new))
    }

    fn try_get_latest_checkpoint(&self) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        self.get_highest_checkpoint()
            .cloned()
            .ok_or(iota_types::storage::error::Error::missing(
                "no checkpoints in store",
            ))
    }

    fn try_get_highest_verified_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        self.get_highest_checkpoint()
            .cloned()
            .ok_or(iota_types::storage::error::Error::missing(
                "no checkpoints in store",
            ))
    }

    fn try_get_highest_synced_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        self.get_highest_checkpoint()
            .cloned()
            .ok_or(iota_types::storage::error::Error::missing(
                "no checkpoints in store",
            ))
    }

    fn try_get_lowest_available_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<iota_types::messages_checkpoint::CheckpointSequenceNumber>
    {
        // we never prune the sim store
        Ok(0)
    }

    fn try_get_checkpoint_by_digest(
        &self,
        digest: &iota_types::messages_checkpoint::CheckpointDigest,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        Ok(self.get_checkpoint_by_digest(digest).cloned())
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        sequence_number: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        Ok(self
            .get_checkpoint_by_sequence_number(sequence_number)
            .cloned())
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        Ok(self.get_checkpoint_contents(digest).cloned())
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        Ok(self
            .get_checkpoint_by_sequence_number(sequence_number)
            .and_then(|c| self.get_checkpoint_contents(&c.content_digest).cloned()))
    }

    fn try_get_transaction(
        &self,
        tx_digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<Arc<VerifiedTransaction>>> {
        Ok(self.get_transaction(tx_digest).cloned().map(Arc::new))
    }

    fn try_get_transaction_effects(
        &self,
        tx_digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<TransactionEffects>> {
        Ok(self.get_transaction_effects(tx_digest).cloned())
    }

    fn try_get_events(
        &self,
        digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<iota_types::effects::TransactionEvents>> {
        Ok(self.get_transaction_events(digest).cloned())
    }

    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        self.try_get_checkpoint_contents_by_sequence_number(sequence_number)?
            .map_or(Ok(None), |contents| {
                iota_types::messages_checkpoint::FullCheckpointContents::try_from_checkpoint_contents(
                    self,
                    contents,
                )
            })
    }

    fn try_get_full_checkpoint_contents(
        &self,
        digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        self.get_checkpoint_contents(digest)
            .map_or(Ok(None), |contents| {
                iota_types::messages_checkpoint::FullCheckpointContents::try_from_checkpoint_contents(
                    self,
                    contents.clone(),
                )
            })
    }
}

#[derive(Debug)]
pub struct KeyStore {
    validator_keys: BTreeMap<AuthorityName, AuthorityKeyPair>,
    account_keys: BTreeMap<IotaAddress, AccountKeyPair>,
}

impl Clone for KeyStore {
    fn clone(&self) -> Self {
        use fastcrypto::traits::KeyPair;
        Self {
            validator_keys: self
                .validator_keys
                .iter()
                .map(|(k, v)| (*k, v.copy()))
                .collect(),
            account_keys: self
                .account_keys
                .iter()
                .map(|(k, v)| (*k, v.copy()))
                .collect(),
        }
    }
}

impl KeyStore {
    pub fn from_network_config(
        network_config: &iota_swarm_config::network_config::NetworkConfig,
    ) -> Self {
        use fastcrypto::traits::KeyPair;

        let validator_keys = network_config
            .validator_configs()
            .iter()
            .map(|config| {
                (
                    config.authority_public_key(),
                    config.authority_key_pair().copy(),
                )
            })
            .collect();

        let account_keys = network_config
            .account_keys
            .iter()
            .map(|key| (key.public().into(), key.copy()))
            .collect();
        Self {
            validator_keys,
            account_keys,
        }
    }

    pub fn validator(&self, name: &AuthorityName) -> Option<&AuthorityKeyPair> {
        self.validator_keys.get(name)
    }

    pub fn accounts(&self) -> impl Iterator<Item = (&IotaAddress, &AccountKeyPair)> {
        self.account_keys.iter()
    }
}

impl SimulatorStore for InMemoryStore {
    fn get_highest_checkpoint(&self) -> Option<VerifiedCheckpoint> {
        self.get_highest_checkpoint().cloned()
    }

    fn get_object(&self, id: &ObjectID) -> Option<Object> {
        self.get_object(id).cloned()
    }

    fn get_object_at_version(&self, id: &ObjectID, version: SequenceNumber) -> Option<Object> {
        self.get_object_at_version(id, version).cloned()
    }

    fn get_system_state(&self) -> iota_types::iota_system_state::IotaSystemState {
        self.get_system_state()
    }

    fn get_clock(&self) -> iota_types::clock::Clock {
        self.get_clock()
    }

    fn get_last_checkpoint_of_epoch(&self, epoch: EpochId) -> Option<CheckpointSequenceNumber> {
        self.get_last_checkpoint_of_epoch(epoch)
    }

    fn get_system_state_by_epoch(
        &self,
        epoch: EpochId,
    ) -> Option<&iota_types::iota_system_state::IotaSystemState> {
        self.get_system_state_by_epoch(epoch)
    }

    fn owned_objects(&self, owner: IotaAddress) -> Box<dyn Iterator<Item = Object> + '_> {
        Box::new(self.owned_objects(owner).cloned())
    }

    fn insert_checkpoint(&mut self, checkpoint: VerifiedCheckpoint) {
        self.insert_checkpoint(checkpoint)
    }

    fn insert_checkpoint_contents(&mut self, contents: CheckpointContents) {
        self.insert_checkpoint_contents(contents)
    }

    fn insert_committee(&mut self, committee: Committee) {
        self.insert_committee(committee)
    }

    fn insert_executed_transaction(
        &mut self,
        transaction: VerifiedTransaction,
        effects: TransactionEffects,
        events: TransactionEvents,
        written_objects: BTreeMap<ObjectID, Object>,
    ) {
        self.insert_executed_transaction(transaction, effects, events, written_objects)
    }

    fn insert_transaction(&mut self, transaction: VerifiedTransaction) {
        self.insert_transaction(transaction)
    }

    fn insert_transaction_effects(&mut self, effects: TransactionEffects) {
        self.insert_transaction_effects(effects)
    }

    fn insert_events(&mut self, tx_digest: &TransactionDigest, events: TransactionEvents) {
        self.insert_events(tx_digest, events)
    }

    fn update_objects(
        &mut self,
        written_objects: BTreeMap<ObjectID, Object>,
        deleted_objects: Vec<(ObjectID, SequenceNumber, ObjectDigest)>,
    ) {
        self.update_objects(written_objects, deleted_objects)
    }

    fn backing_store(&self) -> &dyn iota_types::storage::BackingStore {
        self
    }

    fn update_last_checkpoint_of_epoch(
        &mut self,
        epoch: EpochId,
        last_checkpoint: CheckpointSequenceNumber,
    ) {
        self.update_last_checkpoint_by_epoch(epoch, last_checkpoint);
    }
}
