// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Logic and types to account for stake delegation during genesis.
use iota_config::genesis::{
    Delegations, TokenAllocation, TokenDistributionSchedule, TokenDistributionScheduleBuilder,
    ValidatorAllocation,
};
use iota_types::{
    base_types::{IotaAddress, ObjectRef},
    object::Object,
    stardust::coin_kind::get_gas_balance_maybe,
};

use crate::stardust::migration::{ExpirationTimestamp, MigrationObjects};

#[derive(Default, Debug, Clone)]
pub struct GenesisStake {
    token_allocation: Vec<TokenAllocation>,
    gas_coins_to_destroy: Vec<ObjectRef>,
    timelocks_to_destroy: Vec<ObjectRef>,
    timelocks_to_split: Vec<(ObjectRef, u64, IotaAddress)>,
}

impl GenesisStake {
    /// Take the inner gas-coin objects that must be destroyed.
    ///
    /// This follows the semantics of [`std::mem::take`].
    pub fn take_gas_coins_to_destroy(&mut self) -> Vec<ObjectRef> {
        std::mem::take(&mut self.gas_coins_to_destroy)
    }

    /// Take the inner timelock objects that must be destroyed.
    ///
    /// This follows the semantics of [`std::mem::take`].
    pub fn take_timelocks_to_destroy(&mut self) -> Vec<ObjectRef> {
        std::mem::take(&mut self.timelocks_to_destroy)
    }

    /// Take the inner timelock objects that must be split.
    ///
    /// This follows the semantics of [`std::mem::take`].
    pub fn take_timelocks_to_split(&mut self) -> Vec<(ObjectRef, u64, IotaAddress)> {
        std::mem::take(&mut self.timelocks_to_split)
    }

    pub fn is_empty(&self) -> bool {
        self.token_allocation.is_empty()
            && self.gas_coins_to_destroy.is_empty()
            && self.timelocks_to_destroy.is_empty()
            && self.timelocks_to_split.is_empty()
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
    pub fn to_token_distribution_schedule(
        &self,
        total_supply_nanos: u64,
    ) -> TokenDistributionSchedule {
        let mut builder = TokenDistributionScheduleBuilder::new();

        let pre_minted_supply = self.calculate_pre_minted_supply(total_supply_nanos);

        builder.set_pre_minted_supply(pre_minted_supply);

        for allocation in self.token_allocation.clone() {
            builder.add_allocation(allocation);
        }
        builder.build()
    }

    /// Extend a [`TokenDistributionSchedule`] without migration with the
    /// inner token allocations.
    ///
    /// The resulting schedule is guaranteed to contain allocations
    /// that sum up the initial total supply of IOTA in nanos.
    ///
    /// ## Errors
    ///
    /// The method fails if the resulting schedule contains is invalid.
    pub fn extend_token_distribution_schedule_without_migration(
        &self,
        mut schedule_without_migration: TokenDistributionSchedule,
        total_supply_nanos: u64,
    ) -> TokenDistributionSchedule {
        schedule_without_migration
            .allocations
            .extend(self.token_allocation.clone());
        schedule_without_migration.pre_minted_supply =
            self.calculate_pre_minted_supply(total_supply_nanos);
        schedule_without_migration.validate();
        schedule_without_migration
    }

    /// Calculates the part of the IOTA supply that is pre-minted.
    fn calculate_pre_minted_supply(&self, total_supply_nanos: u64) -> u64 {
        total_supply_nanos - self.sum_token_allocation()
    }

    /// Creates a `GenesisStake` using a `Delegations` containing the necessary
    /// allocations for validators by some delegators.
    ///
    /// This function invokes `delegate_genesis_stake` for each delegator found
    /// in `Delegations`.
    pub fn new_with_delegations(
        delegations: Delegations,
        migration_objects: &MigrationObjects,
    ) -> anyhow::Result<Self> {
        let mut stake = GenesisStake::default();

        for (delegator, validators_allocations) in delegations.allocations {
            // Fetch all timelock and gas objects owned by the delegator
            let timelocks_pool =
                migration_objects.get_sorted_timelocks_and_expiration_by_owner(delegator);
            let gas_coins_pool = migration_objects.get_gas_coins_by_owner(delegator);
            if timelocks_pool.is_none() && gas_coins_pool.is_none() {
                anyhow::bail!("no timelocks or gas-coin objects found for delegator {delegator:?}");
            }
            stake.delegate_genesis_stake(
                &validators_allocations,
                delegator,
                &mut timelocks_pool.unwrap_or_default().into_iter(),
                &mut gas_coins_pool
                    .unwrap_or_default()
                    .into_iter()
                    .map(|object| (object, 0)),
            )?;
        }

        Ok(stake)
    }

    fn create_token_allocation(
        &mut self,
        recipient_address: IotaAddress,
        amount_nanos: u64,
        staked_with_validator: Option<IotaAddress>,
        staked_with_timelock_expiration: Option<u64>,
    ) {
        self.token_allocation.push(TokenAllocation {
            recipient_address,
            amount_nanos,
            staked_with_validator,
            staked_with_timelock_expiration,
        });
    }

    /// Create the necessary allocations for `validators_allocations` using the
    /// assets of the `delegator`.
    ///
    /// This function iterates in turn over [`TimeLock`] and
    /// [`GasCoin`][iota_types::gas_coin::GasCoin] objects created
    /// during stardust migration that are owned by the `delegator`.
    pub fn delegate_genesis_stake<'obj>(
        &mut self,
        validators_allocations: &[ValidatorAllocation],
        delegator: IotaAddress,
        timelocks_pool: &mut impl Iterator<Item = (&'obj Object, ExpirationTimestamp)>,
        gas_coins_pool: &mut impl Iterator<Item = (&'obj Object, ExpirationTimestamp)>,
    ) -> anyhow::Result<()> {
        // Temp stores for holding the surplus
        let mut timelock_surplus = SurplusCoin::default();
        let mut gas_surplus = SurplusCoin::default();

        // Then, try to create new token allocations for each validator using the
        // objects fetched above
        for validator_allocation in validators_allocations {
            // The validator address
            let validator = validator_allocation.validator;
            // The target amount of nanos to be staked, either with timelock or gas objects
            let mut target_stake_nanos = validator_allocation.amount_nanos_to_stake;
            // The gas to pay to the validator
            let gas_to_pay_nanos = validator_allocation.amount_nanos_to_pay_gas;

            // Start filling allocations with timelocks

            // Pick fresh timelock objects (if present) and possibly reuse the surplus
            // coming from the previous iteration.
            // The method `pick_objects_for_allocation` firstly checks if the
            // `timelock_surplus` can be used to reach or reduce the `target_stake_nanos`.
            // Then it iterates over the `timelocks_pool`. For each timelock object, its
            // balance is used to reduce the `target_stake_nanos` while its the object
            // reference is placed into a vector `to_destroy`. At the end, the
            // `pick_objects_for_allocation` method returns an `AllocationObjects` including
            // the list of objects to destroy, the list `staked_with_timelock` containing
            // the information for creating token allocations with timestamps
            // and a CoinSurplus (even empty).
            let mut timelock_allocation_objects = pick_objects_for_allocation(
                timelocks_pool,
                target_stake_nanos,
                &mut timelock_surplus,
            );
            if !timelock_allocation_objects.staked_with_timelock.is_empty() {
                // Inside this block some timelock objects were picked from the pool; so we can
                // save all the references to timelocks to destroy, if there are any
                self.timelocks_to_destroy
                    .append(&mut timelock_allocation_objects.to_destroy);
                // Finally we create some token allocations based on timelock_allocation_objects
                timelock_allocation_objects
                    .staked_with_timelock
                    .iter()
                    .for_each(|&(timelocked_amount, expiration_timestamp)| {
                        // For timelocks we create a `TokenAllocation` object with
                        // `staked_with_timelock` filled with entries
                        self.create_token_allocation(
                            delegator,
                            timelocked_amount,
                            Some(validator),
                            Some(expiration_timestamp),
                        );
                    });
            }
            // The remainder of the target stake after timelock objects were used.
            target_stake_nanos -= timelock_allocation_objects.amount_nanos;

            // After allocating timelocked stakes, then
            // 1. allocate gas coin stakes (if timelocked funds were not enough)
            // 2. and/or allocate gas coin payments (if indicated in the validator
            //    allocation).

            // The target amount of gas coin nanos to be allocated, either with staking or
            // to pay
            let target_gas_nanos = target_stake_nanos + gas_to_pay_nanos;
            // Pick fresh gas coin objects (if present) and possibly reuse the surplus
            // coming from the previous iteration. The logic is the same as above with
            // timelocks.
            let mut gas_coin_objects =
                pick_objects_for_allocation(gas_coins_pool, target_gas_nanos, &mut gas_surplus);
            if gas_coin_objects.amount_nanos >= target_gas_nanos {
                // Inside this block some gas coin objects were picked from the pool; so we can
                // save all the references to gas coins to destroy
                self.gas_coins_to_destroy
                    .append(&mut gas_coin_objects.to_destroy);
                // Then
                // Case 1. allocate gas stakes
                if target_stake_nanos > 0 {
                    // For staking gas coins we create a `TokenAllocation` object with
                    // an empty `staked_with_timelock`
                    self.create_token_allocation(
                        delegator,
                        target_stake_nanos,
                        Some(validator),
                        None,
                    );
                }
                // Case 2. allocate gas payments
                if gas_to_pay_nanos > 0 {
                    // For gas coins payments we create a `TokenAllocation` object with
                    // `recipient_address` being the validator and no stake
                    self.create_token_allocation(validator, gas_to_pay_nanos, None, None);
                }
            } else {
                // It means the delegator finished all the timelock or gas funds
                return Err(anyhow::anyhow!(
                    "Not enough funds for delegator {:?}",
                    delegator
                ));
            }
        }

        // If some surplus amount is left, then return it to the delegator
        // In the case of a timelock object, it must be split during the `genesis` PTB
        // execution
        if let (Some(surplus_timelock), surplus_nanos, _) = timelock_surplus.take() {
            self.timelocks_to_split
                .push((surplus_timelock, surplus_nanos, delegator));
        }
        // In the case of a gas coin, it must be destroyed and the surplus re-allocated
        // to the delegator (no split)
        if let (Some(surplus_gas_coin), surplus_nanos, _) = gas_surplus.take() {
            self.gas_coins_to_destroy.push(surplus_gas_coin);
            self.create_token_allocation(delegator, surplus_nanos, None, None);
        }

        Ok(())
    }
}

/// The objects picked for token allocation during genesis
#[derive(Default, Debug, Clone)]
struct AllocationObjects {
    /// The list of objects to destroy for the allocations
    to_destroy: Vec<ObjectRef>,
    /// The total amount of nanos to be allocated from this
    /// collection of objects.
    amount_nanos: u64,
    /// A (possible empty) vector of (amount, timelock_expiration) pairs
    /// indicating the amount to timelock stake and its expiration
    staked_with_timelock: Vec<(u64, u64)>,
}

/// The surplus object that should be split for this allocation. Only part
/// of its balance will be used for this collection of this
/// `AllocationObjects`, the surplus might be used later.
#[derive(Default, Debug, Clone)]
struct SurplusCoin {
    // The reference of the coin to possibly split to get the surplus.
    coin_object_ref: Option<ObjectRef>,
    /// The surplus amount for that coin object.
    surplus_nanos: u64,
    /// Possibly indicate a timelock stake expiration.
    timestamp: u64,
}

impl SurplusCoin {
    // Check if the current surplus can be reused.
    // The surplus coin_object_ref is returned to be included in a `to_destroy` list
    // when surplus_nanos <= target_amount_nanos. Otherwise it means the
    // target_amount_nanos is completely reached, so we can still keep
    // coin_object_ref as surplus coin and only reduce the surplus_nanos value.
    pub fn maybe_reuse_surplus(
        &mut self,
        target_amount_nanos: u64,
    ) -> (Option<ObjectRef>, u64, u64) {
        // If the surplus is some, then we can use the surplus nanos
        if self.coin_object_ref.is_some() {
            // If the surplus nanos are less or equal than the target, then use them all and
            // return the coin object to be destroyed
            if self.surplus_nanos <= target_amount_nanos {
                let (coin_object_ref_opt, surplus, timestamp) = self.take();
                (Some(coin_object_ref_opt.unwrap()), surplus, timestamp)
            } else {
                // If the surplus nanos more than the target, do not return the coin object
                self.surplus_nanos -= target_amount_nanos;
                (None, target_amount_nanos, self.timestamp)
            }
        } else {
            (None, 0, 0)
        }
    }

    // Destroy the `CoinSurplus` and take the fields.
    pub fn take(&mut self) -> (Option<ObjectRef>, u64, u64) {
        let surplus = self.surplus_nanos;
        self.surplus_nanos = 0;
        let timestamp = self.timestamp;
        self.timestamp = 0;
        (self.coin_object_ref.take(), surplus, timestamp)
    }
}

/// Pick gas-coin like objects from a pool to cover
/// the `target_amount_nanos`. It might also make use of a previous coin
/// surplus.
///
/// This does not split any surplus balance, but delegates
/// splitting to the caller.
fn pick_objects_for_allocation<'obj>(
    pool: &mut impl Iterator<Item = (&'obj Object, ExpirationTimestamp)>,
    target_amount_nanos: u64,
    surplus_coin: &mut SurplusCoin,
) -> AllocationObjects {
    // Vector used to keep track of timestamps while allocating timelock coins.
    // Will be left empty in the case of gas coins
    let mut staked_with_timelock = vec![];
    // Vector used to keep track of the coins to destroy.
    let mut to_destroy = vec![];
    // Variable used to keep track of allocated nanos during the picking.
    let mut allocation_amount_nanos = 0;

    // Maybe use the surplus coin passed as input.
    let (surplus_object_option, used_surplus_nanos, surplus_timestamp) =
        surplus_coin.maybe_reuse_surplus(target_amount_nanos);

    // If the surplus coin was used then allocate the nanos and maybe destroy it
    if used_surplus_nanos > 0 {
        allocation_amount_nanos += used_surplus_nanos;
        if surplus_timestamp > 0 {
            staked_with_timelock.push((used_surplus_nanos, surplus_timestamp));
        }
        // If the `surplus_object` is returned by `maybe_reuse_surplus`, then it means
        // it used all its `used_surplus_nanos` and it can be destroyed.
        if let Some(surplus_object) = surplus_object_option {
            to_destroy.push(surplus_object);
        }
    }
    // Else, if the `surplus_object` was not completely drained, then we
    // don't need to continue. In this case `allocation_amount_nanos ==
    // target_amount_nanos`.

    // Only if `allocation_amount_nanos` < `target_amount_nanos` then pick an
    // object (if we still have objects in the pool). If this object's balance is
    // less than the difference required to reach the target, then push this
    // object's reference into the `to_destroy` list. Else, take out only the
    // required amount and set the object as a "surplus" (then break the loop).
    while allocation_amount_nanos < target_amount_nanos {
        if let Some((object, timestamp)) = pool.next() {
            // In here we pick an object
            let obj_ref = object.compute_object_reference();
            let object_balance = get_gas_balance_maybe(object)
                .expect("the pool should only contain gas coins or timelock balance objects")
                .value();

            // Then we create the allocation
            let difference_from_target = target_amount_nanos - allocation_amount_nanos;
            let to_allocate = object_balance.min(difference_from_target);
            allocation_amount_nanos += to_allocate;
            if timestamp > 0 {
                staked_with_timelock.push((to_allocate, timestamp));
            }

            // If the balance is less or equal than the difference from target, then
            // place `obj_ref` in `to_destroy` and continue
            if object_balance <= difference_from_target {
                to_destroy.push(obj_ref);
            } else {
                // Else, do NOT place `obj_ref` in `to_destroy` because it is reused in
                // the SurplusCoin and then BREAK, because we reached the target
                *surplus_coin = SurplusCoin {
                    coin_object_ref: Some(obj_ref),
                    surplus_nanos: object_balance - difference_from_target,
                    timestamp,
                };
                break;
            }
        } else {
            // We have no more objects to pick from the pool; the function will end with
            // allocation_amount_nanos < target_amount_nanos
            break;
        }
    }

    AllocationObjects {
        to_destroy,
        amount_nanos: allocation_amount_nanos,
        staked_with_timelock,
    }
}
