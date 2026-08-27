// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use enum_dispatch::enum_dispatch;
use iota_config::NodeConfig;
use iota_sdk_types::{CheckpointDigest, DenyRuleSet, Version};
use iota_types::{
    deny_list_v1::get_deny_list_obj_initial_shared_version,
    epoch_data::EpochData,
    error::IotaResult,
    iota_system_state::epoch_start_iota_system_state::{
        EpochStartSystemState, EpochStartSystemStateTrait,
    },
    messages_checkpoint::CheckpointTimestamp,
    randomness_state::get_randomness_state_obj_initial_shared_version,
    storage::ObjectStore,
    transaction_deny_rules::{
        get_transaction_deny_rules, get_transaction_deny_rules_obj_initial_shared_version,
    },
};
use serde::{Deserialize, Serialize};

#[enum_dispatch]
pub trait EpochStartConfigTrait {
    fn epoch_digest(&self) -> CheckpointDigest;
    fn epoch_start_state(&self) -> &EpochStartSystemState;
    fn flags(&self) -> &[EpochFlag];
    fn randomness_obj_initial_shared_version(&self) -> Version;
    fn coin_deny_list_obj_initial_shared_version(&self) -> Version;
    /// `None` until the `TransactionDenyRulesCreate` end-of-epoch transaction
    /// has created the object.
    fn transaction_deny_rules_obj_initial_shared_version(&self) -> Option<Version>;
    /// The deny rule state read from the `TransactionDenyRules` object at
    /// epoch start (`None` while the object does not exist). Seeds the
    /// enforcement cache and the mirrored on-chain state for the epoch.
    fn transaction_deny_rules_state(&self) -> Option<&DenyRuleSet>;
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
    _WritebackCacheEnabledDeprecated = 0,
    _DataQuarantineFromBeginningOfEpochDeprecated = 1,

    // Used for `test_epoch_flag_upgrade`.
    #[cfg(msim)]
    DummyFlag = 2,
}

impl EpochFlag {
    pub fn default_flags_for_new_epoch(_config: &NodeConfig) -> Vec<Self> {
        // NodeConfig arg is not currently used, but we keep it here for future
        // flags that might depend on the config.
        Self::default_flags_impl()
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
        Self::default_flags_impl()
    }

    fn default_flags_impl() -> Vec<Self> {
        vec![
            #[cfg(msim)]
            EpochFlag::DummyFlag,
        ]
    }
}

impl fmt::Display for EpochFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Important - implementation should return low cardinality values because this
        // is used as metric key
        match self {
            EpochFlag::_WritebackCacheEnabledDeprecated => {
                write!(f, "WritebackCacheEnabled (DEPRECATED)")
            }
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
#[derive(Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
#[enum_dispatch(EpochStartConfigTrait)]
pub enum EpochStartConfiguration {
    V1(EpochStartConfigurationV1),
    V2(EpochStartConfigurationV2),
    V3(EpochStartConfigurationV3),
}

impl EpochStartConfiguration {
    /// Test-only: stamps the deny-rules object fields onto a V3 config, as
    /// `new` does when the object exists at the epoch boundary.
    #[cfg(test)]
    pub fn set_transaction_deny_rules_for_testing(
        &mut self,
        initial_shared_version: Version,
        state: DenyRuleSet,
    ) {
        match self {
            Self::V3(config) => {
                config.transaction_deny_rules_obj_initial_shared_version =
                    Some(initial_shared_version);
                config.transaction_deny_rules_state = Some(state);
            }
            _ => panic!("only a V3 config carries deny-rules fields"),
        }
    }

    pub fn new(
        system_state: EpochStartSystemState,
        epoch_digest: CheckpointDigest,
        object_store: &dyn ObjectStore,
        initial_epoch_flags: Vec<EpochFlag>,
    ) -> IotaResult<Self> {
        let randomness_obj_initial_shared_version =
            get_randomness_state_obj_initial_shared_version(object_store)?;
        let coin_deny_list_obj_initial_shared_version =
            get_deny_list_obj_initial_shared_version(object_store);
        let transaction_deny_rules_obj_initial_shared_version =
            get_transaction_deny_rules_obj_initial_shared_version(object_store)?;
        let transaction_deny_rules_state = get_transaction_deny_rules(object_store)?;
        debug_assert_eq!(
            transaction_deny_rules_obj_initial_shared_version.is_some(),
            transaction_deny_rules_state.is_some()
        );
        Ok(Self::V3(EpochStartConfigurationV3 {
            system_state,
            epoch_digest,
            flags: initial_epoch_flags,
            randomness_obj_initial_shared_version,
            coin_deny_list_obj_initial_shared_version,
            transaction_deny_rules_obj_initial_shared_version,
            transaction_deny_rules_state,
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
            Self::V3(config) => Self::V3(EpochStartConfigurationV3 {
                system_state: config.system_state.new_at_next_epoch_for_testing(),
                epoch_digest: config.epoch_digest,
                flags: config.flags.clone(),
                randomness_obj_initial_shared_version: config.randomness_obj_initial_shared_version,
                coin_deny_list_obj_initial_shared_version: config
                    .coin_deny_list_obj_initial_shared_version,
                transaction_deny_rules_obj_initial_shared_version: config
                    .transaction_deny_rules_obj_initial_shared_version,
                transaction_deny_rules_state: config.transaction_deny_rules_state.clone(),
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

#[derive(Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct EpochStartConfigurationV1 {
    system_state: EpochStartSystemState,
    epoch_digest: CheckpointDigest,
    flags: Vec<EpochFlag>,
    /// Do the state objects exist at the beginning of the epoch?
    authenticator_obj_initial_shared_version: Option<Version>,
    randomness_obj_initial_shared_version: Version,
    coin_deny_list_obj_initial_shared_version: Version,
    bridge_obj_initial_shared_version: Option<Version>,
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

    fn randomness_obj_initial_shared_version(&self) -> Version {
        self.randomness_obj_initial_shared_version
    }

    fn coin_deny_list_obj_initial_shared_version(&self) -> Version {
        self.coin_deny_list_obj_initial_shared_version
    }

    fn transaction_deny_rules_obj_initial_shared_version(&self) -> Option<Version> {
        None
    }

    fn transaction_deny_rules_state(&self) -> Option<&DenyRuleSet> {
        None
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct EpochStartConfigurationV2 {
    system_state: EpochStartSystemState,
    epoch_digest: CheckpointDigest,
    flags: Vec<EpochFlag>,
    /// Do the state objects exist at the beginning of the epoch?
    authenticator_obj_initial_shared_version: Option<Version>,
    randomness_obj_initial_shared_version: Version,
    coin_deny_list_obj_initial_shared_version: Version,
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

    fn randomness_obj_initial_shared_version(&self) -> Version {
        self.randomness_obj_initial_shared_version
    }

    fn coin_deny_list_obj_initial_shared_version(&self) -> Version {
        self.coin_deny_list_obj_initial_shared_version
    }

    fn transaction_deny_rules_obj_initial_shared_version(&self) -> Option<Version> {
        None
    }

    fn transaction_deny_rules_state(&self) -> Option<&DenyRuleSet> {
        None
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct EpochStartConfigurationV3 {
    system_state: EpochStartSystemState,
    epoch_digest: CheckpointDigest,
    flags: Vec<EpochFlag>,
    randomness_obj_initial_shared_version: Version,
    coin_deny_list_obj_initial_shared_version: Version,
    transaction_deny_rules_obj_initial_shared_version: Option<Version>,
    /// Present exactly when the initial shared version is: both come from the
    /// same object at epoch start.
    transaction_deny_rules_state: Option<DenyRuleSet>,
}

impl EpochStartConfigTrait for EpochStartConfigurationV3 {
    fn epoch_digest(&self) -> CheckpointDigest {
        self.epoch_digest
    }

    fn epoch_start_state(&self) -> &EpochStartSystemState {
        &self.system_state
    }

    fn flags(&self) -> &[EpochFlag] {
        &self.flags
    }

    fn randomness_obj_initial_shared_version(&self) -> Version {
        self.randomness_obj_initial_shared_version
    }

    fn coin_deny_list_obj_initial_shared_version(&self) -> Version {
        self.coin_deny_list_obj_initial_shared_version
    }

    fn transaction_deny_rules_obj_initial_shared_version(&self) -> Option<Version> {
        self.transaction_deny_rules_obj_initial_shared_version
    }

    fn transaction_deny_rules_state(&self) -> Option<&DenyRuleSet> {
        self.transaction_deny_rules_state.as_ref()
    }
}
