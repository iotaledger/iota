// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Logic and types to account for stake delegation during genesis.
use iota_config::genesis::{
    TimelockAllocation, TokenAllocation, TokenDistributionSchedule,
    TokenDistributionScheduleBuilder,
};
use iota_types::{
    base_types::{IotaAddress, ObjectRef},
    object::Object,
};

use crate::{
    stardust::{migration::MigrationObjects, types::coin_kind::get_gas_balance_maybe},
    validator_info::GenesisValidatorInfo,
};

#[derive(Default, Debug, Clone)]
pub struct GenesisStake {
    timelock_allocations: Vec<TimelockAllocation>,
    token_allocation: Vec<TokenAllocation>,
    gas_coins_to_burn: Vec<ObjectRef>,
}

impl GenesisStake {
    /// Get a reference of the inner timelock allocations.
    pub fn timelock_allocations(&self) -> &Vec<TimelockAllocation> {
        &self.timelock_allocations
    }

    /// Take the inner gas-coin objects that must be burned.
    ///
    /// This follows the semantics of [`std::mem::take`].
    pub fn take_gas_coins_to_burn(&mut self) -> Vec<ObjectRef> {
        std::mem::take(&mut self.gas_coins_to_burn)
    }

    pub fn is_empty(&self) -> bool {
        self.timelock_allocations.is_empty()
            && self.token_allocation.is_empty()
            && self.gas_coins_to_burn.is_empty()
    }

    /// Calculate the total amount of token allocations.
    pub fn sum_token_allocation(&self) -> u64 {
        self.token_allocation
            .iter()
            .map(|allocation| allocation.amount_micros)
            .sum()
    }

    /// Create a new valid [`TokenDistributionSchedule`] from the
    /// inner token allocations.
    pub fn to_token_distribution_schedule(&self) -> TokenDistributionSchedule {
        let mut builder = TokenDistributionScheduleBuilder::new();
        for allocation in self.token_allocation.clone() {
            builder.add_allocation(allocation);
        }
        builder.build()
    }

    /// Extend a vanilla [`TokenDistributionSchedule`] with the
    /// inner token allocations.
    ///
    /// ## Panic
    ///
    /// The method panics if the resulting schedule is invalid.
    pub fn extend_vanilla_token_distribution_schedule(
        &self,
        mut vanilla_schedule: TokenDistributionSchedule,
    ) -> TokenDistributionSchedule {
        vanilla_schedule
            .allocations
            .extend(self.token_allocation.clone());
        vanilla_schedule.stake_subsidy_fund_nanos -= self.sum_token_allocation();
        vanilla_schedule.validate();
        vanilla_schedule
    }
}

/// The objects picked for token allocation during genesis
#[derive(Default, Debug, Clone)]
pub struct AllocationObjects {
    inner: Vec<ObjectRef>,
    /// The total amount of nanos to be allocated from this
    /// collection of objects.
    amount_nanos: u64,
    /// The surplus amount that is not be allocated from this
    /// collection of objects.
    surplus_nanos: u64,
}

/// Pick gas-coin like objects from a pool to cover
/// the `target_amount`.
///
/// This does not split any surplus balance, but delegates
/// splitting to the caller.
pub fn pick_objects_for_allocation<'obj>(
    pool: &mut impl Iterator<Item = &'obj Object>,
    target_amount: u64,
) -> AllocationObjects {
    let mut amount = 0;
    let objects = pool
        .by_ref()
        .map_while(|timelock| {
            if amount < target_amount {
                amount += get_gas_balance_maybe(timelock)?.value();
                Some(timelock.compute_object_reference())
            } else {
                None
            }
        })
        .collect();
    AllocationObjects {
        inner: objects,
        amount_nanos: amount.min(target_amount),
        surplus_nanos: amount.saturating_sub(target_amount),
    }
}

/// Create the necessary allocations to cover `amount_nanos` for all
/// `validators`.
///
/// This function iterates in turn over [`TimeLock`] and
/// [`GasCoin`][iota_types::gas_coin::GasCoin] objects created
/// during stardust migration that are owned by the `delegator`.
pub fn delegate_genesis_stake(
    validators: &[GenesisValidatorInfo],
    delegator: IotaAddress,
    migration_objects: &MigrationObjects,
    amount_nanos: u64,
) -> anyhow::Result<GenesisStake> {
    let timelocks_pool = migration_objects.get_sorted_timelocks_by_owner(delegator);
    let gas_coins_pool = migration_objects.get_gas_coins_by_owner(delegator);
    if timelocks_pool.is_none() && gas_coins_pool.is_none() {
        anyhow::bail!("no timelocks or gas-coin objects found for delegator {delegator:?}");
    }
    let mut timelocks_pool = timelocks_pool.unwrap_or_default().into_iter();
    let mut gas_coins_pool = gas_coins_pool.unwrap_or_default().into_iter();
    let mut genesis_stake = GenesisStake::default();

    // For each validator we try to fill their allocation up to
    // total_amount_to_stake_per_validator
    for validator in validators {
        let target_stake = amount_nanos;

        // Start filling allocations with timelocks
        let timelock_objects = pick_objects_for_allocation(&mut timelocks_pool, target_stake);
        // TODO: This is not an optimal solution because the last timelock
        // might have a surplus amount, which cannot be used without splitting.

        if !timelock_objects.inner.is_empty() {
            // For timelocks we need cannot add the stake directly, so we create
            // `TimelockAllocation` objects
            genesis_stake.timelock_allocations.push(TimelockAllocation {
                recipient_address: delegator,
                amount_nanos: timelock_objects.amount_nanos,
                surplus_nanos: timelock_objects.surplus_nanos,
                timelock_objects: timelock_objects.inner,
                staked_with_validator: validator.info.iota_address(),
            })
        }

        // Then cover any remaining target stake with gas coins
        let remainder_target_stake = target_stake - timelock_objects.amount_nanos;

        let gas_coin_objects =
            pick_objects_for_allocation(&mut gas_coins_pool, remainder_target_stake);
        genesis_stake.gas_coins_to_burn = gas_coin_objects.inner;

        // TODO: also here, this is not an optimal solution because the last gas object
        // might have a surplus amount, which cannot be used without splitting.

        if gas_coin_objects.amount_nanos < remainder_target_stake {
            return Err(anyhow::anyhow!(
                "Not enough funds for delegator {:?}",
                delegator
            ));
        } else if gas_coin_objects.amount_nanos > 0 {
            genesis_stake.token_allocation.push(TokenAllocation {
                recipient_address: delegator,
                amount_micros: gas_coin_objects.amount_nanos,
                staked_with_validator: Some(validator.info.iota_address()),
            });
            if gas_coin_objects.surplus_nanos > 0 {
                // This essentially schedules returning any surplus amount
                // from the last coin in `gas_coin_objects` to the delegator
                // as a new coin
                genesis_stake.token_allocation.push(TokenAllocation {
                    recipient_address: delegator,
                    amount_micros: gas_coin_objects.surplus_nanos,
                    staked_with_validator: None,
                });
            }
        }
    }
    Ok(genesis_stake)
}
