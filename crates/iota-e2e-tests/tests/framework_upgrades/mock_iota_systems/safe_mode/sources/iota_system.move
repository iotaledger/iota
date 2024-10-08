// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::iota_system {
    use iota::balance::Balance;
    use iota::object::UID;
    use iota::iota::IOTA;
    use iota::transfer;
    use iota::tx_context::{Self, TxContext};
    use iota::dynamic_field;

    use iota_system::validator::Validator;
    use iota_system::iota_system_state_inner::IotaSystemStateInner;
    use iota_system::iota_system_state_inner;

    friend iota_system::genesis;

    struct IotaSystemState has key {
        id: UID,
        version: u64,
    }

    public(friend) fun create(
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
        let self = IotaSystemState {
            id,
            version,
        };
        dynamic_field::add(&mut self.id, version, system_state);
        transfer::share_object(self);
    }

    fun advance_epoch(
        storage_charge: Balance<IOTA>,
        computation_reward: Balance<IOTA>,
        wrapper: &mut IotaSystemState,
        _new_epoch: u64,
        _next_protocol_version: u64,
        storage_rebate: u64,
        _non_refundable_storage_fee: u64,
        _storage_fund_reinvest_rate: u64,
        _reward_slashing_rate: u64,
        _epoch_start_timestamp_ms: u64,
        ctx: &mut TxContext,
    ) : Balance<IOTA> {
        let self = load_system_state_mut(wrapper);
        assert!(tx_context::sender(ctx) == @0x1, 0); // aborts here
        iota_system_state_inner::advance_epoch(
            self,
            storage_charge,
            computation_reward,
            storage_rebate,
        )
    }

    fun load_system_state_mut(self: &mut IotaSystemState): &mut IotaSystemStateInner {
        let version = self.version;
        dynamic_field::borrow_mut(&mut self.id, version)
    }
}
