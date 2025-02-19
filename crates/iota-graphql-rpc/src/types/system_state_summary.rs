// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use async_graphql::*;
use iota_types::{
    base_types::{IotaAddress as NativeIotaAddress, ObjectID},
    iota_system_state::iota_system_state_summary::{
        IotaSystemStateSummary as NativeSystemStateSummary, IotaValidatorSummary,
    },
};

use super::validator_set::ValidatorSet;
use crate::types::{
    address::Address, big_int::BigInt, gas::GasCostSummary, iota_address::IotaAddress,
    safe_mode::SafeMode, storage_fund::StorageFund, system_parameters::SystemParameters,
    uint53::UInt53, validator::Validator,
};

#[derive(Clone, Debug)]
pub(crate) struct SystemStateSummary {
    pub native: NativeSystemStateSummary,
}

/// Data related to validators.
///
/// A subset of the information wrapped in [`NativeSystemStateSummary`].
pub(crate) struct NativeStateValidatorInfo {
    pub active_validators: Vec<IotaValidatorSummary>,
    pub at_risk_validators: Vec<(NativeIotaAddress, u64)>,
    pub validator_report_records: Vec<(NativeIotaAddress, Vec<NativeIotaAddress>)>,
    pub pending_removals: Vec<u64>,
    pub pending_active_validators_id: ObjectID,
    pub pending_active_validators_size: u64,
    pub staking_pool_mappings_id: ObjectID,
    pub staking_pool_mappings_size: u64,
    pub inactive_pools_id: ObjectID,
    pub inactive_pools_size: u64,
    pub validator_candidates_id: ObjectID,
    pub validator_candidates_size: u64,
}

impl NativeStateValidatorInfo {
    /// Transform inner data into a sequence of [`Validator`]s.
    ///
    /// `checkpoint_viewed_at` represents the checkpoint sequence number at
    /// which the set of `IotaValidatorSummary` was queried for. Each
    /// `Validator` will inherit this checkpoint, so that when viewing the
    /// `Validator`'s state, it will be as if it was read at the same
    /// checkpoint.
    pub fn to_validators_mut(
        &mut self,
        checkpoint_viewed_at: u64,
        requested_for_epoch: u64,
    ) -> Vec<Validator> {
        let active = std::mem::take(&mut self.active_validators);
        let at_risk = BTreeMap::from_iter(self.at_risk_validators.drain(..));
        let reports = BTreeMap::from_iter(self.validator_report_records.drain(..));

        active
            .into_iter()
            .map(move |validator_summary| {
                let at_risk = at_risk.get(&validator_summary.iota_address).copied();
                let report_records = reports.get(&validator_summary.iota_address).map(|addrs| {
                    addrs
                        .iter()
                        .cloned()
                        .map(|a| Address {
                            address: IotaAddress::from(a),
                            checkpoint_viewed_at,
                        })
                        .collect()
                });

                Validator {
                    validator_summary,
                    at_risk,
                    report_records,
                    checkpoint_viewed_at,
                    requested_for_epoch,
                }
            })
            .collect()
    }

    pub fn into_validator_set(
        mut self,
        total_stake: u64,
        checkpoint_viewed_at: u64,
        requested_for_epoch: u64,
    ) -> ValidatorSet {
        let active_validators = self.to_validators_mut(checkpoint_viewed_at, requested_for_epoch);

        ValidatorSet {
            total_stake: Some(BigInt::from(total_stake)),
            active_validators: Some(active_validators),
            pending_removals: Some(self.pending_removals),
            pending_active_validators_id: Some(self.pending_active_validators_id.into()),
            pending_active_validators_size: Some(self.pending_active_validators_size),
            staking_pool_mappings_id: Some(self.staking_pool_mappings_id.into()),
            staking_pool_mappings_size: Some(self.staking_pool_mappings_size),
            inactive_pools_id: Some(self.inactive_pools_id.into()),
            inactive_pools_size: Some(self.inactive_pools_size),
            validator_candidates_id: Some(self.validator_candidates_id.into()),
            validator_candidates_size: Some(self.validator_candidates_size),
            checkpoint_viewed_at,
        }
    }
}

impl From<NativeSystemStateSummary> for NativeStateValidatorInfo {
    fn from(summary: NativeSystemStateSummary) -> Self {
        let (
            active_validators,
            at_risk_validators,
            validator_report_records,
            pending_removals,
            pending_active_validators_id,
            pending_active_validators_size,
            staking_pool_mappings_id,
            staking_pool_mappings_size,
            inactive_pools_id,
            inactive_pools_size,
            validator_candidates_id,
            validator_candidates_size,
        ) = match summary {
            NativeSystemStateSummary::V1(inner) => (
                inner.active_validators,
                inner.at_risk_validators,
                inner.validator_report_records,
                inner.pending_removals,
                inner.pending_active_validators_id,
                inner.pending_active_validators_size,
                inner.staking_pool_mappings_id,
                inner.staking_pool_mappings_size,
                inner.inactive_pools_id,
                inner.inactive_pools_size,
                inner.validator_candidates_id,
                inner.validator_candidates_size,
            ),
            NativeSystemStateSummary::V2(inner) => (
                inner.active_validators,
                inner.at_risk_validators,
                inner.validator_report_records,
                inner.pending_removals,
                inner.pending_active_validators_id,
                inner.pending_active_validators_size,
                inner.staking_pool_mappings_id,
                inner.staking_pool_mappings_size,
                inner.inactive_pools_id,
                inner.inactive_pools_size,
                inner.validator_candidates_id,
                inner.validator_candidates_size,
            ),
            _ => unimplemented!(),
        };
        Self {
            active_validators,
            at_risk_validators,
            validator_report_records,
            pending_removals,
            pending_active_validators_id,
            pending_active_validators_size,
            staking_pool_mappings_id,
            staking_pool_mappings_size,
            inactive_pools_id,
            inactive_pools_size,
            validator_candidates_id,
            validator_candidates_size,
        }
    }
}

macro_rules! state_summary_take {
    ($enum:expr, $field:ident) => {{
        match $enum {
            NativeSystemStateSummary::V1(inner) => inner.$field,
            NativeSystemStateSummary::V2(inner) => inner.$field,
            _ => unimplemented!(),
        }
    }};
}

/// Access system-state data of relevance to this library.
pub(crate) trait SystemStateSummaryView {
    fn epoch(&self) -> u64;

    fn storage_fund_total_object_storage_rebates(&self) -> u64;

    fn storage_fund_non_refundable_balance(&self) -> u64;

    fn safe_mode(&self) -> bool;

    fn safe_mode_computation_charges(&self) -> u64;

    fn safe_mode_computation_charges_burned(&self) -> u64;

    fn safe_mode_storage_charges(&self) -> u64;

    fn safe_mode_storage_rebates(&self) -> u64;

    fn safe_mode_non_refundable_storage_fee(&self) -> u64;

    fn system_state_version(&self) -> u64;

    fn iota_total_supply(&self) -> u64;

    fn iota_treasury_cap_id(&self) -> ObjectID;

    fn epoch_duration_ms(&self) -> u64;

    fn max_validator_count(&self) -> u64;

    fn min_validator_joining_stake(&self) -> u64;

    fn validator_low_stake_threshold(&self) -> u64;

    fn validator_very_low_stake_threshold(&self) -> u64;

    fn validator_low_stake_grace_period(&self) -> u64;
}

impl SystemStateSummaryView for NativeSystemStateSummary {
    fn epoch(&self) -> u64 {
        state_summary_take!(self, epoch)
    }

    fn storage_fund_total_object_storage_rebates(&self) -> u64 {
        state_summary_take!(self, storage_fund_total_object_storage_rebates)
    }

    fn storage_fund_non_refundable_balance(&self) -> u64 {
        state_summary_take!(self, storage_fund_non_refundable_balance)
    }

    fn safe_mode(&self) -> bool {
        state_summary_take!(self, safe_mode)
    }

    fn safe_mode_computation_charges(&self) -> u64 {
        match &self {
            NativeSystemStateSummary::V1(v1) => v1.safe_mode_computation_rewards,
            NativeSystemStateSummary::V2(v2) => v2.safe_mode_computation_charges,
            _ => unimplemented!(),
        }
    }

    fn safe_mode_computation_charges_burned(&self) -> u64 {
        match &self {
            NativeSystemStateSummary::V1(v1) => v1.safe_mode_computation_rewards,
            NativeSystemStateSummary::V2(v2) => v2.safe_mode_computation_charges_burned,
            _ => unimplemented!(),
        }
    }

    fn safe_mode_storage_charges(&self) -> u64 {
        state_summary_take!(&self, safe_mode_storage_charges)
    }

    fn safe_mode_storage_rebates(&self) -> u64 {
        state_summary_take!(&self, safe_mode_storage_rebates)
    }

    fn safe_mode_non_refundable_storage_fee(&self) -> u64 {
        state_summary_take!(&self, safe_mode_non_refundable_storage_fee)
    }

    fn system_state_version(&self) -> u64 {
        state_summary_take!(&self, system_state_version)
    }

    fn iota_total_supply(&self) -> u64 {
        state_summary_take!(&self, iota_total_supply)
    }

    fn iota_treasury_cap_id(&self) -> ObjectID {
        state_summary_take!(&self, iota_treasury_cap_id)
    }

    fn epoch_duration_ms(&self) -> u64 {
        state_summary_take!(&self, epoch_duration_ms)
    }

    fn max_validator_count(&self) -> u64 {
        state_summary_take!(&self, max_validator_count)
    }

    fn min_validator_joining_stake(&self) -> u64 {
        state_summary_take!(&self, min_validator_joining_stake)
    }

    fn validator_low_stake_threshold(&self) -> u64 {
        state_summary_take!(&self, validator_low_stake_threshold)
    }

    fn validator_very_low_stake_threshold(&self) -> u64 {
        state_summary_take!(&self, validator_very_low_stake_threshold)
    }

    fn validator_low_stake_grace_period(&self) -> u64 {
        state_summary_take!(&self, validator_low_stake_grace_period)
    }
}

/// Aspects that affect the running of the system that are managed by the
/// validators either directly, or through system transactions.
#[Object]
impl SystemStateSummary {
    /// IOTA set aside to account for objects stored on-chain, at the start of
    /// the epoch. This is also used for storage rebates.
    async fn storage_fund(&self) -> Option<StorageFund> {
        Some(StorageFund {
            total_object_storage_rebates: Some(BigInt::from(
                self.native.storage_fund_total_object_storage_rebates(),
            )),
            non_refundable_balance: Some(BigInt::from(
                self.native.storage_fund_non_refundable_balance(),
            )),
        })
    }

    /// Information about whether this epoch was started in safe mode, which
    /// happens if the full epoch change logic fails for some reason.
    async fn safe_mode(&self) -> Option<SafeMode> {
        Some(SafeMode {
            enabled: Some(self.native.safe_mode()),
            gas_summary: Some(GasCostSummary {
                computation_cost: self.native.safe_mode_computation_charges(),
                computation_cost_burned: self.native.safe_mode_computation_charges_burned(),
                storage_cost: self.native.safe_mode_storage_charges(),
                storage_rebate: self.native.safe_mode_storage_rebates(),
                non_refundable_storage_fee: self.native.safe_mode_non_refundable_storage_fee(),
            }),
        })
    }

    /// The value of the `version` field of `0x5`, the
    /// `0x3::iota::IotaSystemState` object.  This version changes whenever
    /// the fields contained in the system state object (held in a dynamic
    /// field attached to `0x5`) change.
    async fn system_state_version(&self) -> Option<UInt53> {
        Some(self.native.system_state_version().into())
    }

    /// The total IOTA supply.
    async fn iota_total_supply(&self) -> Option<u64> {
        Some(self.native.iota_total_supply())
    }

    /// The treasury-cap id.
    async fn iota_treasury_cap_id(&self) -> Option<IotaAddress> {
        Some(self.native.iota_treasury_cap_id().into())
    }

    /// Details of the system that are decided during genesis.
    async fn system_parameters(&self) -> Option<SystemParameters> {
        Some(SystemParameters {
            duration_ms: Some(BigInt::from(self.native.epoch_duration_ms())),
            // TODO min validator count can be extracted, but it requires some JSON RPC changes,
            // so we decided to wait on it for now.
            min_validator_count: None,
            max_validator_count: Some(self.native.max_validator_count()),
            min_validator_joining_stake: Some(BigInt::from(
                self.native.min_validator_joining_stake(),
            )),
            validator_low_stake_threshold: Some(BigInt::from(
                self.native.validator_low_stake_threshold(),
            )),
            validator_very_low_stake_threshold: Some(BigInt::from(
                self.native.validator_very_low_stake_threshold(),
            )),
            validator_low_stake_grace_period: Some(BigInt::from(
                self.native.validator_low_stake_grace_period(),
            )),
        })
    }
}
