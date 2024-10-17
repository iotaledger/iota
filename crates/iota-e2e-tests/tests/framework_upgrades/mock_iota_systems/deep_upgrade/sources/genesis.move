// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::genesis {
    use std::vector;
    use iota::balance::{Self, Balance};
    use iota::object::UID;
    use iota::iota::IOTA;
    use iota::tx_context::{Self, TxContext};
    use std::option::Option;

    use iota_system::iota_system;
    use iota_system::validator;

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

        max_validator_count: u64,
        min_validator_joining_stake: u64,
        validator_low_stake_threshold: u64,
        validator_very_low_stake_threshold: u64,
        validator_low_stake_grace_period: u64,
    }
      
    public struct TokenDistributionSchedule has drop {
        pre_minted_supply: u64,
        allocations: vector<TokenAllocation>,
    }

    public struct TokenAllocation has drop {
        recipient_address: address,
        amount_nanos: u64,
        staked_with_validator: Option<address>,
    }

    fun create(
        iota_system_state_id: UID,
        mut iota_supply: Balance<IOTA>,
        genesis_chain_parameters: GenesisChainParameters,
        genesis_validators: vector<GenesisValidatorMetadata>,
        _token_distribution_schedule: TokenDistributionSchedule,
        ctx: &mut TxContext,
    ) {
        assert!(tx_context::epoch(ctx) == 0, 0);

        let mut validators = vector::empty();
        let count = vector::length(&genesis_validators);
        let mut i = 0;
        while (i < count) {
            let GenesisValidatorMetadata {
                name: _,
                description: _,
                image_url: _,
                project_url: _,
                iota_address,
                gas_price: _,
                commission_rate: _,
                protocol_public_key,
                proof_of_possession: _,
                network_public_key,
                worker_public_key,
                network_address,
                p2p_address,
                primary_address,
                worker_address,
            } = *vector::borrow(&genesis_validators, i);

            let validator = validator::new(
                iota_address,
                protocol_public_key,
                network_public_key,
                worker_public_key,
                network_address,
                p2p_address,
                primary_address,
                worker_address,
                balance::split(&mut iota_supply, 2500),
                ctx
            );

            vector::push_back(&mut validators, validator);

            i = i + 1;
        };

        iota_system::create(
            iota_system_state_id,
            validators,
            iota_supply, // storage_fund
            genesis_chain_parameters.protocol_version,
            genesis_chain_parameters.chain_start_timestamp_ms,
            genesis_chain_parameters.epoch_duration_ms,
            ctx,
        );
    }
}
