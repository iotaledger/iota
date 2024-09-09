// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::iota_system {
    use std::vector;

    use iota::balance::Balance;
    use iota::object::UID;
    use iota::iota::IOTA;
    use iota::transfer;
    use iota::tx_context::{Self, TxContext};
    use iota::dynamic_field;

    use iota_system::validator::Validator;
    use iota_system::iota_system_state_inner::{Self, IotaSystemStateInner, IotaSystemStateInnerV2};

    public struct IotaSystemState has key {
        id: UID,
        version: u64,
    }

    public(package) fun create(
        id: UID,
        validators: vector<Validator>,
        storage_fund: Balance<IOTA>,
        protocol_version: u64,
        epoch_start_timestamp_ms: u64,
        epoch_duration_ms: u64,
        ctx: &mut TxContext,
    ) {
        let system_state = iota_system_state_inner::create(
            validators,
            storage_fund,
            protocol_version,
            epoch_start_timestamp_ms,
            epoch_duration_ms,
            ctx,
        );
        let version = iota_system_state_inner::genesis_system_state_version();
        let mut self = IotaSystemState {
            id,
            version,
        };
        dynamic_field::add(&mut self.id, version, system_state);
        transfer::share_object(self);
    }

    fun advance_epoch(
        storage_reward: Balance<IOTA>,
        computation_reward: Balance<IOTA>,
        wrapper: &mut IotaSystemState,
        new_epoch: u64,
        next_protocol_version: u64,
        storage_rebate: u64,
        _non_refundable_storage_fee: u64,
        _storage_fund_reinvest_rate: u64, // share of storage fund's rewards that's reinvested
                                         // into storage fund, in basis point.
        _reward_slashing_rate: u64, // how much rewards are slashed to punish a validator, in bps.
        epoch_start_timestamp_ms: u64, // Timestamp of the epoch start
        ctx: &mut TxContext,
    ) : Balance<IOTA> {
        let self = load_system_state_mut(wrapper);
        assert!(tx_context::sender(ctx) == @0x0, 0);
        let storage_rebate = iota_system_state_inner::advance_epoch(
            self,
            new_epoch,
            next_protocol_version,
            storage_reward,
            computation_reward,
            storage_rebate,
            epoch_start_timestamp_ms,
        );

        storage_rebate
    }

    public fun active_validator_addresses(wrapper: &mut IotaSystemState): vector<address> {
        vector::empty()
    }

    fun load_system_state_mut(self: &mut IotaSystemState): &mut IotaSystemStateInnerV2 {
        load_inner_maybe_upgrade(self)
    }

    fun load_inner_maybe_upgrade(self: &mut IotaSystemState): &mut IotaSystemStateInnerV2 {
        let mut version = self.version;
        if (version == iota_system_state_inner::genesis_system_state_version()) {
            let inner: IotaSystemStateInner = dynamic_field::remove(&mut self.id, version);
            let new_inner = iota_system_state_inner::v1_to_v2(inner);
            version = iota_system_state_inner::system_state_version(&new_inner);
            dynamic_field::add(&mut self.id, version, new_inner);
            self.version = version;
        };

        let inner: &mut IotaSystemStateInnerV2 = dynamic_field::borrow_mut(&mut self.id, version);
        assert!(iota_system_state_inner::system_state_version(inner) == version, 0);
        inner
    }
}
