// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectID},
    supported_protocol_versions::Chain,
};
use move_core_types::{ident_str, identifier::IdentStr};
use serde::{Deserialize, Serialize};

use crate::Domain;

pub const NAME_SERVICE_DOMAIN_MODULE: &IdentStr = ident_str!("domain");
pub const NAME_SERVICE_DOMAIN_STRUCT: &IdentStr = ident_str!("Domain");
pub const LEAF_EXPIRATION_TIMESTAMP: u64 = 0;
pub const DEFAULT_TLD: &str = "iota";
pub const ACCEPTED_SEPARATORS: [char; 2] = ['.', '*'];
pub const IOTA_NEW_FORMAT_SEPARATOR: char = '@';

// TODO: merge to IotaNamesConfig or remove depending on https://github.com/iotaledger/iota-names/pull/75
// Devnet values
pub const REGISTRATION_PACKAGE: &str =
    "0x160581f35fb2a58a4964d513a96c70e0b64053a254936ae12b5f4d17087436f5";
pub const UTILS_PACKAGE: &str =
    "0xdea9e554fbee54e8dd0ac1d036d46047b5621b8f8739aa155258d656303af8cf";
pub const IOTA_FRAMEWORK: &str = "0x2";
pub const CLOCK_OBJECT_ID: &str = "0x6";

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct IotaNamesConfig {
    pub package_address: IotaAddress,
    pub object_id: ObjectID,
    pub registry_id: ObjectID,
    pub reverse_registry_id: ObjectID,
}

impl Default for IotaNamesConfig {
    fn default() -> Self {
        // TODO change to mainnet
        Self::devnet()
    }
}

impl IotaNamesConfig {
    pub fn new(
        package_address: IotaAddress,
        object_id: ObjectID,
        registry_id: ObjectID,
        reverse_registry_id: ObjectID,
    ) -> Self {
        Self {
            package_address,
            object_id,
            registry_id,
            reverse_registry_id,
        }
    }

    pub fn from_chain(chain: &Chain) -> Self {
        match chain {
            // Chain::Mainnet => IotaNamesConfig::mainnet(),
            // Chain::Testnet => IotaNamesConfig::testnet(),
            Chain::Unknown => IotaNamesConfig::devnet(),
            _ => todo!("uncomment Mainnet and Testnet when IOTA-Names is published there"),
        }
    }

    pub fn record_field_id(&self, domain: &Domain) -> ObjectID {
        let domain_type_tag = Domain::type_(self.package_address);
        let domain_bytes = bcs::to_bytes(domain).unwrap();

        iota_types::dynamic_field::derive_dynamic_field_id(
            self.registry_id,
            &TypeTag::Struct(Box::new(domain_type_tag)),
            &domain_bytes,
        )
        .unwrap()
    }

    pub fn reverse_record_field_id(&self, address: &IotaAddress) -> ObjectID {
        iota_types::dynamic_field::derive_dynamic_field_id(
            self.reverse_registry_id,
            &TypeTag::Address,
            address.as_ref(),
        )
        .unwrap()
    }

    // TODO add mainnet() and testnet()

    // Create a config based on the package and object ids published on devnet
    pub fn devnet() -> Self {
        const IOTA_NAMES_PACKAGE_ADDRESS: &str =
            "0x20c890da38609db67e2713e6b33b4e4d3c6a8e9f620f9bb48f918d2337e31503";
        const IOTA_NAMES_OBJECT_ID: &str =
            "0x55716ea4b9b7563537a1ef2705f1b06060b35f15f2ea00a20de29c547c319bef";
        const IOTA_NAMES_REGISTRY_ID: &str =
            "0xd48c0a882059036ca8c21dcc8d8bcaefc923aa678f225d3d515b79e3094e5616";
        const IOTA_NAMES_REVERSE_REGISTRY_ID: &str =
            "0x82139fa7c076816b67e2ff0927f2b30e4d6e2874a3a108649152a7b7d9eb25ac";

        let package_address = IotaAddress::from_str(IOTA_NAMES_PACKAGE_ADDRESS).unwrap();
        let object_id = ObjectID::from_str(IOTA_NAMES_OBJECT_ID).unwrap();
        let registry_id = ObjectID::from_str(IOTA_NAMES_REGISTRY_ID).unwrap();
        let reverse_registry_id = ObjectID::from_str(IOTA_NAMES_REVERSE_REGISTRY_ID).unwrap();

        Self::new(package_address, object_id, registry_id, reverse_registry_id)
    }
}
