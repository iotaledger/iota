// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, IotaTransactionKind,
    ObjectChange,
};
use iota_types::{
    base_types::{IotaAddress, ObjectDigest, ObjectID, SequenceNumber},
    crypto::AggregateAuthoritySignature,
    digests::TransactionDigest,
    dynamic_field::DynamicFieldType,
    effects::TransactionEffects,
    event::{SystemEpochInfoEvent, SystemEpochInfoEventV1, SystemEpochInfoEventV2},
    iota_serde::IotaStructTag,
    iota_system_state::iota_system_state_summary::{IotaSystemStateSummary, IotaValidatorSummary},
    messages_checkpoint::{
        CheckpointCommitment, CheckpointDigest, CheckpointSequenceNumber, EndOfEpochData,
    },
    move_package::MovePackage,
    object::{Object, Owner},
    transaction::SenderSignedData,
};
use move_core_types::language_storage::StructTag;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::errors::IndexerError;

pub type IndexerResult<T> = Result<T, IndexerError>;

#[derive(Debug)]
pub struct IndexedCheckpoint {
    pub sequence_number: u64,
    pub checkpoint_digest: CheckpointDigest,
    pub epoch: u64,
    pub tx_digests: Vec<TransactionDigest>,
    pub network_total_transactions: u64,
    pub previous_checkpoint_digest: Option<CheckpointDigest>,
    pub timestamp_ms: u64,
    pub total_gas_cost: i64, // total gas cost could be negative
    pub computation_cost: u64,
    pub computation_cost_burned: u64,
    pub storage_cost: u64,
    pub storage_rebate: u64,
    pub non_refundable_storage_fee: u64,
    pub checkpoint_commitments: Vec<CheckpointCommitment>,
    pub validator_signature: AggregateAuthoritySignature,
    pub successful_tx_num: usize,
    pub end_of_epoch_data: Option<EndOfEpochData>,
    pub end_of_epoch: bool,
    pub min_tx_sequence_number: u64,
    pub max_tx_sequence_number: u64,
}

impl IndexedCheckpoint {
    pub fn from_iota_checkpoint(
        checkpoint: &iota_types::messages_checkpoint::CertifiedCheckpointSummary,
        contents: &iota_types::messages_checkpoint::CheckpointContents,
        successful_tx_num: usize,
    ) -> Self {
        let total_gas_cost = checkpoint.epoch_rolling_gas_cost_summary.computation_cost as i64
            + checkpoint.epoch_rolling_gas_cost_summary.storage_cost as i64
            - checkpoint.epoch_rolling_gas_cost_summary.storage_rebate as i64;
        let tx_digests = contents.iter().map(|t| t.transaction).collect::<Vec<_>>();
        let max_tx_sequence_number = checkpoint.network_total_transactions - 1;
        // NOTE: + 1u64 first to avoid subtraction with overflow
        let min_tx_sequence_number = max_tx_sequence_number + 1u64 - tx_digests.len() as u64;
        let auth_sig = &checkpoint.auth_sig().signature;
        Self {
            sequence_number: checkpoint.sequence_number,
            checkpoint_digest: *checkpoint.digest(),
            epoch: checkpoint.epoch,
            tx_digests,
            previous_checkpoint_digest: checkpoint.previous_digest,
            end_of_epoch_data: checkpoint.end_of_epoch_data.clone(),
            end_of_epoch: checkpoint.end_of_epoch_data.clone().is_some(),
            total_gas_cost,
            computation_cost: checkpoint.epoch_rolling_gas_cost_summary.computation_cost,
            computation_cost_burned: checkpoint
                .epoch_rolling_gas_cost_summary
                .computation_cost_burned,
            storage_cost: checkpoint.epoch_rolling_gas_cost_summary.storage_cost,
            storage_rebate: checkpoint.epoch_rolling_gas_cost_summary.storage_rebate,
            non_refundable_storage_fee: checkpoint
                .epoch_rolling_gas_cost_summary
                .non_refundable_storage_fee,
            successful_tx_num,
            network_total_transactions: checkpoint.network_total_transactions,
            timestamp_ms: checkpoint.timestamp_ms,
            validator_signature: auth_sig.clone(),
            checkpoint_commitments: checkpoint.checkpoint_commitments.clone(),
            min_tx_sequence_number,
            max_tx_sequence_number,
        }
    }
}

/// View on the common inner state-summary fields.
///
/// This exposes whatever makes sense to the scope of this
/// library.
pub(crate) trait IotaSystemStateSummaryView {
    fn epoch(&self) -> u64;
    fn epoch_start_timestamp_ms(&self) -> u64;
    fn reference_gas_price(&self) -> u64;
    fn protocol_version(&self) -> u64;
    fn iota_total_supply(&self) -> u64;
    fn active_validators(&self) -> &[IotaValidatorSummary];
    fn inactive_pools_id(&self) -> ObjectID;
    fn inactive_pools_size(&self) -> u64;
    fn validator_candidates_id(&self) -> ObjectID;
    fn validator_candidates_size(&self) -> u64;
    fn to_committee_members(&self) -> Vec<u64>;
}

/// Access common fields of the inner variants wrapped by
/// [`IotaSystemStateSummary`].
///
/// ## Panics
///
/// If the `field` identifier does not correspond to an existing field in any
/// of the inner types wrapped in the variants.
macro_rules! state_summary_get {
    ($enum:expr, $field:ident) => {{
        match $enum {
            IotaSystemStateSummary::V1(ref inner) => &inner.$field,
            IotaSystemStateSummary::V2(ref inner) => &inner.$field,
            _ => unimplemented!(),
        }
    }};
}

impl IotaSystemStateSummaryView for IotaSystemStateSummary {
    fn epoch(&self) -> u64 {
        *state_summary_get!(self, epoch)
    }

    fn epoch_start_timestamp_ms(&self) -> u64 {
        *state_summary_get!(self, epoch_start_timestamp_ms)
    }

    fn reference_gas_price(&self) -> u64 {
        *state_summary_get!(self, reference_gas_price)
    }

    fn protocol_version(&self) -> u64 {
        *state_summary_get!(self, protocol_version)
    }

    fn iota_total_supply(&self) -> u64 {
        *state_summary_get!(self, iota_total_supply)
    }

    fn active_validators(&self) -> &[IotaValidatorSummary] {
        state_summary_get!(self, active_validators)
    }

    fn inactive_pools_id(&self) -> ObjectID {
        *state_summary_get!(self, inactive_pools_id)
    }

    fn inactive_pools_size(&self) -> u64 {
        *state_summary_get!(self, inactive_pools_size)
    }

    fn validator_candidates_id(&self) -> ObjectID {
        *state_summary_get!(self, validator_candidates_id)
    }

    fn validator_candidates_size(&self) -> u64 {
        *state_summary_get!(self, validator_candidates_size)
    }

    fn to_committee_members(&self) -> Vec<u64> {
        match self {
            IotaSystemStateSummary::V1(inner) => (0..inner.active_validators.len())
                .map(|i| i as u64)
                .collect(),
            IotaSystemStateSummary::V2(inner) => inner.committee_members.clone(),
            _ => unimplemented!(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct IndexedEpochInfoEvent {
    pub storage_charge: u64,
    pub storage_rebate: u64,
    pub total_gas_fees: u64,
    pub total_stake_rewards_distributed: u64,
    pub burnt_tokens_amount: u64,
    pub minted_tokens_amount: u64,
    pub total_stake: u64,
    pub storage_fund_balance: u64,
}

impl From<&SystemEpochInfoEventV1> for IndexedEpochInfoEvent {
    fn from(event: &SystemEpochInfoEventV1) -> Self {
        Self {
            storage_charge: event.storage_charge,
            storage_rebate: event.storage_rebate,
            total_gas_fees: event.total_gas_fees,
            total_stake_rewards_distributed: event.total_stake_rewards_distributed,
            burnt_tokens_amount: event.burnt_tokens_amount,
            minted_tokens_amount: event.minted_tokens_amount,
            total_stake: event.total_stake,
            storage_fund_balance: event.storage_fund_balance,
        }
    }
}

impl From<&SystemEpochInfoEventV2> for IndexedEpochInfoEvent {
    fn from(event: &SystemEpochInfoEventV2) -> Self {
        Self {
            storage_charge: event.storage_charge,
            storage_rebate: event.storage_rebate,
            total_gas_fees: event.total_gas_fees,
            total_stake_rewards_distributed: event.total_stake_rewards_distributed,
            burnt_tokens_amount: event.burnt_tokens_amount,
            minted_tokens_amount: event.minted_tokens_amount,
            total_stake: event.total_stake,
            storage_fund_balance: event.storage_fund_balance,
        }
    }
}

impl From<&SystemEpochInfoEvent> for IndexedEpochInfoEvent {
    fn from(event: &SystemEpochInfoEvent) -> Self {
        match event {
            SystemEpochInfoEvent::V1(inner) => inner.into(),
            SystemEpochInfoEvent::V2(inner) => inner.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexedEvent {
    pub tx_sequence_number: u64,
    pub event_sequence_number: u64,
    pub checkpoint_sequence_number: u64,
    pub transaction_digest: TransactionDigest,
    pub senders: Vec<IotaAddress>,
    pub package: ObjectID,
    pub module: String,
    pub event_type: String,
    pub event_type_package: ObjectID,
    pub event_type_module: String,
    /// Struct name of the event, without type parameters.
    pub event_type_name: String,
    pub bcs: Vec<u8>,
    pub timestamp_ms: u64,
}

impl IndexedEvent {
    pub fn from_event(
        tx_sequence_number: u64,
        event_sequence_number: u64,
        checkpoint_sequence_number: u64,
        transaction_digest: TransactionDigest,
        event: &iota_types::event::Event,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            tx_sequence_number,
            event_sequence_number,
            checkpoint_sequence_number,
            transaction_digest,
            senders: vec![event.sender],
            package: event.package_id,
            module: event.transaction_module.to_string(),
            event_type: event.type_.to_canonical_string(/* with_prefix */ true),
            event_type_package: event.type_.address.into(),
            event_type_module: event.type_.module.to_string(),
            event_type_name: event.type_.name.to_string(),
            bcs: event.contents.clone(),
            timestamp_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventIndex {
    pub tx_sequence_number: u64,
    pub event_sequence_number: u64,
    pub sender: IotaAddress,
    pub emit_package: ObjectID,
    pub emit_module: String,
    pub type_package: ObjectID,
    pub type_module: String,
    /// Struct name of the event, without type parameters.
    pub type_name: String,
    /// Type instantiation of the event, with type name and type parameters, if
    /// any.
    pub type_instantiation: String,
}

impl EventIndex {
    pub fn from_event(
        tx_sequence_number: u64,
        event_sequence_number: u64,
        event: &iota_types::event::Event,
    ) -> Self {
        let type_instantiation = event
            .type_
            .to_canonical_string(/* with_prefix */ true)
            .splitn(3, "::")
            .collect::<Vec<_>>()[2]
            .to_string();
        Self {
            tx_sequence_number,
            event_sequence_number,
            sender: event.sender,
            emit_package: event.package_id,
            emit_module: event.transaction_module.to_string(),
            type_package: event.type_.address.into(),
            type_module: event.type_.module.to_string(),
            type_name: event.type_.name.to_string(),
            type_instantiation,
        }
    }
}

#[cfg(any(test, feature = "pg_integration"))]
impl EventIndex {
    /// Generate a random event index for testing purposes.
    pub fn random() -> Self {
        use rand::Rng;

        let mut rng = rand::thread_rng();
        EventIndex {
            tx_sequence_number: rng.gen(),
            event_sequence_number: rng.gen(),
            sender: IotaAddress::random_for_testing_only(),
            emit_package: ObjectID::random(),
            emit_module: rng.gen::<u64>().to_string(),
            type_package: ObjectID::random(),
            type_module: rng.gen::<u64>().to_string(),
            type_name: rng.gen::<u64>().to_string(),
            type_instantiation: rng.gen::<u64>().to_string(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum OwnerType {
    Immutable = 0,
    Address = 1,
    Object = 2,
    Shared = 3,
}

pub enum ObjectStatus {
    Active = 0,
    WrappedOrDeleted = 1,
}

impl TryFrom<i16> for ObjectStatus {
    type Error = IndexerError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => ObjectStatus::Active,
            1 => ObjectStatus::WrappedOrDeleted,
            value => {
                return Err(IndexerError::PersistentStorageDataCorruption(format!(
                    "{value} as ObjectStatus"
                )));
            }
        })
    }
}

impl TryFrom<i16> for OwnerType {
    type Error = IndexerError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => OwnerType::Immutable,
            1 => OwnerType::Address,
            2 => OwnerType::Object,
            3 => OwnerType::Shared,
            value => {
                return Err(IndexerError::PersistentStorageDataCorruption(format!(
                    "{value} as OwnerType"
                )));
            }
        })
    }
}

// Returns owner_type, owner_address
pub fn owner_to_owner_info(owner: &Owner) -> (OwnerType, Option<IotaAddress>) {
    match owner {
        Owner::AddressOwner(address) => (OwnerType::Address, Some(*address)),
        Owner::ObjectOwner(address) => (OwnerType::Object, Some(*address)),
        Owner::Shared { .. } => (OwnerType::Shared, None),
        Owner::Immutable => (OwnerType::Immutable, None),
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DynamicFieldKind {
    DynamicField = 0,
    DynamicObject = 1,
}

#[derive(Clone, Debug)]
pub struct IndexedObject {
    pub checkpoint_sequence_number: CheckpointSequenceNumber,
    pub object: Object,
    pub df_kind: Option<DynamicFieldType>,
}

impl IndexedObject {
    pub fn from_object(
        checkpoint_sequence_number: CheckpointSequenceNumber,
        object: Object,
        df_kind: Option<DynamicFieldType>,
    ) -> Self {
        Self {
            checkpoint_sequence_number,
            object,
            df_kind,
        }
    }
}

#[derive(Clone, Debug)]
pub struct IndexedDeletedObject {
    pub object_id: ObjectID,
    pub object_version: u64,
    pub checkpoint_sequence_number: u64,
}

#[derive(Debug)]
pub struct IndexedPackage {
    pub package_id: ObjectID,
    pub move_package: MovePackage,
    pub checkpoint_sequence_number: u64,
}

#[derive(Debug, Clone)]
pub struct IndexedTransaction {
    pub tx_sequence_number: u64,
    pub tx_digest: TransactionDigest,
    pub sender_signed_data: SenderSignedData,
    pub effects: TransactionEffects,
    pub checkpoint_sequence_number: u64,
    pub timestamp_ms: u64,
    pub object_changes: Vec<IndexedObjectChange>,
    pub balance_change: Vec<iota_json_rpc_types::BalanceChange>,
    pub events: Vec<iota_types::event::Event>,
    pub transaction_kind: IotaTransactionKind,
    pub successful_tx_num: u64,
}

#[derive(Debug, Clone)]
pub struct TxIndex {
    pub tx_sequence_number: u64,
    pub tx_kind: IotaTransactionKind,
    pub transaction_digest: TransactionDigest,
    pub checkpoint_sequence_number: u64,
    pub input_objects: Vec<ObjectID>,
    pub changed_objects: Vec<ObjectID>,
    pub payers: Vec<IotaAddress>,
    pub sender: IotaAddress,
    pub recipients: Vec<IotaAddress>,
    pub move_calls: Vec<(ObjectID, String, String)>,
}

#[cfg(any(test, feature = "pg_integration"))]
impl TxIndex {
    /// Generate a random TxIndex for testing purposes.
    pub fn random() -> Self {
        use std::iter::repeat_with;

        use rand::Rng;

        const MAX_OBJECTS: usize = 1000;
        const MAX_PAYERS: usize = 100;
        const MAX_RECIPIENTS: usize = 1000;
        const MAX_MOVE_CALLS: usize = 1000;

        let mut rng = rand::thread_rng();

        let tx_kind = if rng.gen_bool(0.5) {
            IotaTransactionKind::SystemTransaction
        } else {
            IotaTransactionKind::ProgrammableTransaction
        };

        let input_objects = repeat_with(ObjectID::random).take(MAX_OBJECTS).collect();
        let changed_objects = repeat_with(ObjectID::random).take(MAX_OBJECTS).collect();
        let payers = repeat_with(IotaAddress::random_for_testing_only)
            .take(rng.gen_range(0..MAX_PAYERS))
            .collect();
        let recipients = repeat_with(IotaAddress::random_for_testing_only)
            .take(rng.gen_range(0..MAX_RECIPIENTS))
            .collect();
        let move_calls = std::iter::repeat_with(|| {
            (
                ObjectID::random(),
                rand::random::<u64>().to_string(),
                rand::random::<u64>().to_string(),
            )
        })
        .take(rng.gen_range(0..MAX_MOVE_CALLS))
        .collect();

        TxIndex {
            tx_sequence_number: rng.gen(),
            tx_kind,
            transaction_digest: TransactionDigest::random(),
            checkpoint_sequence_number: rng.gen(),
            input_objects,
            changed_objects,
            payers,
            sender: IotaAddress::random_for_testing_only(),
            recipients,
            move_calls,
        }
    }
}

// ObjectChange is not bcs deserializable, IndexedObjectChange is.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum IndexedObjectChange {
    Published {
        package_id: ObjectID,
        version: SequenceNumber,
        digest: ObjectDigest,
        modules: Vec<String>,
    },
    Transferred {
        sender: IotaAddress,
        recipient: Owner,
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectID,
        version: SequenceNumber,
        digest: ObjectDigest,
    },
    /// Object mutated.
    Mutated {
        sender: IotaAddress,
        owner: Owner,
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectID,
        version: SequenceNumber,
        previous_version: SequenceNumber,
        digest: ObjectDigest,
    },
    /// Delete object
    Deleted {
        sender: IotaAddress,
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectID,
        version: SequenceNumber,
    },
    /// Wrapped object
    Wrapped {
        sender: IotaAddress,
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectID,
        version: SequenceNumber,
    },
    /// New object creation
    Created {
        sender: IotaAddress,
        owner: Owner,
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectID,
        version: SequenceNumber,
        digest: ObjectDigest,
    },
}

impl From<ObjectChange> for IndexedObjectChange {
    fn from(oc: ObjectChange) -> Self {
        match oc {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => Self::Published {
                package_id,
                version,
                digest,
                modules,
            },
            ObjectChange::Transferred {
                sender,
                recipient,
                object_type,
                object_id,
                version,
                digest,
            } => Self::Transferred {
                sender,
                recipient,
                object_type,
                object_id,
                version,
                digest,
            },
            ObjectChange::Mutated {
                sender,
                owner,
                object_type,
                object_id,
                version,
                previous_version,
                digest,
            } => Self::Mutated {
                sender,
                owner,
                object_type,
                object_id,
                version,
                previous_version,
                digest,
            },
            ObjectChange::Deleted {
                sender,
                object_type,
                object_id,
                version,
            } => Self::Deleted {
                sender,
                object_type,
                object_id,
                version,
            },
            ObjectChange::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            } => Self::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            },
            ObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => Self::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            },
        }
    }
}

impl From<IndexedObjectChange> for ObjectChange {
    fn from(val: IndexedObjectChange) -> Self {
        match val {
            IndexedObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => ObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            },
            IndexedObjectChange::Transferred {
                sender,
                recipient,
                object_type,
                object_id,
                version,
                digest,
            } => ObjectChange::Transferred {
                sender,
                recipient,
                object_type,
                object_id,
                version,
                digest,
            },
            IndexedObjectChange::Mutated {
                sender,
                owner,
                object_type,
                object_id,
                version,
                previous_version,
                digest,
            } => ObjectChange::Mutated {
                sender,
                owner,
                object_type,
                object_id,
                version,
                previous_version,
                digest,
            },
            IndexedObjectChange::Deleted {
                sender,
                object_type,
                object_id,
                version,
            } => ObjectChange::Deleted {
                sender,
                object_type,
                object_id,
                version,
            },
            IndexedObjectChange::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            } => ObjectChange::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            },
            IndexedObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => ObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            },
        }
    }
}

// IotaTransactionBlockResponseWithOptions is only used on the reading path
pub struct IotaTransactionBlockResponseWithOptions {
    pub response: IotaTransactionBlockResponse,
    pub options: IotaTransactionBlockResponseOptions,
}

impl From<IotaTransactionBlockResponseWithOptions> for IotaTransactionBlockResponse {
    fn from(value: IotaTransactionBlockResponseWithOptions) -> Self {
        let IotaTransactionBlockResponseWithOptions { response, options } = value;

        IotaTransactionBlockResponse {
            digest: response.digest,
            transaction: options.show_input.then_some(response.transaction).flatten(),
            raw_transaction: if options.show_raw_input {
                response.raw_transaction
            } else {
                Default::default()
            },
            effects: options.show_effects.then_some(response.effects).flatten(),
            events: options.show_events.then_some(response.events).flatten(),
            object_changes: options
                .show_object_changes
                .then_some(response.object_changes)
                .flatten(),
            balance_changes: options
                .show_balance_changes
                .then_some(response.balance_changes)
                .flatten(),
            timestamp_ms: response.timestamp_ms,
            confirmed_local_execution: response.confirmed_local_execution,
            checkpoint: response.checkpoint,
            errors: vec![],
            raw_effects: if options.show_raw_effects {
                response.raw_effects
            } else {
                Default::default()
            },
        }
    }
}
