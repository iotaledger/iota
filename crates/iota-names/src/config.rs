// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectID},
    supported_protocol_versions::Chain,
};
use serde::{Deserialize, Serialize};

use crate::Name;

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
}

impl Default for IotaNamesConfig {
    fn default() -> Self {
        Self::mainnet()
    }
}

impl IotaNamesConfig {
    pub fn new(
        package_address: IotaAddress,
        object_id: ObjectID,
        payments_package_address: IotaAddress,
        registry_id: ObjectID,
        reverse_registry_id: ObjectID,
    ) -> Self {
        Self {
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
        }
    }

    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::new(
            std::env::var("IOTA_NAMES_PACKAGE_ADDRESS")?.parse()?,
            std::env::var("IOTA_NAMES_OBJECT_ID")?.parse()?,
            std::env::var("IOTA_NAMES_PAYMENTS_PACKAGE_ADDRESS")?.parse()?,
            std::env::var("IOTA_NAMES_REGISTRY_ID")?.parse()?,
            std::env::var("IOTA_NAMES_REVERSE_REGISTRY_ID")?.parse()?,
        ))
    }

    pub fn from_chain(chain: &Chain) -> Self {
        match chain {
            Chain::Mainnet => IotaNamesConfig::mainnet(),
            Chain::Testnet => IotaNamesConfig::testnet(),
            Chain::Unknown => IotaNamesConfig::devnet(),
        }
    }

    pub fn record_field_id(&self, name: &Name) -> ObjectID {
        let name_type_tag = Name::type_(self.package_address);
        let name_bytes = bcs::to_bytes(name).unwrap();

        iota_types::dynamic_field::derive_dynamic_field_id(
            self.registry_id,
            &TypeTag::Struct(Box::new(name_type_tag)),
            &name_bytes,
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

    // Create a config based on the package and object ids published on mainnet.
    pub fn mainnet() -> Self {
        const PACKAGE_ADDRESS: &str =
            "0x6d2c743607ef275bd6934fe5c2a7e5179cca6fbd2049cfa79de2310b74f3cf83";
        const OBJECT_ID: &str =
            "0xa14e5d0481a7aa346157078e6facba3cd895d97038cd87b9f2cc24b0c6102d75";
        const PAYMENTS_PACKAGE_ADDRESS: &str =
            "0x53d3d37f00949a1baad95fa4fca0b3d0d70ff6121be316f9e46d37c2d29c71eb";
        const REGISTRY_ID: &str =
            "0xa773cef7d762871354f6ae19ad174dfb1153d2d247c4886ada0b5330b9543b57";
        const REVERSE_REGISTRY_ID: &str =
            "0x18fa62ab8b0ab95ae61088082bd5db796863016fda8f3205b1ea7d13b1792317";

        let package_address = IotaAddress::from_str(PACKAGE_ADDRESS).unwrap();
        let object_id = ObjectID::from_str(OBJECT_ID).unwrap();
        let payments_package_address = IotaAddress::from_str(PAYMENTS_PACKAGE_ADDRESS).unwrap();
        let registry_id = ObjectID::from_str(REGISTRY_ID).unwrap();
        let reverse_registry_id = ObjectID::from_str(REVERSE_REGISTRY_ID).unwrap();

        Self::new(
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
        )
    }

    // Create a config based on the package and object ids published on testnet.
    pub fn testnet() -> Self {
        const PACKAGE_ADDRESS: &str =
            "0x7fff6e95f385349bec98d17121ab2bfa3e134f2f0b1ccefc270313415f7835ea";
        const OBJECT_ID: &str =
            "0x7cab491740d51e0d75b26bf9984e49ba2e32a2d0694cabcee605543ed13c7dec";
        const PAYMENTS_PACKAGE_ADDRESS: &str =
            "0x6b1b01f4c72786a893191d5c6e73d3012f7529f86fdee3bc8c163323cee08441";
        const REGISTRY_ID: &str =
            "0x2dfc6f6d46ba55217425643a59dc85fe4d8ed273a9f74077bd0ee280dbb4f590";
        const REVERSE_REGISTRY_ID: &str =
            "0x3550bcacb793ef8b776264665e7c99fa3d897695ed664656aac693cf9cf9b76b";

        let package_address = IotaAddress::from_str(PACKAGE_ADDRESS).unwrap();
        let object_id = ObjectID::from_str(OBJECT_ID).unwrap();
        let payments_package_address = IotaAddress::from_str(PAYMENTS_PACKAGE_ADDRESS).unwrap();
        let registry_id = ObjectID::from_str(REGISTRY_ID).unwrap();
        let reverse_registry_id = ObjectID::from_str(REVERSE_REGISTRY_ID).unwrap();

        Self::new(
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
        )
    }

    // Create a config based on the package and object ids published on devnet.
    pub fn devnet() -> Self {
        const PACKAGE_ADDRESS: &str =
            "0x742d00d422294ca697c53662f571f8dc328296d62db2211e2bd05a1857c13e06";
        const OBJECT_ID: &str =
            "0x49ec1d51f532ba32f1b14d1794fdcd7727664587bde3fb65be31bd4eb7f32f21";
        const PAYMENTS_PACKAGE_ADDRESS: &str =
            "0xe14ccc7c77add03bb9b6ad902a9f92a470c6a25bf8a9793927b2678510bfbb31";
        const REGISTRY_ID: &str =
            "0x2e86c49747003e46be1691604e6c1fbf902b967e22452532de405647bff7af95";
        const REVERSE_REGISTRY_ID: &str =
            "0xad03947f9e0648b7cb85f8c8325ee95c58898cda5d21925184ed1e5f70a75cfb";

        let package_address = IotaAddress::from_str(PACKAGE_ADDRESS).unwrap();
        let object_id = ObjectID::from_str(OBJECT_ID).unwrap();
        let payments_package_address = IotaAddress::from_str(PAYMENTS_PACKAGE_ADDRESS).unwrap();
        let registry_id = ObjectID::from_str(REGISTRY_ID).unwrap();
        let reverse_registry_id = ObjectID::from_str(REVERSE_REGISTRY_ID).unwrap();

        Self::new(
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
        )
    }
}
