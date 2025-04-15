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
    pub subdomain_proxy_package_id: ObjectID,
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
        subdomain_proxy_package_id: ObjectID,
    ) -> Self {
        Self {
            package_address,
            object_id,
            payment_package_address,
            registry_id,
            reverse_registry_id,
            subdomain_proxy_package_id,
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
            "0xe27899d691184f66821f8fed5e7c26f3c65b26921947956435a655c8d7efc573";
        const OBJECT_ID: &str =
            "0xdad5289ef0d64f8f3b4d72522907f3f67109fa00bfbcba2dd03c68084f1dfc89";
        const PAYMENT_PACKAGE_ADDRESS: &str =
            "0x8e1d3fafb70764eccc2e6b61812daf0a4db40db3c5cea515bf4d390f11016030";
        const REGISTRY_ID: &str =
            "0xff608b2b0d500b4d0cb25ff165bc3e01fce9bf3ef7fb002840b814d304a08b2a";
        const REVERSE_REGISTRY_ID: &str =
            "0x1c2eddd6c4f7510b35a9de575d9ccb1ad640de6aa3a5626937c21c9c62beaeed";
        const SUBDOMAIN_PROXY_PACKAGE_ID: &str =
            "0xf43e05a098dd8a339d478907418f42b30eddf661b029a48f313edee1420e22fe";

        let package_address = IotaAddress::from_str(PACKAGE_ADDRESS).unwrap();
        let object_id = ObjectID::from_str(OBJECT_ID).unwrap();
        let payment_package_address = IotaAddress::from_str(PAYMENT_PACKAGE_ADDRESS).unwrap();
        let registry_id = ObjectID::from_str(REGISTRY_ID).unwrap();
        let reverse_registry_id = ObjectID::from_str(REVERSE_REGISTRY_ID).unwrap();
        let subdomain_proxy_package_id = ObjectID::from_str(SUBDOMAIN_PROXY_PACKAGE_ID).unwrap();

        Self::new(
            package_address,
            object_id,
            payment_package_address,
            registry_id,
            reverse_registry_id,
            subdomain_proxy_package_id,
        )
    }
}
