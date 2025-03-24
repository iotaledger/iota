// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectID},
    supported_protocol_versions::Chain,
};
use serde::{Deserialize, Serialize};

use crate::Domain;

pub const LEAF_EXPIRATION_TIMESTAMP: u64 = 0;
pub const DEFAULT_TLD: &str = "iota";
pub const ACCEPTED_SEPARATORS: [char; 2] = ['.', '*'];
pub const IOTA_NEW_FORMAT_SEPARATOR: char = '@';

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct IotaNamesConfig {
    pub package_address: IotaAddress,
    pub object_id: ObjectID,
    pub payment_package_address: IotaAddress,
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
        payment_package_address: IotaAddress,
        registry_id: ObjectID,
        reverse_registry_id: ObjectID,
    ) -> Self {
        Self {
            package_address,
            object_id,
            payment_package_address,
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
        const PACKAGE_ADDRESS: &str =
            "0x29ab1b20537b620febdb8bf1f93fcfb32d8024e96f4bd6661688f5ab2d912638";
        const OBJECT_ID: &str =
            "0x7a223639d9fbb6aa121cc4bd110c92a282ebe07b64de3a9f5e600174a8c90be8";
        const PAYMENT_PACKAGE_ADDRESS: &str =
            "0xd0c72a25021bb06b3b998c54d5833b62ebc37098293086144902e1ec07ffb168";
        const REGISTRY_ID: &str =
            "0x38c633815b6819b96a11b0887148e278778e3db92e271c9fa24673e22cd3b1ab";
        const REVERSE_REGISTRY_ID: &str =
            "0xf1cb010ece093927fa73e09c00a8f3f054f04ce2b881b648867fbf9efda82258";

        let package_address = IotaAddress::from_str(PACKAGE_ADDRESS).unwrap();
        let object_id = ObjectID::from_str(OBJECT_ID).unwrap();
        let payment_package_address = IotaAddress::from_str(PAYMENT_PACKAGE_ADDRESS).unwrap();
        let registry_id = ObjectID::from_str(REGISTRY_ID).unwrap();
        let reverse_registry_id = ObjectID::from_str(REVERSE_REGISTRY_ID).unwrap();

        Self::new(
            package_address,
            object_id,
            payment_package_address,
            registry_id,
            reverse_registry_id,
        )
    }
}
