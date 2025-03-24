// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::iota_system_state_inner {
    use iota::bag::{Self, Bag};
    use iota::balance::{Self, Balance};
    use iota::event;
    use iota::iota::IOTA;
    use iota::iota::IotaTreasuryCap;
    use iota::system_admin_cap::IotaSystemAdminCap;
    use iota::table::{Self, Table};

    use iota_system::validator::{ValidatorV1, ValidatorV2};
    use iota_system::validator_wrapper::Validator;
    use iota_system::validator_wrapper;
    use iota_system::validator;

    const SYSTEM_STATE_VERSION_V1: u64 = 18446744073709551605;  // u64::MAX - 10
        // Not using MAX - 9 since it's already used in the shallow upgrade test.
    const SYSTEM_STATE_VERSION_V2: u64 = 18446744073709551607;  // u64::MAX - 8

    public struct SystemEpochInfoEventV2 has copy, drop {
        epoch: u64,
        protocol_version: u64,
        total_stake: u64,
        storage_charge: u64,
        storage_rebate: u64,
        storage_fund_balance: u64,
        total_gas_fees: u64,
        total_stake_rewards_distributed: u64,
        burnt_tokens_amount: u64,
        minted_tokens_amount: u64,
        tips_amount: u64,
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

    public struct ValidatorSetV2 has store {
        active_validators: vector<ValidatorV2>,
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
        iota_system_admin_cap: IotaSystemAdminCap,
        reference_gas_price: u64,
        safe_mode: bool,
        epoch_start_timestamp_ms: u64,
        extra_fields: Bag,
    }

    public struct IotaSystemStateV2 has store {
        new_dummy_field: u64,
        epoch: u64,
        protocol_version: u64,
        system_state_version: u64,
        iota_treasury_cap: IotaTreasuryCap,
        validators: ValidatorSetV2,
        storage_fund: Balance<IOTA>,
        parameters: SystemParametersV1,
        iota_system_admin_cap: IotaSystemAdminCap,
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
        iota_system_admin_cap: IotaSystemAdminCap,
        ctx: &mut TxContext,
    ): IotaSystemStateV1 {
        let validators = new_validator_set(validators, ctx);
        let system_state = IotaSystemStateV1 {
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
            iota_system_admin_cap,
            reference_gas_price: 1,
            safe_mode: false,
            epoch_start_timestamp_ms,
            extra_fields: bag::new(ctx),
        };
        system_state
    }

    public(package) fun advance_epoch(
        self: &mut IotaSystemStateV2,
        new_epoch: u64,
        next_protocol_version: u64,
        _validator_subsidy: u64,
        mut storage_charge: Balance<IOTA>,
        mut computation_charge: Balance<IOTA>,
        mut _computation_charge_burned: u64,
        mut storage_rebate_amount: u64,
        mut _non_refundable_storage_fee_amount: u64,
        _reward_slashing_rate: u64,
        epoch_start_timestamp_ms: u64,
        _max_committee_members_count: u64,
        _ctx: &mut TxContext,
    ) : Balance<IOTA> {
        touch_dummy_inactive_validator(self);

        self.epoch_start_timestamp_ms = epoch_start_timestamp_ms;
        self.epoch = self.epoch + 1;
        assert!(new_epoch == self.epoch, 0);
        self.safe_mode = false;
        self.protocol_version = next_protocol_version;

        let storage_charge_value = storage_charge.value();
        let total_gas_fees = computation_charge.value();

        balance::join(&mut self.storage_fund, computation_charge);
        balance::join(&mut self.storage_fund, storage_charge);
        let storage_rebate = balance::split(&mut self.storage_fund, storage_rebate_amount);

        event::emit(
            SystemEpochInfoEventV2 {
                epoch: self.epoch,
                protocol_version: self.protocol_version,
                total_stake: 0,
                storage_charge: storage_charge_value,
                storage_rebate: storage_rebate_amount,
                storage_fund_balance: self.storage_fund.value(),
                total_gas_fees,
                total_stake_rewards_distributed: 0,
                burnt_tokens_amount: 0,
                minted_tokens_amount: 0,
                tips_amount: 0,
            }
        );

        storage_rebate
    }

    public(package) fun protocol_version(self: &IotaSystemStateV2): u64 { self.protocol_version }
    public(package) fun system_state_version(self: &IotaSystemStateV2): u64 { self.system_state_version }
    public(package) fun genesis_system_state_version(): u64 {
        SYSTEM_STATE_VERSION_V1
    }

    fun new_validator_set(init_active_validators: vector<ValidatorV1>, ctx: &mut TxContext): ValidatorSetV1 {
        ValidatorSetV1 {
            active_validators: init_active_validators,
            inactive_validators: table::new(ctx),
            extra_fields: bag::new(ctx),
        }
    }

    public(package) fun v1_to_v2(v1: IotaSystemStateV1): IotaSystemStateV2 {
        let IotaSystemStateV1 {
            epoch,
            protocol_version,
            system_state_version: old_system_state_version,
            iota_treasury_cap,
            validators,
            storage_fund,
            parameters,
            iota_system_admin_cap,
            reference_gas_price,
            safe_mode,
            epoch_start_timestamp_ms,
            extra_fields,
        } = v1;
        let new_validator_set = validator_set_v1_to_v2(validators);
        assert!(old_system_state_version == SYSTEM_STATE_VERSION_V1, 0);
        IotaSystemStateV2 {
            new_dummy_field: 100,
            epoch,
            protocol_version,
            system_state_version: SYSTEM_STATE_VERSION_V2,
            iota_treasury_cap,
            validators: new_validator_set,
            storage_fund,
            parameters,
            iota_system_admin_cap,
            reference_gas_price,
            safe_mode,
            epoch_start_timestamp_ms,
            extra_fields,
        }
    }

    /// Load the dummy inactive validator added in the base version, trigger it to be upgraded.
    fun touch_dummy_inactive_validator(self: &mut IotaSystemStateV2) {
        let validator_wrapper = table::borrow_mut(&mut self.validators.inactive_validators, object::id_from_address(@0x0));
        let _ = validator_wrapper::load_validator_maybe_upgrade(validator_wrapper);
    }

    fun validator_set_v1_to_v2(v1: ValidatorSetV1): ValidatorSetV2 {
        let ValidatorSetV1 {
            mut active_validators,
            inactive_validators,
            extra_fields,
        } = v1;
        let mut new_active_validators = vector[];
        while (!vector::is_empty(&active_validators)) {
            let validator = vector::pop_back(&mut active_validators);
            let validator = validator::v1_to_v2(validator);
            vector::push_back(&mut new_active_validators, validator);
        };
        vector::destroy_empty(active_validators);
        vector::reverse(&mut new_active_validators);
        ValidatorSetV2 {
            active_validators: new_active_validators,
            inactive_validators,
            extra_fields,
        }
    }
}
