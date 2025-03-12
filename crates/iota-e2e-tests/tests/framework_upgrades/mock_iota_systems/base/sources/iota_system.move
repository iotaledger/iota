// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::iota_system {
    use iota::balance::Balance;
    use iota::dynamic_field;
    use iota::iota::IOTA;
    use iota::iota::IotaTreasuryCap;
    use iota::system_admin_cap::IotaSystemAdminCap;

    use iota_system::validator::ValidatorV1;
    use iota_system::iota_system_state_inner::{Self, IotaSystemStateV1};

    public struct IotaSystemState has key {
        id: UID,
        version: u64,
    }

    public(package) fun create(
        id: UID,
        iota_treasury_cap: IotaTreasuryCap,
        validators: vector<ValidatorV1>,
        storage_fund: Balance<IOTA>,
        protocol_version: u64,
        epoch_start_timestamp_ms: u64,
        epoch_duration_ms: u64,
        iota_system_admin_cap: IotaSystemAdminCap,
        ctx: &mut TxContext,
    ) {
        let system_state = iota_system_state_inner::create(
            iota_treasury_cap,
            validators,
            storage_fund,
            protocol_version,
            epoch_start_timestamp_ms,
            epoch_duration_ms,
            iota_system_admin_cap,
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

    #[allow(unused_function)]
    fun advance_epoch(
        validator_subsidy: u64,
        storage_charge: Balance<IOTA>,
        computation_charge: Balance<IOTA>,
        computation_charge_burned: u64,
        wrapper: &mut IotaSystemState,
        new_epoch: u64,
        next_protocol_version: u64,
        storage_rebate: u64,
        non_refundable_storage_fee: u64,
        reward_slashing_rate: u64,
        epoch_start_timestamp_ms: u64,
        ctx: &mut TxContext,
    ) : Balance<IOTA> {
        let self = load_system_state_mut(wrapper);
        assert!(tx_context::sender(ctx) == @0x0, 0);
        let storage_rebate = iota_system_state_inner::advance_epoch(
            self,
            new_epoch,
            next_protocol_version,
            validator_subsidy,
            storage_charge,
            computation_charge,
            computation_charge_burned,
            storage_rebate,
            non_refundable_storage_fee,
            reward_slashing_rate,
            epoch_start_timestamp_ms,
            ctx
        );

        storage_rebate
    }

    public fun active_validator_addresses(_wrapper: &mut IotaSystemState): vector<address> {
        vector::empty()
    }

    fun load_system_state_mut(self: &mut IotaSystemState): &mut IotaSystemStateV1 {
        load_inner_maybe_upgrade(self)
    }

    fun load_inner_maybe_upgrade(self: &mut IotaSystemState): &mut IotaSystemStateV1 {
        let version = self.version;
        // TODO: This is where we check the version and perform upgrade if necessary.

        let inner: &mut IotaSystemStateV1 = dynamic_field::borrow_mut(&mut self.id, version);
        assert!(iota_system_state_inner::system_state_version(inner) == version, 0);
        inner
    }
}
