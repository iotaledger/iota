// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Extension traits for creating unlock conditions from Stardust types during
//! migration.

use iota_stardust_types::block::address::Address;
use iota_types::base_types::IotaAddress;
// Re-export the canonical types from iota-types
pub use iota_types::stardust::output::unlock_conditions::{
    ExpirationUnlockCondition, StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
};

use super::super::address::stardust_to_iota_address;

/// Extension trait for creating `ExpirationUnlockCondition` from Stardust
/// types.
pub trait ExpirationUnlockConditionExt {
    fn new_from_stardust(
        owner_address: &Address,
        expiration_unlock_condition: &iota_stardust_types::block::output::unlock_condition::ExpirationUnlockCondition,
    ) -> anyhow::Result<ExpirationUnlockCondition>;
}

impl ExpirationUnlockConditionExt for ExpirationUnlockCondition {
    fn new_from_stardust(
        owner_address: &Address,
        expiration_unlock_condition: &iota_stardust_types::block::output::unlock_condition::ExpirationUnlockCondition,
    ) -> anyhow::Result<ExpirationUnlockCondition> {
        let owner = stardust_to_iota_address(owner_address)?;
        let return_address =
            stardust_to_iota_address(expiration_unlock_condition.return_address())?;
        let unix_time = expiration_unlock_condition.timestamp();

        Ok(ExpirationUnlockCondition {
            owner,
            return_address,
            unix_time,
        })
    }
}

/// Extension trait for creating `StorageDepositReturnUnlockCondition` from
/// Stardust types.
pub trait StorageDepositReturnUnlockConditionExt {
    fn try_from_stardust(
        unlock: &iota_stardust_types::block::output::unlock_condition::StorageDepositReturnUnlockCondition,
    ) -> anyhow::Result<StorageDepositReturnUnlockCondition>;
}

impl StorageDepositReturnUnlockConditionExt for StorageDepositReturnUnlockCondition {
    fn try_from_stardust(
        unlock: &iota_stardust_types::block::output::unlock_condition::StorageDepositReturnUnlockCondition,
    ) -> anyhow::Result<StorageDepositReturnUnlockCondition> {
        let return_address: IotaAddress = unlock.return_address().to_string().parse()?;
        let return_amount = unlock.amount();
        Ok(StorageDepositReturnUnlockCondition {
            return_address,
            return_amount,
        })
    }
}

/// Extension trait for creating `TimelockUnlockCondition` from Stardust types.
pub trait TimelockUnlockConditionExt {
    fn from_stardust(
        unlock: &iota_stardust_types::block::output::unlock_condition::TimelockUnlockCondition,
    ) -> TimelockUnlockCondition;
}

impl TimelockUnlockConditionExt for TimelockUnlockCondition {
    fn from_stardust(
        unlock: &iota_stardust_types::block::output::unlock_condition::TimelockUnlockCondition,
    ) -> TimelockUnlockCondition {
        TimelockUnlockCondition {
            unix_time: unlock.timestamp(),
        }
    }
}
