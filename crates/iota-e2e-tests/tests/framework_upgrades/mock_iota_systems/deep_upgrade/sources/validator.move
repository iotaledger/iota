// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::validator {
    use std::ascii;

    use std::string::{Self, String};
    use iota::bag::{Self, Bag};
    use iota::balance::{Self, Balance};
    use iota::iota::IOTA;

    public struct ValidatorMetadataV1 has store {
        iota_address: address,
        authority_pubkey_bytes: vector<u8>,
        network_pubkey_bytes: vector<u8>,
        protocol_pubkey_bytes: vector<u8>,
        net_address: String,
        p2p_address: String,
        primary_address: String,
        extra_fields: Bag,
    }

    public struct ValidatorV1 has store {
        metadata: ValidatorMetadataV1,
        voting_power: u64,
        stake: Balance<IOTA>,
        extra_fields: Bag,
    }

    public struct ValidatorV2 has store {
        new_dummy_field: u64,
        metadata: ValidatorMetadataV1,
        voting_power: u64,
        stake: Balance<IOTA>,
        extra_fields: Bag,
    }

    public(package) fun new(
        iota_address: address,
        authority_pubkey_bytes: vector<u8>,
        network_pubkey_bytes: vector<u8>,
        protocol_pubkey_bytes: vector<u8>,
        net_address: vector<u8>,
        p2p_address: vector<u8>,
        primary_address: vector<u8>,
        init_stake: Balance<IOTA>,
        ctx: &mut TxContext
    ): ValidatorV1 {
        let metadata = ValidatorMetadataV1 {
            iota_address,
            authority_pubkey_bytes,
            network_pubkey_bytes,
            protocol_pubkey_bytes,
            net_address: string::from_ascii(ascii::string(net_address)),
            p2p_address: string::from_ascii(ascii::string(p2p_address)),
            primary_address: string::from_ascii(ascii::string(primary_address)),
            extra_fields: bag::new(ctx),
        };

        ValidatorV1 {
            metadata,
            voting_power: balance::value(&init_stake),
            stake: init_stake,
            extra_fields: bag::new(ctx),
        }
    }

    public(package) fun v1_to_v2(v1: ValidatorV1): ValidatorV2 {
        let ValidatorV1 {
            metadata,
            voting_power,
            stake,
            extra_fields,
        } = v1;
        ValidatorV2 {
            new_dummy_field: 100,
            metadata,
            voting_power,
            stake,
            extra_fields,
        }
    }
}
