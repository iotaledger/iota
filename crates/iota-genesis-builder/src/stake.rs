// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Logic and types to account for stake delegation during genesis.
use iota_config::genesis::{
    TokenAllocation, TokenDistributionSchedule, TokenDistributionScheduleBuilder,
};
use iota_types::{
    base_types::{IotaAddress, ObjectRef},
    gas_coin::STARDUST_TOTAL_SUPPLY_NANOS,
    object::Object,
    stardust::coin_kind::get_gas_balance_maybe,
};

use crate::{
    stardust::migration::{ExpirationTimestamp, MigrationObjects},
    validator_info::GenesisValidatorInfo,
};

#[derive(Default, Debug, Clone)]
pub struct GenesisStake {
    token_allocation: Vec<TokenAllocation>,
    gas_coins_to_burn: Vec<ObjectRef>,
    timelocks_to_burn: Vec<ObjectRef>,
    timelocks_to_split: Vec<(ObjectRef, u64, IotaAddress)>,
}

impl GenesisStake {
    /// Take the inner gas-coin objects that must be burned.
    ///
    /// This follows the semantics of [`std::mem::take`].
    pub fn take_gas_coins_to_burn(&mut self) -> Vec<ObjectRef> {
        std::mem::take(&mut self.gas_coins_to_burn)
    }

    /// Take the inner timelock objects that must be burned.
    ///
    /// This follows the semantics of [`std::mem::take`].
    pub fn take_timelocks_to_burn(&mut self) -> Vec<ObjectRef> {
        std::mem::take(&mut self.timelocks_to_burn)
    }

    /// Take the inner timelock objects that must be split.
    ///
    /// This follows the semantics of [`std::mem::take`].
    pub fn take_timelocks_to_split(&mut self) -> Vec<(ObjectRef, u64, IotaAddress)> {
        std::mem::take(&mut self.timelocks_to_split)
    }

    pub fn is_empty(&self) -> bool {
        self.token_allocation.is_empty()
            && self.gas_coins_to_burn.is_empty()
            && self.timelocks_to_burn.is_empty()
    }

    /// Calculate the total amount of token allocations.
    pub fn sum_token_allocation(&self) -> u64 {
        self.token_allocation
            .iter()
            .map(|allocation| allocation.amount_nanos)
            .sum()
    }

    /// Create a new valid [`TokenDistributionSchedule`] from the
    /// inner token allocations.
    pub fn to_token_distribution_schedule(&self) -> TokenDistributionSchedule {
        let mut builder = TokenDistributionScheduleBuilder::new();

        let pre_minted_supply = self.calculate_pre_minted_supply();

        builder.set_pre_minted_supply(pre_minted_supply);

        for allocation in self.token_allocation.clone() {
            builder.add_allocation(allocation);
        }
        builder.build()
    }

    /// Extend a vanilla [`TokenDistributionSchedule`] with the
    /// inner token allocations.
    ///
    /// The resulting schedule is guaranteed to contain allocations
    /// that sum up the initial total supply of Iota in nanos.
    ///
    /// ## Errors
    ///
    /// The method fails if the resulting schedule contains is invalid.
    pub fn extend_vanilla_token_distribution_schedule(
        &self,
        mut vanilla_schedule: TokenDistributionSchedule,
    ) -> TokenDistributionSchedule {
        vanilla_schedule
            .allocations
            .extend(self.token_allocation.clone());
        vanilla_schedule.pre_minted_supply = self.calculate_pre_minted_supply();
        vanilla_schedule.validate();
        vanilla_schedule
    }

    /// Calculates the part of the IOTA supply that is pre-minted.
    fn calculate_pre_minted_supply(&self) -> u64 {
        STARDUST_TOTAL_SUPPLY_NANOS - self.sum_token_allocation()
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
    /// A (possible empty) vector of (amount, timelock_expiration) pairs
    /// indicating the amount to timelock stake and its expiration
    staked_with_timelock: Vec<(u64, u64)>,
}

/// Pick gas-coin like objects from a pool to cover
/// the `target_amount`.
///
/// This does not split any surplus balance, but delegates
/// splitting to the caller.
pub fn pick_objects_for_allocation<'obj>(
    pool: &mut impl Iterator<Item = (&'obj Object, ExpirationTimestamp)>,
    target_amount: u64,
) -> AllocationObjects {
    let mut amount_nanos = 0;
    let mut surplus_nanos = 0;
    // Will be left empty in the case of gas coins
    let mut staked_with_timelock = vec![];

    let objects = pool
        .by_ref()
        .map_while(|(object, timestamp)| {
            if amount_nanos < target_amount {
                let mut object_balance = get_gas_balance_maybe(object)?.value();
                // Check if remaining is needed to be handled
                let remaining_needed = target_amount - amount_nanos;
                if object_balance > remaining_needed {
                    surplus_nanos = object_balance - remaining_needed;
                    object_balance = remaining_needed;
                }
                // Finally update amount
                amount_nanos += object_balance;
                // Store timestamp if it is a Timelock
                if timestamp > 0 {
                    staked_with_timelock.push((object_balance, timestamp));
                }
                Some(object.compute_object_reference())
            } else {
                None
            }
        })
        .collect();

    AllocationObjects {
        inner: objects,
        amount_nanos,
        surplus_nanos,
        staked_with_timelock,
    }
}

/// Create the necessary allocations to cover `amount_nanos` for all
/// `validators`.
///
/// This function iterates in turn over [`TimeLock`] and
/// [`GasCoin`][iota_types::gas_coin::GasCoin] objects created
/// during stardust migration that are owned by the `delegator`.
pub fn delegate_genesis_stake<'info>(
    validators: impl Iterator<Item = &'info GenesisValidatorInfo>,
    delegator: IotaAddress,
    migration_objects: &MigrationObjects,
    amount_nanos: u64,
) -> anyhow::Result<GenesisStake> {
    let timelocks_pool = migration_objects.get_sorted_timelocks_and_expiration_by_owner(delegator);
    let gas_coins_pool = migration_objects.get_gas_coins_by_owner(delegator);
    if timelocks_pool.is_none() && gas_coins_pool.is_none() {
        anyhow::bail!("no timelocks or gas-coin objects found for delegator {delegator:?}");
    }
    let mut timelocks_pool = timelocks_pool.unwrap_or_default().into_iter();
    let mut gas_coins_pool = gas_coins_pool
        .unwrap_or_default()
        .into_iter()
        .map(|object| (object, 0));
    let mut genesis_stake = GenesisStake::default();

    // For each validator we try to fill their allocation up to
    // total_amount_to_stake_per_validator
    for validator in validators {
        let target_stake = amount_nanos;

        // Start filling allocations with timelocks
        let mut timelock_objects = pick_objects_for_allocation(&mut timelocks_pool, target_stake);
        // TODO: This is not an optimal solution because the last timelock
        // might have a surplus amount, which cannot be used without splitting.
        if !timelock_objects.inner.is_empty() {
            timelock_objects.staked_with_timelock.iter().for_each(
                |&(timelocked_amount, expiration_timestamp)| {
                    // For timelocks we create a `TokenAllocation` object with
                    // `staked_with_timelock` filled with entries
                    genesis_stake.token_allocation.push(TokenAllocation {
                        recipient_address: delegator,
                        amount_nanos: timelocked_amount,
                        staked_with_validator: Some(validator.info.iota_address()),
                        staked_with_timelock_expiration: Some(expiration_timestamp),
                    });
                },
            );
            // Get the reference to the timelock to split needed to get exactly
            // `amount_nanos`
            let timelock_to_split = *timelock_objects
                .inner
                .last()
                .expect("there should be at least two objects");
            // Save all the references to timelocks to burn
            genesis_stake
                .timelocks_to_burn
                .append(&mut timelock_objects.inner);
            // Save the reference for the token to split (and then burn)
            genesis_stake.timelocks_to_split.push((
                timelock_to_split,
                timelock_objects.surplus_nanos,
                delegator,
            ))
        }

        // Then cover any remaining target stake with gas coins
        let remainder_target_stake = target_stake - timelock_objects.amount_nanos;
        let mut gas_coin_objects =
            pick_objects_for_allocation(&mut gas_coins_pool, remainder_target_stake);
        genesis_stake
            .gas_coins_to_burn
            .append(&mut gas_coin_objects.inner);
        // TODO: also here, this is not an optimal solution because the last gas object
        // might have a surplus amount, which cannot be used without splitting.
        if gas_coin_objects.amount_nanos < remainder_target_stake {
            return Err(anyhow::anyhow!(
                "Not enough funds for delegator {:?}",
                delegator
            ));
        } else if gas_coin_objects.amount_nanos > 0 {
            // For gas coins we create a `TokenAllocation` object with
            // an empty`staked_with_timelock`
            genesis_stake.token_allocation.push(TokenAllocation {
                recipient_address: delegator,
                amount_nanos: gas_coin_objects.amount_nanos,
                staked_with_validator: Some(validator.info.iota_address()),
                staked_with_timelock_expiration: None,
            });
            if gas_coin_objects.surplus_nanos > 0 {
                // This essentially schedules returning any surplus amount
                // from the last coin in `gas_coin_objects` to the delegator
                // as a new coin, so that the split is not needed
                genesis_stake.token_allocation.push(TokenAllocation {
                    recipient_address: delegator,
                    amount_nanos: gas_coin_objects.surplus_nanos,
                    staked_with_validator: None,
                    staked_with_timelock_expiration: None,
                });
            }
        }
    }
    Ok(genesis_stake)
}
