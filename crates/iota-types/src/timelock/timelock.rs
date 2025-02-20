// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_stardust_sdk::types::block::output::{BasicOutput, OutputId};
use move_core_types::{
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use serde::{Deserialize, Serialize};

use super::{
    label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
};
use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    balance::Balance,
    base_types::{IotaAddress, MoveObjectType, ObjectID, SequenceNumber, TxContext},
    error::{ExecutionError, IotaError},
    gas_coin::GasCoin,
    id::UID,
    object::{Data, MoveObject, Object, Owner},
};

#[cfg(test)]
#[path = "../unit_tests/timelock/timelock_tests.rs"]
mod timelock_tests;

pub const TIMELOCK_MODULE_NAME: &IdentStr = ident_str!("timelock");
pub const TIMELOCK_STRUCT_NAME: &IdentStr = ident_str!("TimeLock");

/// All basic outputs whose IDs start with this prefix represent vested rewards
/// that were created during the stardust upgrade on IOTA mainnet.
pub const VESTED_REWARD_ID_PREFIX: &str =
    "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18";

#[derive(Debug, thiserror::Error)]
pub enum VestedRewardError {
    #[error("failed to create genesis move object, owner: {owner}, timelock: {timelock:#?}")]
    ObjectCreation {
        owner: IotaAddress,
        timelock: TimeLock<Balance>,
        source: ExecutionError,
    },
    #[error("a vested reward must not contain native tokens")]
    NativeTokensNotSupported,
    #[error("a basic output is not a vested reward")]
    NotVestedReward,
    #[error("a vested reward must have two unlock conditions")]
    UnlockConditionsNumberMismatch,
    #[error("only timelocked vested rewards can be migrated as `TimeLock<Balance<IOTA>>`")]
    UnlockedVestedReward,
}

/// Checks if an output is a timelocked vested reward.
pub fn is_timelocked_vested_reward(
    output_id: OutputId,
    basic_output: &BasicOutput,
    target_milestone_timestamp_sec: u32,
) -> bool {
    is_vested_reward(output_id, basic_output)
        && basic_output
            .unlock_conditions()
            .is_time_locked(target_milestone_timestamp_sec)
}

/// Checks if an output is a vested reward, if it has a specific ID prefix,
/// and if it contains a timelock unlock condition,
/// and if an output has no native tokens,
/// and if an output has only 2 unlock conditions and their address.
pub fn is_vested_reward(output_id: OutputId, basic_output: &BasicOutput) -> bool {
    let has_vesting_prefix = output_id.to_string().starts_with(VESTED_REWARD_ID_PREFIX);

    has_vesting_prefix
        && basic_output.unlock_conditions().timelock().is_some()
        && basic_output.native_tokens().len() == 0
        && basic_output.unlock_conditions().len() == 2
        && basic_output.unlock_conditions().address().is_some()
}

/// Creates a `TimeLock<Balance<IOTA>>` from a Stardust-based Basic Output
/// that represents a vested reward.
pub fn try_from_stardust(
    output_id: OutputId,
    basic_output: &BasicOutput,
    target_milestone_timestamp_sec: u32,
) -> Result<TimeLock<Balance>, VestedRewardError> {
    if !is_vested_reward(output_id, basic_output) {
        return Err(VestedRewardError::NotVestedReward);
    }

    if !basic_output
        .unlock_conditions()
        .is_time_locked(target_milestone_timestamp_sec)
    {
        return Err(VestedRewardError::UnlockedVestedReward);
    }

    if basic_output.unlock_conditions().len() != 2 {
        return Err(VestedRewardError::UnlockConditionsNumberMismatch);
    }

    if basic_output.native_tokens().len() > 0 {
        return Err(VestedRewardError::NativeTokensNotSupported);
    }

    let id = UID::new(ObjectID::new(output_id.hash()));
    let locked = Balance::new(basic_output.amount());

    // We already checked the existence of the timelock unlock condition at this
    // point.
    let timelock_uc = basic_output
        .unlock_conditions()
        .timelock()
        .expect("a vested reward should contain a timelock unlock condition");
    let expiration_timestamp_ms = Into::<u64>::into(timelock_uc.timestamp()) * 1000;

    let label = Option::Some(label_struct_tag_to_string(stardust_upgrade_label_type()));

    Ok(TimeLock::new(id, locked, expiration_timestamp_ms, label))
}

/// Creates a genesis object from a time-locked balance.
pub fn to_genesis_object(
    timelock: TimeLock<Balance>,
    owner: IotaAddress,
    protocol_config: &ProtocolConfig,
    tx_context: &TxContext,
    version: SequenceNumber,
) -> Result<Object, VestedRewardError> {
    let move_object = {
        MoveObject::new_from_execution(
            MoveObjectType::timelocked_iota_balance(),
            version,
            timelock.to_bcs_bytes(),
            protocol_config,
        )
        .map_err(|source| VestedRewardError::ObjectCreation {
            owner,
            timelock,
            source,
        })?
    };

    Ok(Object::new_from_genesis(
        Data::Move(move_object),
        Owner::AddressOwner(owner),
        tx_context.digest(),
    ))
}

/// Rust version of the Move stardust::TimeLock type.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct TimeLock<T> {
    id: UID,
    /// The locked object.
    locked: T,
    /// This is the epoch time stamp of when the lock expires.
    expiration_timestamp_ms: u64,
    /// Timelock related label.
    label: Option<String>,
}

impl<T> TimeLock<T> {
    /// Constructor.
    pub fn new(id: UID, locked: T, expiration_timestamp_ms: u64, label: Option<String>) -> Self {
        Self {
            id,
            locked,
            expiration_timestamp_ms,
            label,
        }
    }

    /// Get the TimeLock's `type`.
    pub fn type_(type_param: TypeTag) -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: TIMELOCK_MODULE_NAME.to_owned(),
            name: TIMELOCK_STRUCT_NAME.to_owned(),
            type_params: vec![type_param],
        }
    }

    /// Get the TimeLock's `id`.
    pub fn id(&self) -> &ObjectID {
        self.id.object_id()
    }

    /// Get the TimeLock's `locked` object.
    pub fn locked(&self) -> &T {
        &self.locked
    }

    /// Get the TimeLock's `expiration_timestamp_ms`.
    pub fn expiration_timestamp_ms(&self) -> u64 {
        self.expiration_timestamp_ms
    }

    /// Get the TimeLock's `label``.
    pub fn label(&self) -> &Option<String> {
        &self.label
    }
}

impl<'de, T> TimeLock<T>
where
    T: Serialize + Deserialize<'de>,
{
    /// Create a `TimeLock` from BCS bytes.
    pub fn from_bcs_bytes(content: &'de [u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize TimeLock object: {:?}", err),
        })
    }

    /// Serialize a `TimeLock` as a `Vec<u8>` of BCS.
    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }
}

/// Is this other StructTag representing a TimeLock?
pub fn is_timelock(other: &StructTag) -> bool {
    other.address == IOTA_FRAMEWORK_ADDRESS
        && other.module.as_ident_str() == TIMELOCK_MODULE_NAME
        && other.name.as_ident_str() == TIMELOCK_STRUCT_NAME
}

/// Is this other StructTag representing a `TimeLock<Balance<T>>`?
pub fn is_timelocked_balance(other: &StructTag) -> bool {
    if !is_timelock(other) {
        return false;
    }

    if other.type_params.len() != 1 {
        return false;
    }

    match &other.type_params[0] {
        TypeTag::Struct(tag) => Balance::is_balance(tag),
        _ => false,
    }
}

/// Is this other StructTag representing a `TimeLock<Balance<IOTA>>`?
pub fn is_timelocked_gas_balance(other: &StructTag) -> bool {
    if !is_timelock(other) {
        return false;
    }

    if other.type_params.len() != 1 {
        return false;
    }

    match &other.type_params[0] {
        TypeTag::Struct(tag) => GasCoin::is_gas_balance(tag),
        _ => false,
    }
}

impl<'de, T> TryFrom<&'de Object> for TimeLock<T>
where
    T: Serialize + Deserialize<'de>,
{
    type Error = IotaError;

    fn try_from(object: &'de Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_timelock() {
                    return TimeLock::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a TimeLock: {:?}", object),
        })
    }
}
