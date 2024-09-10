// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module locked_stake::locked_stake {
    use iota::coin;
    use iota::balance::{Self, Balance};
    use iota::vec_map::{Self, VecMap};
    use iota::iota::IOTA;
    use iota_system::staking_pool::StakedIota;
    use iota_system::iota_system::{Self, IotaSystemState};
    use locked_stake::epoch_time_lock::{Self, EpochTimeLock};

    const EInsufficientBalance: u64 = 0;
    const EStakeObjectNonExistent: u64 = 1;

    /// An object that locks IOTA tokens and stake objects until a given epoch, and allows
    /// staking and unstaking operations when locked.
    public struct LockedStake has key {
        id: UID,
        staked_iota: VecMap<ID, StakedIota>,
        iota: Balance<IOTA>,
        locked_until_epoch: EpochTimeLock,
    }

    // ============================= basic operations =============================

    /// Create a new LockedStake object with empty staked_iota and iota balance given a lock time.
    /// Aborts if the given epoch has already passed.
    public fun new(locked_until_epoch: u64, ctx: &mut TxContext): LockedStake {
        LockedStake {
            id: object::new(ctx),
            staked_iota: vec_map::empty(),
            iota: balance::zero(),
            locked_until_epoch: epoch_time_lock::new(locked_until_epoch, ctx),
        }
    }

    /// Unlocks and returns all the assets stored inside this LockedStake object.
    /// Aborts if the unlock epoch is in the future.
    public fun unlock(ls: LockedStake, ctx: &TxContext): (VecMap<ID, StakedIota>, Balance<IOTA>) {
        let LockedStake { id, staked_iota, iota, locked_until_epoch } = ls;
        epoch_time_lock::destroy(locked_until_epoch, ctx);
        object::delete(id);
        (staked_iota, iota)
    }

    /// Deposit a new stake object to the LockedStake object.
    public fun deposit_staked_iota(ls: &mut LockedStake, staked_iota: StakedIota) {
        let id = object::id(&staked_iota);
        // This insertion can't abort since each object has a unique id.
        vec_map::insert(&mut ls.staked_iota, id, staked_iota);
    }

    /// Deposit iota balance to the LockedStake object.
    public fun deposit_iota(ls: &mut LockedStake, iota: Balance<IOTA>) {
        balance::join(&mut ls.iota, iota);
    }

    /// Take `amount` of IOTA from the iota balance, stakes it, and puts the stake object
    /// back into the staked iota vec map.
    public fun stake(
        ls: &mut LockedStake,
        iota_system: &mut IotaSystemState,
        amount: u64,
        validator_address: address,
        ctx: &mut TxContext
    ) {
        assert!(balance::value(&ls.iota) >= amount, EInsufficientBalance);
        let stake = iota_system::request_add_stake_non_entry(
            iota_system,
            coin::from_balance(balance::split(&mut ls.iota, amount), ctx),
            validator_address,
            ctx
        );
        deposit_staked_iota(ls, stake);
    }

    /// Unstake the stake object with `staked_iota_id` and puts the resulting principal
    /// and rewards back into the locked iota balance.
    /// Returns the amount of IOTA unstaked, including both principal and rewards.
    /// Aborts if no stake exists with the given id.
    public fun unstake(
        ls: &mut LockedStake,
        iota_system: &mut IotaSystemState,
        staked_iota_id: ID,
        ctx: &mut TxContext
    ): u64 {
        assert!(vec_map::contains(&ls.staked_iota, &staked_iota_id), EStakeObjectNonExistent);
        let (_, stake) = vec_map::remove(&mut ls.staked_iota, &staked_iota_id);
        let iota_balance = iota_system::request_withdraw_stake_non_entry(iota_system, stake, ctx);
        let amount = balance::value(&iota_balance);
        deposit_iota(ls, iota_balance);
        amount
    }

    // ============================= getters =============================

    public fun staked_iota(ls: &LockedStake): &VecMap<ID, StakedIota> {
        &ls.staked_iota
    }

    public fun iota_balance(ls: &LockedStake): u64 {
        balance::value(&ls.iota)
    }

    public fun locked_until_epoch(ls: &LockedStake): u64 {
        epoch_time_lock::epoch(&ls.locked_until_epoch)
    }

    // TODO: possibly add some scenarios like switching stake, creating a new LockedStake and transferring
    // it to the sender, etc. But these can also be done as PTBs.
}
