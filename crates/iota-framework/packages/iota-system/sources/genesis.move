// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::genesis {

    use std::string::String;

    use iota::balance::{Self, Balance};
    use iota::iota::{Self, IOTA};
    use iota::timelock::SystemTimelockCap;
    use iota_system::iota_system;
    use iota_system::validator::{Self, Validator};
    use iota_system::validator_set;
    use iota_system::iota_system_state_inner;
    use iota_system::stake_subsidy;
    use iota_system::timelocked_staking;

    public struct GenesisValidatorMetadata has drop, copy {
        name: vector<u8>,
        description: vector<u8>,
        image_url: vector<u8>,
        project_url: vector<u8>,

        iota_address: address,

        gas_price: u64,
        commission_rate: u64,

        protocol_public_key: vector<u8>,
        proof_of_possession: vector<u8>,

        network_public_key: vector<u8>,
        worker_public_key: vector<u8>,

        network_address: vector<u8>,
        p2p_address: vector<u8>,
        primary_address: vector<u8>,
        worker_address: vector<u8>,
    }

    public struct GenesisChainParameters has drop, copy {
        protocol_version: u64,
        chain_start_timestamp_ms: u64,
        epoch_duration_ms: u64,

        // Stake Subsidy parameters
        stake_subsidy_start_epoch: u64,
        stake_subsidy_initial_distribution_amount: u64,
        stake_subsidy_period_length: u64,
        stake_subsidy_decrease_rate: u16,

        // Validator committee parameters
        max_validator_count: u64,
        min_validator_joining_stake: u64,
        validator_low_stake_threshold: u64,
        validator_very_low_stake_threshold: u64,
        validator_low_stake_grace_period: u64,
    }

    public struct TokenDistributionSchedule {
        stake_subsidy_fund_nanos: u64,
        allocations: vector<TokenAllocation>,
    }

    public struct TokenAllocation {
        recipient_address: address,
        amount_nanos: u64,

        /// Indicates if this allocation should be staked at genesis and with which validator
        staked_with_validator: Option<address>,
        /// Indicates if this allocation should be staked with timelock at genesis
        /// and contains its timelock_expiration
        staked_with_timelock_expiration: Option<u64>,
    }

    // Error codes
    /// The `create` function was called at a non-genesis epoch.
    const ENotCalledAtGenesis: u64 = 0;
    /// The `create` function was called with duplicate validators.
    const EDuplicateValidator: u64 = 1;

    #[allow(unused_function)]
    /// This function will be explicitly called once at genesis.
    /// It will create a singleton IotaSystemState object, which contains
    /// all the information we need in the system.
    fun create(
        iota_system_state_id: UID,
        mut iota_supply: Balance<IOTA>,
        genesis_chain_parameters: GenesisChainParameters,
        genesis_validators: vector<GenesisValidatorMetadata>,
        token_distribution_schedule: TokenDistributionSchedule,
        timelock_genesis_label: Option<String>,
        system_timelock_cap: SystemTimelockCap,
        ctx: &mut TxContext,
    ) {
        // Ensure this is only called at genesis
        assert!(ctx.epoch() == 0, ENotCalledAtGenesis);

        let TokenDistributionSchedule {
            stake_subsidy_fund_nanos,
            allocations,
        } = token_distribution_schedule;

        let subsidy_fund = iota_supply.split(stake_subsidy_fund_nanos);
        let storage_fund = balance::zero();

        // Create all the `Validator` structs
        let mut validators = vector[];
        let count = genesis_validators.length();
        let mut i = 0;
        while (i < count) {
            let GenesisValidatorMetadata {
                name,
                description,
                image_url,
                project_url,
                iota_address,
                gas_price,
                commission_rate,
                protocol_public_key,
                proof_of_possession,
                network_public_key,
                worker_public_key,
                network_address,
                p2p_address,
                primary_address,
                worker_address,
            } = genesis_validators[i];

            let validator = validator::new(
                iota_address,
                protocol_public_key,
                network_public_key,
                worker_public_key,
                proof_of_possession,
                name,
                description,
                image_url,
                project_url,
                network_address,
                p2p_address,
                primary_address,
                worker_address,
                gas_price,
                commission_rate,
                ctx
            );

            // Ensure that each validator is unique
            assert!(
                !validator_set::is_duplicate_validator(&validators, &validator),
                EDuplicateValidator,
            );

            validators.push_back(validator);

            i = i + 1;
        };

        // Allocate tokens and staking operations
        allocate_tokens(
            iota_supply,
            allocations,
            &mut validators,
            timelock_genesis_label,
            ctx
        );

        // Activate all validators
        activate_validators(&mut validators);

        let system_parameters = iota_system_state_inner::create_system_parameters(
            genesis_chain_parameters.epoch_duration_ms,
            genesis_chain_parameters.stake_subsidy_start_epoch,

            // Validator committee parameters
            genesis_chain_parameters.max_validator_count,
            genesis_chain_parameters.min_validator_joining_stake,
            genesis_chain_parameters.validator_low_stake_threshold,
            genesis_chain_parameters.validator_very_low_stake_threshold,
            genesis_chain_parameters.validator_low_stake_grace_period,

            ctx,
        );

        let stake_subsidy = stake_subsidy::create(
            subsidy_fund,
            genesis_chain_parameters.stake_subsidy_initial_distribution_amount,
            genesis_chain_parameters.stake_subsidy_period_length,
            genesis_chain_parameters.stake_subsidy_decrease_rate,
            ctx,
        );

        iota_system::create(
            iota_system_state_id,
            validators,
            storage_fund,
            genesis_chain_parameters.protocol_version,
            genesis_chain_parameters.chain_start_timestamp_ms,
            system_parameters,
            stake_subsidy,
            system_timelock_cap,
            ctx,
        );
    }

    fun allocate_tokens(
        mut iota_supply: Balance<IOTA>,
        mut allocations: vector<TokenAllocation>,
        validators: &mut vector<Validator>,
        timelock_genesis_label: Option<String>,
        ctx: &mut TxContext,
    ) {

        while (!allocations.is_empty()) {
            let TokenAllocation {
                recipient_address,
                amount_nanos,
                staked_with_validator,
                staked_with_timelock_expiration,
            } = allocations.pop_back();

            let allocation_balance = iota_supply.split(amount_nanos);

            if (staked_with_validator.is_some()) {
                let validator_address = staked_with_validator.destroy_some();
                let validator = validator_set::get_validator_mut(
                    validators, validator_address
                );
                if (staked_with_timelock_expiration.is_some()) {
                    let timelock_expiration = staked_with_timelock_expiration.destroy_some();
                    timelocked_staking::request_add_stake_at_genesis(
                        validator,
                        allocation_balance,
                        recipient_address,
                        timelock_expiration,
                        timelock_genesis_label,
                        ctx
                    );
                } else {
                    validator.request_add_stake_at_genesis(
                        allocation_balance,
                        recipient_address,
                        ctx
                    );
                }
            } else {
                iota::transfer(
                    allocation_balance.into_coin(ctx),
                    recipient_address,
                );
            };
        };
        allocations.destroy_empty();

        // Provided allocations must fully allocate the iota_supply and there
        // should be none left at this point.
        iota_supply.destroy_zero();
    }

    fun activate_validators(validators: &mut vector<Validator>) {
        // Activate all genesis validators
        let count = validators.length();
        let mut i = 0;
        while (i < count) {
            let validator = &mut validators[i];
            validator.activate(0);

            i = i + 1;
        };

    }
}
