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
            "0xa1d2ed2008d31d358cfaf61a89aa7cfaa78ed183dbe683620258e98c59f48b13";
        const OBJECT_ID: &str =
            "0xbf3563622035af599057c46f4b871e0a9817c7bab759354532402be6d9538ba3";
        const PAYMENT_PACKAGE_ADDRESS: &str =
            "0xb06b8075797480a9bb660c927b666ca0301cdffa622e7c6b9c583bd2b45c781a";
        const REGISTRY_ID: &str =
            "0xd5e98aa3e79cff0cd5146dc4d7dea863eaffcce06703e47473f88214c4746501";
        const REVERSE_REGISTRY_ID: &str =
            "0xcafc893c3801416ffa4c262888eaa994e055d717d9b0819db3aef4ce35ab5829";
        const SUBDOMAIN_PROXY_PACKAGE_ID: &str =
            "0x11e01b25113cf141676d2f0b97068adbd2c98dd15ce1f52bd21c595faf63ec55";

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
