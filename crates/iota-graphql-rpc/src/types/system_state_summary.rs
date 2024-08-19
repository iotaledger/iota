// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary as NativeSystemStateSummary;

use super::{
    big_int::BigInt, gas::GasCostSummary, safe_mode::SafeMode, storage_fund::StorageFund,
    system_parameters::SystemParameters,
};

#[derive(Clone, Debug)]
pub(crate) struct SystemStateSummary {
    pub native: NativeSystemStateSummary,
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
                self.native.storage_fund_total_object_storage_rebates,
            )),
            non_refundable_balance: Some(BigInt::from(
                self.native.storage_fund_non_refundable_balance,
            )),
        })
    }

    /// Information about whether this epoch was started in safe mode, which
    /// happens if the full epoch change logic fails for some reason.
    async fn safe_mode(&self) -> Option<SafeMode> {
        Some(SafeMode {
            enabled: Some(self.native.safe_mode),
            gas_summary: Some(GasCostSummary {
                computation_cost: self.native.safe_mode_computation_rewards,
                storage_cost: self.native.safe_mode_storage_charges,
                storage_rebate: self.native.safe_mode_storage_rebates,
                non_refundable_storage_fee: self.native.safe_mode_non_refundable_storage_fee,
            }),
        })
    }

    /// The value of the `version` field of `0x5`, the
    /// `0x3::iota::IotaSystemState` object.  This version changes whenever
    /// the fields contained in the system state object (held in a dynamic
    /// field attached to `0x5`) change.
    async fn system_state_version(&self) -> Option<u64> {
        Some(self.native.system_state_version)
    }

    /// The total IOTA supply.
    async fn iota_total_supply(&self) -> Option<u64> {
        Some(self.native.iota_total_supply)
    }

    /// Details of the system that are decided during genesis.
    async fn system_parameters(&self) -> Option<SystemParameters> {
        Some(SystemParameters {
            duration_ms: Some(BigInt::from(self.native.epoch_duration_ms)),
            // TODO min validator count can be extracted, but it requires some JSON RPC changes,
            // so we decided to wait on it for now.
            min_validator_count: None,
            max_validator_count: Some(self.native.max_validator_count),
            min_validator_joining_stake: Some(BigInt::from(
                self.native.min_validator_joining_stake,
            )),
            validator_low_stake_threshold: Some(BigInt::from(
                self.native.validator_low_stake_threshold,
            )),
            validator_very_low_stake_threshold: Some(BigInt::from(
                self.native.validator_very_low_stake_threshold,
            )),
            validator_low_stake_grace_period: Some(BigInt::from(
                self.native.validator_low_stake_grace_period,
            )),
        })
    }
}
