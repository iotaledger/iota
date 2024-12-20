// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use camino::Utf8PathBuf;
use iota_config::local_ip_utils;
use iota_genesis_builder::{Builder, validator_info::ValidatorInfo};
use iota_types::{
    base_types::IotaAddress,
    crypto::{
        AccountKeyPair, AuthorityKeyPair, KeypairTraits, NetworkKeyPair,
        generate_proof_of_possession, get_key_pair_from_rng,
    },
};

#[tokio::main]
async fn main() {
    let dir = std::env::current_dir().unwrap();
    let dir = Utf8PathBuf::try_from(dir).unwrap();

    let mut builder = Builder::new();
    let mut keys = Vec::new();
    for i in 0..2 {
        let authority_key: AuthorityKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let protocol_key: NetworkKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let account_key: AccountKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let network_key: NetworkKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let validator = ValidatorInfo {
            name: format!("Validator {}", i),
            authority_key: authority_key.public().into(),
            protocol_key: protocol_key.public().clone(),
            account_address: IotaAddress::from(account_key.public()),
            network_key: network_key.public().clone(),
            gas_price: iota_config::node::DEFAULT_VALIDATOR_GAS_PRICE,
            commission_rate: iota_config::node::DEFAULT_COMMISSION_RATE,
            network_address: local_ip_utils::new_local_tcp_address_for_testing(),
            p2p_address: local_ip_utils::new_local_udp_address_for_testing(),
            primary_address: local_ip_utils::new_local_tcp_address_for_testing(),
            description: String::new(),
            image_url: String::new(),
            project_url: String::new(),
        };
        let pop = generate_proof_of_possession(&authority_key, account_key.public().into());
        keys.push(authority_key);
        builder = builder.add_validator(validator, pop);
    }

    for key in keys {
        builder = builder.add_validator_signature(&key);
    }

    builder.save(dir).unwrap();
}
