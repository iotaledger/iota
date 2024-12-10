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
    /// that sum up the initial total supply of Iota in nanos.
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
        let mut timelock_surplus = CoinSurplus::default();
        let mut gas_surplus = CoinSurplus::default();

        // Then, try to create new token allocations for each validator using the
        // objects fetched above
        for validator_allocation in validators_allocations {
            // The validaotr address
            let validator = validator_allocation.address;
            // The target amount of nanos to be staked, either with timelock or gas objects
            let mut target_stake = validator_allocation.amount_nanos_to_stake;
            // The gas to pay to the validator
            let gas_to_pay = validator_allocation.amount_nanos_to_pay_gas;

            // Start filling allocations with timelocks

            // Pick fresh timelock objects (if present) and possibly reuse the surplus
            // coming from the previous iteration
            let mut timelock_objects =
                pick_objects_for_allocation(timelocks_pool, target_stake, &mut timelock_surplus);
            if !timelock_objects.to_burn.is_empty() {
                // Inside this block, the previous surplus is empty so we update it (possibly)
                // with a new one
                timelock_surplus = timelock_objects.surplus;
                // Save all the references to timelocks to burn
                self.timelocks_to_burn.append(&mut timelock_objects.to_burn);
                // Finally we create some token allocations based on timelock_objects
                timelock_objects.staked_with_timelock.iter().for_each(
                    |&(timelocked_amount, expiration_timestamp)| {
                        // For timelocks we create a `TokenAllocation` object with
                        // `staked_with_timelock` filled with entries
                        self.create_token_allocation(
                            delegator,
                            timelocked_amount,
                            Some(validator),
                            Some(expiration_timestamp),
                        );
                    },
                );
            }
            // The remainder of the target stake after timelocks were used.
            target_stake -= timelock_objects.amount_nanos;

            // After allocating timelocked stakes, then
            // 1. allocate gas stakes (if timelocked funds were not enough)
            // 2. and/or allocate gas payments (if indicated in the validator allocation).

            // The target amount of gas coin nanos to be allocated, either with staking or
            // to pay
            let target_gas = target_stake + gas_to_pay;
            // Pick fresh gas coin objects (if present) and possibly reuse the surplus
            // coming from the previous iteration
            let mut gas_coin_objects =
                pick_objects_for_allocation(gas_coins_pool, target_gas, &mut gas_surplus);
            if gas_coin_objects.amount_nanos >= target_gas {
                // Inside this block, the previous surplus is empty so we update it (possibly)
                // with a new one
                gas_surplus = gas_coin_objects.surplus;
                // Save all the references to gas coins to burn
                self.gas_coins_to_burn.append(&mut gas_coin_objects.to_burn);
                // Then
                // Case 1. allocate gas stakes
                if target_stake > 0 {
                    // For staking gas coins we create a `TokenAllocation` object with
                    // an empty `staked_with_timelock`
                    self.create_token_allocation(delegator, target_stake, Some(validator), None);
                }
                // Case 2. allocate gas payments
                if gas_to_pay > 0 {
                    // For gas coins payments we create a `TokenAllocation` object with
                    // `recipient_address` being the validator and no stake
                    self.create_token_allocation(validator, gas_to_pay, None, None);
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Not enough funds for delegator {:?}",
                    delegator
                ));
            }
        }

        // If some surplus amount is left, then return it to the delegator
        // In the case of a timelock object, it must be split during the `genesis` PTB
        // execution
        if let (Some(surplus_timelock), surplus_nanos) = timelock_surplus.take() {
            self.timelocks_to_split
                .push((surplus_timelock, surplus_nanos, delegator));
        }
        // In the case a the gas coin, it must be burned and the surplus re-allocated to
        // the delegator (no split)
        if let (Some(surplus_gas_coin), surplus_nanos) = gas_surplus.take() {
            self.gas_coins_to_burn.push(surplus_gas_coin);
            self.create_token_allocation(delegator, surplus_nanos, None, None);
        }

        Ok(())
    }
}

/// The objects picked for token allocation during genesis
#[derive(Default, Debug, Clone)]
struct AllocationObjects {
    /// The list of objects to burn for the allocations
    to_burn: Vec<ObjectRef>,
    /// The total amount of nanos to be allocated from this
    /// collection of objects.
    amount_nanos: u64,
    /// The surplus object that should be split for this allocation. Only part
    /// of its balance will be used for this collection of this
    /// `AllocationObjects`, the surplus might be used later.
    surplus: CoinSurplus,
    /// A (possible empty) vector of (amount, timelock_expiration) pairs
    /// indicating the amount to timelock stake and its expiration
    staked_with_timelock: Vec<(u64, u64)>,
}

/// The objects picked for token allocation during genesis
#[derive(Default, Debug, Clone)]
struct CoinSurplus {
    // The reference of the coin to possibly split to get the surplus.
    obj_ref: Option<ObjectRef>,
    /// The surplus amount for that coin object.
    surplus_nanos: u64,
    /// Possibly indicate a timelock stake expiration.
    timestamp: u64,
}

impl CoinSurplus {
    // Check if the current surplus can be reused.
    // The surplus_timelock object is returned to be burnt when its surplus_nanos <=
    // target_stake. Otherwise surplus_timelock is kept as coin surplus with a
    // reduced surplus_nanos and the target_amount is completely filled.
    pub fn maybe_reuse_surplus(
        &mut self,
        target_amount: u64,
    ) -> (Option<ObjectRef>, Option<u64>, u64) {
        if self.obj_ref.is_some() {
            if self.surplus_nanos <= target_amount {
                let surplus = self.surplus_nanos;
                self.surplus_nanos = 0;
                (self.obj_ref.take(), Some(surplus), self.timestamp)
            } else {
                self.surplus_nanos -= target_amount;
                (None, Some(target_amount), self.timestamp)
            }
        } else {
            (None, None, 0)
        }
    }

    // Destroy the `CoinSurplus` and take the fields.
    pub fn take(self) -> (Option<ObjectRef>, u64) {
        (self.obj_ref, self.surplus_nanos)
    }
}

/// Pick gas-coin like objects from a pool to cover
/// the `target_amount`. It might also make use of a previous surplus.
///
/// This does not split any surplus balance, but delegates
/// splitting to the caller.
fn pick_objects_for_allocation<'obj>(
    pool: &mut impl Iterator<Item = (&'obj Object, ExpirationTimestamp)>,
    target_amount: u64,
    previous_surplus: &mut CoinSurplus,
) -> AllocationObjects {
    let mut allocation_tot_amount = 0;
    let mut surplus = Default::default();
    // Will be left empty in the case of gas coins
    let mut staked_with_timelock = vec![];
    let mut to_burn = vec![];

    if let (surplus_object_option, Some(surplus_nanos), timestamp) =
        previous_surplus.maybe_reuse_surplus(target_amount)
    {
        if timestamp > 0 {
            if let Some(timelock_object) = surplus_object_option {
                to_burn.push(timelock_object);
                staked_with_timelock.push((surplus_nanos, timestamp));
            }
        } else {
            if let Some(gas_coin_object) = surplus_object_option {
                to_burn.push(gas_coin_object);
            }
        }
        allocation_tot_amount += surplus_nanos;
    }

    if allocation_tot_amount < target_amount {
        to_burn.append(
            &mut pool
                .by_ref()
                .map_while(|(object, timestamp)| {
                    if allocation_tot_amount < target_amount {
                        let difference_from_target = target_amount - allocation_tot_amount;
                        let obj_ref = object.compute_object_reference();
                        let object_balance = get_gas_balance_maybe(object)?.value();

                        if object_balance <= difference_from_target {
                            if timestamp > 0 {
                                staked_with_timelock.push((object_balance, timestamp));
                            }
                            allocation_tot_amount += object_balance;
                            // Continue
                            Some(obj_ref)
                        } else {
                            surplus = CoinSurplus {
                                obj_ref: Some(obj_ref),
                                surplus_nanos: object_balance - difference_from_target,
                                timestamp,
                            };
                            if timestamp > 0 {
                                staked_with_timelock.push((difference_from_target, timestamp));
                            }
                            allocation_tot_amount += difference_from_target;
                            // Break
                            None
                        }
                    } else {
                        // Break
                        None
                    }
                })
                .collect::<Vec<_>>(),
        );
    }

    AllocationObjects {
        to_burn,
        amount_nanos: allocation_tot_amount,
        surplus,
        staked_with_timelock,
    }
}
