// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use anyhow::Result;
use enum_dispatch::enum_dispatch;
use iota_protocol_config::{ProtocolConfig, ProtocolVersion};
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use self::{
    iota_system_state_inner_v1::{IotaSystemStateV1, ValidatorV1},
    iota_system_state_inner_v2::IotaSystemStateV2,
    iota_system_state_summary::{IotaSystemStateSummary, IotaValidatorSummary},
};
use crate::{
    IOTA_SYSTEM_ADDRESS, IOTA_SYSTEM_STATE_OBJECT_ID, MoveTypeTagTrait,
    base_types::ObjectID,
    committee::CommitteeWithNetworkMetadata,
    dynamic_field::{Field, get_dynamic_field_from_store, get_dynamic_field_object_from_store},
    error::IotaError,
    id::UID,
    iota_system_state::epoch_start_iota_system_state::EpochStartSystemState,
    object::{MoveObject, Object},
    storage::ObjectStore,
    versioned::Versioned,
};

pub mod epoch_start_iota_system_state;
pub mod iota_system_state_inner_v1;
pub mod iota_system_state_inner_v2;
pub mod iota_system_state_summary;

#[cfg(msim)]
mod simtest_iota_system_state_inner;
#[cfg(msim)]
use self::simtest_iota_system_state_inner::{
    SimTestIotaSystemStateDeepV1, SimTestIotaSystemStateShallowV1, SimTestIotaSystemStateV1,
    SimTestValidatorDeepV1, SimTestValidatorV1,
};

const IOTA_SYSTEM_STATE_WRAPPER_STRUCT_NAME: &IdentStr = ident_str!("IotaSystemState");

pub const IOTA_SYSTEM_MODULE_NAME: &IdentStr = ident_str!("iota_system");
pub const ADVANCE_EPOCH_FUNCTION_NAME: &IdentStr = ident_str!("advance_epoch");
pub const ADVANCE_EPOCH_SAFE_MODE_FUNCTION_NAME: &IdentStr = ident_str!("advance_epoch_safe_mode");

#[cfg(msim)]
pub const IOTA_SYSTEM_STATE_SIM_TEST_V1: u64 = 18446744073709551605; // u64::MAX - 10
#[cfg(msim)]
pub const IOTA_SYSTEM_STATE_SIM_TEST_SHALLOW_V1: u64 = 18446744073709551606; // u64::MAX - 9
#[cfg(msim)]
pub const IOTA_SYSTEM_STATE_SIM_TEST_DEEP_V1: u64 = 18446744073709551607; // u64::MAX - 8

/// Rust version of the Move iota::iota_system::IotaSystemState type
/// This repreents the object with 0x5 ID.
/// In Rust, this type should be rarely used since it's just a thin
/// wrapper used to access the inner object.
/// Within this module, we use it to determine the current version of the system
/// state inner object type, so that we could deserialize the inner object
/// correctly. Outside of this module, we only use it in genesis snapshot and
/// testing.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IotaSystemStateWrapper {
    pub id: UID,
    pub version: u64,
}

impl IotaSystemStateWrapper {
    pub fn type_() -> StructTag {
        StructTag {
            address: IOTA_SYSTEM_ADDRESS,
            name: IOTA_SYSTEM_STATE_WRAPPER_STRUCT_NAME.to_owned(),
            module: IOTA_SYSTEM_MODULE_NAME.to_owned(),
            type_params: vec![],
        }
    }

    /// Advances epoch in safe mode natively in Rust, without involking Move.
    /// This ensures that there cannot be any failure from Move and is
    /// guaranteed to succeed. Returns the old and new inner system state
    /// object.
    pub fn advance_epoch_safe_mode(
        &self,
        params: &AdvanceEpochParams,
        object_store: &dyn ObjectStore,
        protocol_config: &ProtocolConfig,
    ) -> (Object, Object) {
        let id = self.id.id.bytes;
        let old_field_object = get_dynamic_field_object_from_store(object_store, id, &self.version)
            .expect("Dynamic field object of wrapper should always be present in the object store");
        let mut new_field_object = old_field_object.clone();
        let move_object = new_field_object
            .data
            .try_as_move_mut()
            .expect("Dynamic field object must be a Move object");
        match self.version {
            1 => {
                Self::advance_epoch_safe_mode_impl::<IotaSystemStateV1>(
                    move_object,
                    params,
                    protocol_config,
                );
            }
            2 => {
                Self::advance_epoch_safe_mode_impl::<IotaSystemStateV2>(
                    move_object,
                    params,
                    protocol_config,
                );
            }
            #[cfg(msim)]
            IOTA_SYSTEM_STATE_SIM_TEST_V1 => {
                Self::advance_epoch_safe_mode_impl::<SimTestIotaSystemStateV1>(
                    move_object,
                    params,
                    protocol_config,
                );
            }
            #[cfg(msim)]
            IOTA_SYSTEM_STATE_SIM_TEST_SHALLOW_V1 => {
                Self::advance_epoch_safe_mode_impl::<SimTestIotaSystemStateShallowV1>(
                    move_object,
                    params,
                    protocol_config,
                );
            }
            #[cfg(msim)]
            IOTA_SYSTEM_STATE_SIM_TEST_DEEP_V1 => {
                Self::advance_epoch_safe_mode_impl::<SimTestIotaSystemStateDeepV1>(
                    move_object,
                    params,
                    protocol_config,
                );
            }
            _ => unreachable!(),
        }
        (old_field_object, new_field_object)
    }

    fn advance_epoch_safe_mode_impl<T>(
        move_object: &mut MoveObject,
        params: &AdvanceEpochParams,
        protocol_config: &ProtocolConfig,
    ) where
        T: Serialize + DeserializeOwned + IotaSystemStateTrait,
    {
        let mut field: Field<u64, T> =
            bcs::from_bytes(move_object.contents()).expect("bcs deserialization should never fail");
        tracing::info!(
            "Advance epoch safe mode: current epoch: {}, protocol_version: {}, system_state_version: {}",
            field.value.epoch(),
            field.value.protocol_version(),
            field.value.system_state_version()
        );
        field.value.advance_epoch_safe_mode(params);
        tracing::info!(
            "Safe mode activated. New epoch: {}, protocol_version: {}, system_state_version: {}",
            field.value.epoch(),
            field.value.protocol_version(),
            field.value.system_state_version()
        );
        let new_contents = bcs::to_bytes(&field).expect("bcs serialization should never fail");
        move_object
            .update_contents(new_contents, protocol_config)
            .expect("Update iota system object content cannot fail since it should be small");
    }
}

/// This is the standard API that all inner system state object type should
/// implement.
#[enum_dispatch]
pub trait IotaSystemStateTrait {
    fn epoch(&self) -> u64;
    fn reference_gas_price(&self) -> u64;
    fn protocol_version(&self) -> u64;
    fn system_state_version(&self) -> u64;
    fn epoch_start_timestamp_ms(&self) -> u64;
    fn epoch_duration_ms(&self) -> u64;
    fn safe_mode(&self) -> bool;
    fn advance_epoch_safe_mode(&mut self, params: &AdvanceEpochParams);
    fn get_current_epoch_committee(&self) -> CommitteeWithNetworkMetadata;
    fn get_pending_active_validators<S: ObjectStore + ?Sized>(
        &self,
        object_store: &S,
    ) -> Result<Vec<IotaValidatorSummary>, IotaError>;
    fn into_epoch_start_state(self) -> EpochStartSystemState;
    fn into_iota_system_state_summary(self) -> IotaSystemStateSummary;
}

/// IotaSystemState provides an abstraction over multiple versions of the inner
/// IotaSystemStateInner object. This should be the primary interface to the
/// system state object in Rust. We use enum dispatch to dispatch all methods
/// defined in IotaSystemStateTrait to the actual implementation in the inner
/// types.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[enum_dispatch(IotaSystemStateTrait)]
pub enum IotaSystemState {
    V1(IotaSystemStateV1),
    V2(IotaSystemStateV2),
    #[cfg(msim)]
    SimTestV1(SimTestIotaSystemStateV1),
    #[cfg(msim)]
    SimTestShallowV1(SimTestIotaSystemStateShallowV1),
    #[cfg(msim)]
    SimTestDeepV1(SimTestIotaSystemStateDeepV1),
}

/// This is the fixed type used by genesis.
pub type IotaSystemStateInnerGenesis = IotaSystemStateV1;
pub type IotaValidatorGenesis = ValidatorV1;

impl IotaSystemState {
    /// Always return the version that we will be using for genesis.
    /// Genesis always uses this version regardless of the current version.
    /// Note that since it's possible for the actual genesis of the network to
    /// diverge from the genesis of the latest Rust code, it's important
    /// that we only use this for tooling purposes.
    pub fn into_genesis_version_for_tooling(self) -> IotaSystemStateInnerGenesis {
        match self {
            IotaSystemState::V1(inner) => inner,
            // Types other than V1 should be unreachable
            _ => unreachable!(),
        }
    }

    pub fn version(&self) -> u64 {
        self.system_state_version()
    }
}

pub fn get_iota_system_state_wrapper(
    object_store: &dyn ObjectStore,
) -> Result<IotaSystemStateWrapper, IotaError> {
    let wrapper = object_store
        .get_object(&IOTA_SYSTEM_STATE_OBJECT_ID)?
        // Don't panic here on None because object_store is a generic store.
        .ok_or_else(|| {
            IotaError::IotaSystemStateRead("IotaSystemStateWrapper object not found".to_owned())
        })?;
    let move_object = wrapper.data.try_as_move().ok_or_else(|| {
        IotaError::IotaSystemStateRead(
            "IotaSystemStateWrapper object must be a Move object".to_owned(),
        )
    })?;
    let result = bcs::from_bytes::<IotaSystemStateWrapper>(move_object.contents())
        .map_err(|err| IotaError::IotaSystemStateRead(err.to_string()))?;
    Ok(result)
}

pub fn get_iota_system_state(object_store: &dyn ObjectStore) -> Result<IotaSystemState, IotaError> {
    let wrapper = get_iota_system_state_wrapper(object_store)?;
    let id = wrapper.id.id.bytes;
    match wrapper.version {
        1 => {
            let result: IotaSystemStateV1 =
                get_dynamic_field_from_store(object_store, id, &wrapper.version).map_err(
                    |err| {
                        IotaError::DynamicFieldRead(format!(
                            "Failed to load iota system state inner object with ID {:?} and version {:?}: {:?}",
                            id, wrapper.version, err
                        ))
                    },
                )?;
            Ok(IotaSystemState::V1(result))
        }
        2 => {
            let result: IotaSystemStateV2 =
                get_dynamic_field_from_store(object_store, id, &wrapper.version).map_err(
                    |err| {
                        IotaError::DynamicFieldRead(format!(
                            "Failed to load iota system state inner object with ID {:?} and version {:?}: {:?}",
                            id, wrapper.version, err
                        ))
                    },
                )?;
            Ok(IotaSystemState::V2(result))
        }
        #[cfg(msim)]
        IOTA_SYSTEM_STATE_SIM_TEST_V1 => {
            let result: SimTestIotaSystemStateV1 =
                get_dynamic_field_from_store(object_store, id, &wrapper.version).map_err(
                    |err| {
                        IotaError::DynamicFieldRead(format!(
                            "Failed to load iota system state inner object with ID {:?} and version {:?}: {:?}",
                            id, wrapper.version, err
                        ))
                    },
                )?;
            Ok(IotaSystemState::SimTestV1(result))
        }
        #[cfg(msim)]
        IOTA_SYSTEM_STATE_SIM_TEST_SHALLOW_V1 => {
            let result: SimTestIotaSystemStateShallowV1 =
                get_dynamic_field_from_store(object_store, id, &wrapper.version).map_err(
                    |err| {
                        IotaError::DynamicFieldRead(format!(
                            "Failed to load iota system state inner object with ID {:?} and version {:?}: {:?}",
                            id, wrapper.version, err
                        ))
                    },
                )?;
            Ok(IotaSystemState::SimTestShallowV1(result))
        }
        #[cfg(msim)]
        IOTA_SYSTEM_STATE_SIM_TEST_DEEP_V1 => {
            let result: SimTestIotaSystemStateDeepV1 =
                get_dynamic_field_from_store(object_store, id, &wrapper.version).map_err(
                    |err| {
                        IotaError::DynamicFieldRead(format!(
                            "Failed to load iota system state inner object with ID {:?} and version {:?}: {:?}",
                            id, wrapper.version, err
                        ))
                    },
                )?;
            Ok(IotaSystemState::SimTestDeepV1(result))
        }
        _ => Err(IotaError::IotaSystemStateRead(format!(
            "Unsupported IotaSystemState version: {}",
            wrapper.version
        ))),
    }
}

/// Given a system state type version, and the ID of the table, along with a
/// key, retrieve the dynamic field as a Validator type. We need the version to
/// determine which inner type to use for the Validator type. This is assuming
/// that the validator is stored in the table as Validator type.
pub fn get_validator_from_table<K>(
    object_store: &dyn ObjectStore,
    table_id: ObjectID,
    key: &K,
) -> Result<IotaValidatorSummary, IotaError>
where
    K: MoveTypeTagTrait + Serialize + DeserializeOwned + fmt::Debug,
{
    let field: Validator =
        get_dynamic_field_from_store(object_store, table_id, key).map_err(|err| {
            IotaError::IotaSystemStateRead(format!(
                "Failed to load validator wrapper from table: {:?}",
                err
            ))
        })?;
    let versioned = field.inner;
    let version = versioned.version;
    match version {
        1 => {
            let validator: ValidatorV1 =
                get_dynamic_field_from_store(object_store, versioned.id.id.bytes, &version)
                    .map_err(|err| {
                        IotaError::IotaSystemStateRead(format!(
                            "Failed to load inner validator from the wrapper: {:?}",
                            err
                        ))
                    })?;
            Ok(validator.into_iota_validator_summary())
        }
        #[cfg(msim)]
        IOTA_SYSTEM_STATE_SIM_TEST_V1 => {
            let validator: SimTestValidatorV1 =
                get_dynamic_field_from_store(object_store, versioned.id.id.bytes, &version)
                    .map_err(|err| {
                        IotaError::IotaSystemStateRead(format!(
                            "Failed to load inner validator from the wrapper: {:?}",
                            err
                        ))
                    })?;
            Ok(validator.into_iota_validator_summary())
        }
        #[cfg(msim)]
        IOTA_SYSTEM_STATE_SIM_TEST_DEEP_V1 => {
            let validator: SimTestValidatorDeepV1 =
                get_dynamic_field_from_store(object_store, versioned.id.id.bytes, &version)
                    .map_err(|err| {
                        IotaError::IotaSystemStateRead(format!(
                            "Failed to load inner validator from the wrapper: {:?}",
                            err
                        ))
                    })?;
            Ok(validator.into_iota_validator_summary())
        }
        _ => Err(IotaError::IotaSystemStateRead(format!(
            "Unsupported Validator version: {}",
            version
        ))),
    }
}

pub fn get_validators_from_table_vec<S, ValidatorType>(
    object_store: &S,
    table_id: ObjectID,
    table_size: u64,
) -> Result<Vec<ValidatorType>, IotaError>
where
    S: ObjectStore + ?Sized,
    ValidatorType: Serialize + DeserializeOwned,
{
    let mut validators = vec![];
    for i in 0..table_size {
        let validator: ValidatorType = get_dynamic_field_from_store(&object_store, table_id, &i)
            .map_err(|err| {
                IotaError::IotaSystemStateRead(format!(
                    "Failed to load validator from table: {:?}",
                    err
                ))
            })?;
        validators.push(validator);
    }
    Ok(validators)
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Default)]
pub struct PoolTokenExchangeRate {
    iota_amount: u64,
    pool_token_amount: u64,
}

impl PoolTokenExchangeRate {
    /// Rate of the staking pool, pool token amount : IOTA amount
    pub fn rate(&self) -> f64 {
        if self.iota_amount == 0 {
            1_f64
        } else {
            self.pool_token_amount as f64 / self.iota_amount as f64
        }
    }

    pub fn new_for_testing(iota_amount: u64, pool_token_amount: u64) -> Self {
        Self {
            iota_amount,
            pool_token_amount,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Validator {
    pub inner: Versioned,
}

#[derive(Debug)]
pub struct AdvanceEpochParams {
    pub epoch: u64,
    pub next_protocol_version: ProtocolVersion,
    pub validator_subsidy: u64,
    pub storage_charge: u64,
    pub computation_charge: u64,
    pub computation_charge_burned: u64,
    pub storage_rebate: u64,
    pub non_refundable_storage_fee: u64,
    pub reward_slashing_rate: u64,
    pub epoch_start_timestamp_ms: u64,
    pub max_committee_members_count: u64,
}

#[cfg(msim)]
pub mod advance_epoch_result_injection {
    use std::cell::RefCell;

    use crate::{
        committee::EpochId,
        error::{ExecutionError, ExecutionErrorKind},
    };

    thread_local! {
        /// Override the result of advance_epoch in the range [start, end).
        static OVERRIDE: RefCell<Option<(EpochId, EpochId)>>  = const { RefCell::new(None) };
    }

    /// Override the result of advance_epoch transaction if new epoch is in the
    /// provided range [start, end).
    pub fn set_override(value: Option<(EpochId, EpochId)>) {
        OVERRIDE.with(|o| *o.borrow_mut() = value);
    }

    /// This function is used to modify the result of advance_epoch transaction
    /// for testing. If the override is set, the result will be an execution
    /// error, otherwise the original result will be returned.
    pub fn maybe_modify_result(
        result: Result<(), ExecutionError>,
        current_epoch: EpochId,
    ) -> Result<(), ExecutionError> {
        if let Some((start, end)) = OVERRIDE.with(|o| *o.borrow()) {
            if current_epoch >= start && current_epoch < end {
                return Err::<(), ExecutionError>(ExecutionError::new(
                    ExecutionErrorKind::FunctionNotFound,
                    None,
                ));
            }
        }
        result
    }
}
