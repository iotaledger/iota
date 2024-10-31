// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::iota_system_state_inner {
    use iota::bag::{Self, Bag};
    use iota::balance::{Self, Balance};
    use iota::event;
    use iota::iota::IOTA;
    use iota::iota::IotaTreasuryCap;
    use iota::table::{Self, Table};

    use iota_system::validator::ValidatorV1;
    use iota_system::validator_wrapper::Validator;
    use iota_system::validator_wrapper;
    use iota_system::validator;

    const SYSTEM_STATE_VERSION_V1: u64 = 18446744073709551605;  // u64::MAX - 10

    public struct SystemEpochInfoEventV1 has copy, drop {
        epoch: u64,
        protocol_version: u64,
        reference_gas_price: u64,
        total_stake: u64,
        storage_charge: u64,
        storage_rebate: u64,
        storage_fund_balance: u64,
        total_gas_fees: u64,
        total_stake_rewards_distributed: u64,
        burnt_tokens_amount: u64,
        minted_tokens_amount: u64,
    }

    public struct SystemParametersV1 has store {
        epoch_duration_ms: u64,
        extra_fields: Bag,
    }

    public struct ValidatorSetV1 has store {
        active_validators: vector<ValidatorV1>,
        inactive_validators: Table<ID, Validator>,
        extra_fields: Bag,
    }

    public struct IotaSystemStateV1 has store {
        epoch: u64,
        protocol_version: u64,
        system_state_version: u64,
        iota_treasury_cap: IotaTreasuryCap,
        validators: ValidatorSetV1,
        storage_fund: Balance<IOTA>,
        parameters: SystemParametersV1,
        reference_gas_price: u64,
        safe_mode: bool,
        epoch_start_timestamp_ms: u64,
        extra_fields: Bag,
    }

    public(package) fun create(
        iota_treasury_cap: IotaTreasuryCap,
        validators: vector<ValidatorV1>,
        storage_fund: Balance<IOTA>,
        protocol_version: u64,
        epoch_start_timestamp_ms: u64,
        epoch_duration_ms: u64,
        ctx: &mut TxContext,
    ): IotaSystemStateV1 {
        let validators = new_validator_set(validators, ctx);
        let mut system_state = IotaSystemStateV1 {
            epoch: 0,
            protocol_version,
            system_state_version: genesis_system_state_version(),
            iota_treasury_cap,
            validators,
            storage_fund,
            parameters: SystemParametersV1 {
                epoch_duration_ms,
                extra_fields: bag::new(ctx),
            },
            reference_gas_price: 1,
            safe_mode: false,
            epoch_start_timestamp_ms,
            extra_fields: bag::new(ctx),
        };
        // Add a dummy inactive validator so that we could test validator upgrade through wrapper latter.
        add_dummy_inactive_validator_for_testing(&mut system_state, ctx);
        system_state
    }

    public(package) fun advance_epoch(
        self: &mut IotaSystemStateV1,
        new_epoch: u64,
        next_protocol_version: u64,
        _validator_target_reward: u64,
        mut storage_charge: Balance<IOTA>,
        mut computation_reward: Balance<IOTA>,
        mut storage_rebate_amount: u64,
        mut _non_refundable_storage_fee_amount: u64,
        _reward_slashing_rate: u64,
        epoch_start_timestamp_ms: u64,
        _ctx: &mut TxContext,
    ) : Balance<IOTA> {
        self.epoch_start_timestamp_ms = epoch_start_timestamp_ms;
        self.epoch = self.epoch + 1;
        assert!(new_epoch == self.epoch, 0);
        self.safe_mode = false;
        self.protocol_version = next_protocol_version;

        let storage_charge_value = storage_charge.value();
        let total_gas_fees = computation_reward.value();

        balance::join(&mut self.storage_fund, computation_reward);
        balance::join(&mut self.storage_fund, storage_charge);
        let storage_rebate = balance::split(&mut self.storage_fund, storage_rebate_amount);

        event::emit(
            SystemEpochInfoEventV1 {
                epoch: self.epoch,
                protocol_version: self.protocol_version,
                reference_gas_price: self.reference_gas_price,
                total_stake: 0,
                storage_charge: storage_charge_value,
                storage_rebate: storage_rebate_amount,
                storage_fund_balance: self.storage_fund.value(),
                total_gas_fees,
                total_stake_rewards_distributed: 0,
                burnt_tokens_amount: 0,
                minted_tokens_amount: 0,
            }
        );

        storage_rebate
    }

    public(package) fun protocol_version(self: &IotaSystemStateV1): u64 { self.protocol_version }
    public(package) fun system_state_version(self: &IotaSystemStateV1): u64 { self.system_state_version }
    public(package) fun genesis_system_state_version(): u64 {
        SYSTEM_STATE_VERSION_V1
    }

    public(package) fun add_dummy_inactive_validator_for_testing(self: &mut IotaSystemStateV1, ctx: &mut TxContext) {
        // Add a new entry to the inactive validator table for upgrade testing.
        let dummy_inactive_validator = validator_wrapper::create_v1(
            validator::new_dummy_inactive_validator(ctx),
            ctx,
        );
        table::add(&mut self.validators.inactive_validators, object::id_from_address(@0x0), dummy_inactive_validator);
    }

    fun new_validator_set(init_active_validators: vector<ValidatorV1>, ctx: &mut TxContext): ValidatorSetV1 {
        ValidatorSetV1 {
            active_validators: init_active_validators,
            inactive_validators: table::new(ctx),
            extra_fields: bag::new(ctx),
        }
    }
}
