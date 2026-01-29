// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use enum_dispatch::enum_dispatch;
use iota_config::{ExecutionCacheType, NodeConfig};
use iota_types::{
    authenticator_state::get_authenticator_state_obj_initial_shared_version,
    base_types::SequenceNumber,
    deny_list_v1::get_deny_list_obj_initial_shared_version,
    epoch_data::EpochData,
    error::IotaResult,
    iota_system_state::epoch_start_iota_system_state::{
        EpochStartSystemState, EpochStartSystemStateTrait,
    },
    messages_checkpoint::{CheckpointDigest, CheckpointTimestamp},
    randomness_state::get_randomness_state_obj_initial_shared_version,
    storage::ObjectStore,
};
use serde::{Deserialize, Serialize};

#[enum_dispatch]
pub trait EpochStartConfigTrait {
    fn epoch_digest(&self) -> CheckpointDigest;
    fn epoch_start_state(&self) -> &EpochStartSystemState;
    fn flags(&self) -> &[EpochFlag];
    fn authenticator_obj_initial_shared_version(&self) -> Option<SequenceNumber>;
    fn randomness_obj_initial_shared_version(&self) -> SequenceNumber;
    fn coin_deny_list_obj_initial_shared_version(&self) -> SequenceNumber;

    fn execution_cache_type(&self) -> ExecutionCacheType {
        if self.flags().contains(&EpochFlag::WritebackCacheEnabled) {
            ExecutionCacheType::WritebackCache
        } else {
            ExecutionCacheType::PassthroughCache
        }
    }
}

// IMPORTANT: Assign explicit values to each variant to ensure that the values
// are stable. When cherry-picking changes from one branch to another, the value
// of variants must never change.
//
// Unlikely: If you cherry pick a change from one branch to another, and there
// is a collision in the value of some variant, the branch which has been
// released should take precedence. In this case, the picked-from branch is
// inconsistent with the released branch, and must be fixed.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub enum EpochFlag {
    // The deprecated flags have all been in production for long enough that
    // we have deleted the old code paths they were guarding.
    // We retain them here in order not to break deserialization.
    _DataQuarantineFromBeginningOfEpochDeprecated = 1,

    // When switching between different cache types mid-epoch, partial checkpoint transactions
    // might already be on disk. During lock initialization, we check if there is any existing
    // lock or not, depending on the used implementation. That's why we should not switch
    // mid-epoch.
    WritebackCacheEnabled = 0,

    // Used for `test_epoch_flag_upgrade`.
    #[cfg(msim)]
    DummyFlag = 2,
}

impl EpochFlag {
    pub fn default_flags_for_new_epoch(config: &NodeConfig) -> Vec<Self> {
        Self::default_flags_impl(config.execution_cache)
    }

    // Return flags that are mandatory for the current version of the code. This is
    // used so that `test_epoch_flag_upgrade` can still work correctly even when
    // there are no optional flags.
    pub fn mandatory_flags() -> Vec<Self> {
        vec![]
    }

    /// For situations in which there is no config available (e.g. setting up a
    /// downloaded snapshot).
    pub fn default_for_no_config() -> Vec<Self> {
        Self::default_flags_impl(Default::default())
    }

    fn default_flags_impl(cache_type: ExecutionCacheType) -> Vec<Self> {
        let mut new_flags = vec![
            #[cfg(msim)]
            EpochFlag::DummyFlag,
        ];

        // Load cache type from env
        if matches!(cache_type.cache_type(), ExecutionCacheType::WritebackCache) {
            new_flags.push(EpochFlag::WritebackCacheEnabled);
        }

        new_flags
    }
}

impl fmt::Display for EpochFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Important - implementation should return low cardinality values because this
        // is used as metric key
        match self {
            EpochFlag::WritebackCacheEnabled => write!(f, "WritebackCacheEnabled"),
            EpochFlag::_DataQuarantineFromBeginningOfEpochDeprecated => {
                write!(f, "DataQuarantineFromBeginningOfEpoch (DEPRECATED)")
            }
            #[cfg(msim)]
            EpochFlag::DummyFlag => {
                write!(f, "DummyFlag")
            }
        }
    }
}

/// Parameters of the epoch fixed at epoch start.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[enum_dispatch(EpochStartConfigTrait)]
pub enum EpochStartConfiguration {
    V1(EpochStartConfigurationV1),
    V2(EpochStartConfigurationV2),
}

impl EpochStartConfiguration {
    pub fn new(
        system_state: EpochStartSystemState,
        epoch_digest: CheckpointDigest,
        object_store: &dyn ObjectStore,
        initial_epoch_flags: Vec<EpochFlag>,
    ) -> IotaResult<Self> {
        let authenticator_obj_initial_shared_version =
            get_authenticator_state_obj_initial_shared_version(object_store)?;
        let randomness_obj_initial_shared_version =
            get_randomness_state_obj_initial_shared_version(object_store)?;
        let coin_deny_list_obj_initial_shared_version =
            get_deny_list_obj_initial_shared_version(object_store);
        Ok(Self::V2(EpochStartConfigurationV2 {
            system_state,
            epoch_digest,
            flags: initial_epoch_flags,
            authenticator_obj_initial_shared_version,
            randomness_obj_initial_shared_version,
            coin_deny_list_obj_initial_shared_version,
        }))
    }

    #[expect(unreachable_patterns)]
    pub fn new_at_next_epoch_for_testing(&self) -> Self {
        // We only need to implement this function for the latest version.
        // When a new version is introduced, this function should be updated.
        match self {
            Self::V1(config) => Self::V1(EpochStartConfigurationV1 {
                system_state: config.system_state.new_at_next_epoch_for_testing(),
                epoch_digest: config.epoch_digest,
                flags: config.flags.clone(),
                authenticator_obj_initial_shared_version: config
                    .authenticator_obj_initial_shared_version,
                randomness_obj_initial_shared_version: config.randomness_obj_initial_shared_version,
                coin_deny_list_obj_initial_shared_version: config
                    .coin_deny_list_obj_initial_shared_version,
                bridge_obj_initial_shared_version: config.bridge_obj_initial_shared_version,
                bridge_committee_initiated: config.bridge_committee_initiated,
            }),
            Self::V2(config) => Self::V2(EpochStartConfigurationV2 {
                system_state: config.system_state.new_at_next_epoch_for_testing(),
                epoch_digest: config.epoch_digest,
                flags: config.flags.clone(),
                authenticator_obj_initial_shared_version: config
                    .authenticator_obj_initial_shared_version,
                randomness_obj_initial_shared_version: config.randomness_obj_initial_shared_version,
                coin_deny_list_obj_initial_shared_version: config
                    .coin_deny_list_obj_initial_shared_version,
            }),
            _ => panic!(
                "This function is only implemented for the latest version of EpochStartConfiguration"
            ),
        }
    }

    pub fn epoch_data(&self) -> EpochData {
        EpochData::new(
            self.epoch_start_state().epoch(),
            self.epoch_start_state().epoch_start_timestamp_ms(),
            self.epoch_digest(),
        )
    }

    pub fn epoch_start_timestamp_ms(&self) -> CheckpointTimestamp {
        self.epoch_start_state().epoch_start_timestamp_ms()
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct EpochStartConfigurationV1 {
    system_state: EpochStartSystemState,
    epoch_digest: CheckpointDigest,
    flags: Vec<EpochFlag>,
    /// Do the state objects exist at the beginning of the epoch?
    authenticator_obj_initial_shared_version: Option<SequenceNumber>,
    randomness_obj_initial_shared_version: SequenceNumber,
    coin_deny_list_obj_initial_shared_version: SequenceNumber,
    bridge_obj_initial_shared_version: Option<SequenceNumber>,
    bridge_committee_initiated: bool,
}

impl EpochStartConfigTrait for EpochStartConfigurationV1 {
    fn epoch_digest(&self) -> CheckpointDigest {
        self.epoch_digest
    }

    fn epoch_start_state(&self) -> &EpochStartSystemState {
        &self.system_state
    }

    fn flags(&self) -> &[EpochFlag] {
        &self.flags
    }

    fn authenticator_obj_initial_shared_version(&self) -> Option<SequenceNumber> {
        self.authenticator_obj_initial_shared_version
    }

    fn randomness_obj_initial_shared_version(&self) -> SequenceNumber {
        self.randomness_obj_initial_shared_version
    }

    fn coin_deny_list_obj_initial_shared_version(&self) -> SequenceNumber {
        self.coin_deny_list_obj_initial_shared_version
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct EpochStartConfigurationV2 {
    system_state: EpochStartSystemState,
    epoch_digest: CheckpointDigest,
    flags: Vec<EpochFlag>,
    /// Do the state objects exist at the beginning of the epoch?
    authenticator_obj_initial_shared_version: Option<SequenceNumber>,
    randomness_obj_initial_shared_version: SequenceNumber,
    coin_deny_list_obj_initial_shared_version: SequenceNumber,
}

impl EpochStartConfigTrait for EpochStartConfigurationV2 {
    fn epoch_digest(&self) -> CheckpointDigest {
        self.epoch_digest
    }

    fn epoch_start_state(&self) -> &EpochStartSystemState {
        &self.system_state
    }

    fn flags(&self) -> &[EpochFlag] {
        &self.flags
    }

    fn authenticator_obj_initial_shared_version(&self) -> Option<SequenceNumber> {
        self.authenticator_obj_initial_shared_version
    }

    fn randomness_obj_initial_shared_version(&self) -> SequenceNumber {
        self.randomness_obj_initial_shared_version
    }

    fn coin_deny_list_obj_initial_shared_version(&self) -> SequenceNumber {
        self.coin_deny_list_obj_initial_shared_version
    }
}
