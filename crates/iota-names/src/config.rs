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

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct IotaNamesConfig {
    /// Address of the `iota_names` package.
    pub package_address: IotaAddress,
    /// ID of the `IotaNames` object.
    pub object_id: ObjectID,
    /// Address of the `payments` package.
    pub payments_package_address: IotaAddress,
    /// ID of the registry table.
    pub registry_id: ObjectID,
    /// ID of the reverse registry table.
    pub reverse_registry_id: ObjectID,
    /// Address of the `subdomain_proxy` package.
    pub subdomain_proxy_package_id: ObjectID,
}

impl Default for IotaNamesConfig {
    fn default() -> Self {
        // TODO change to mainnet https://github.com/iotaledger/iota/issues/6532
        // TODO change to testnet https://github.com/iotaledger/iota/issues/6531
        Self::devnet()
    }
}

impl IotaNamesConfig {
    pub fn new(
        package_address: IotaAddress,
        object_id: ObjectID,
        payments_package_address: IotaAddress,
        registry_id: ObjectID,
        reverse_registry_id: ObjectID,
        subdomain_proxy_package_id: ObjectID,
    ) -> Self {
        Self {
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
            subdomain_proxy_package_id,
        }
    }

    pub fn from_chain(chain: &Chain) -> Self {
        match chain {
            Chain::Mainnet => todo!("https://github.com/iotaledger/iota/issues/6532"),
            Chain::Testnet => todo!("https://github.com/iotaledger/iota/issues/6531"),
            Chain::Unknown => IotaNamesConfig::devnet(),
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

    // TODO add mainnet https://github.com/iotaledger/iota/issues/6532

    // TODO add testnet https://github.com/iotaledger/iota/issues/6531

    // Create a config based on the package and object ids published on devnet.
    pub fn devnet() -> Self {
        const PACKAGE_ADDRESS: &str =
            "0x8d65e0a24ce12487615928c0abca3f016404c7b5db243bb038943e4a2f91cc3e";
        const OBJECT_ID: &str =
            "0xdad5289ef0d64f8f3b4d72522907f3f67109fa00bfbcba2dd03c68084f1dfc89";
        const PAYMENTS_PACKAGE_ADDRESS: &str =
            "0x8e1d3fafb70764eccc2e6b61812daf0a4db40db3c5cea515bf4d390f11016030";
        const REGISTRY_ID: &str =
            "0x5759205b20dcce68d2630cbadd839beebaa5e267444e89a49bca4648b48a74a2";
        const REVERSE_REGISTRY_ID: &str =
            "0x5d5d6662c71d116b16bdf9ed4f5a99647bd1186bfd6ce9ddb882511a59883905";
        const SUBDOMAIN_PROXY_PACKAGE_ID: &str =
            "0x9d289df7ba7154940c8508a5836590fd0e5e41cb710faa0d8210bdc2f274441e";

        let package_address = IotaAddress::from_str(PACKAGE_ADDRESS).unwrap();
        let object_id = ObjectID::from_str(OBJECT_ID).unwrap();
        let payments_package_address = IotaAddress::from_str(PAYMENTS_PACKAGE_ADDRESS).unwrap();
        let registry_id = ObjectID::from_str(REGISTRY_ID).unwrap();
        let reverse_registry_id = ObjectID::from_str(REVERSE_REGISTRY_ID).unwrap();
        let subdomain_proxy_package_id = ObjectID::from_str(SUBDOMAIN_PROXY_PACKAGE_ID).unwrap();

        Self::new(
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
            subdomain_proxy_package_id,
        )
    }
}
