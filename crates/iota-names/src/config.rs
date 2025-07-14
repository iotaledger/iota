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
            Chain::Mainnet => todo!("https://github.com/iotaledger/iota/issues/6532"),
            Chain::Testnet => todo!("https://github.com/iotaledger/iota/issues/6531"),
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

    // TODO add mainnet https://github.com/iotaledger/iota/issues/6532

    // TODO add testnet https://github.com/iotaledger/iota/issues/6531

    // Create a config based on the package and object ids published on devnet.
    pub fn devnet() -> Self {
        const PACKAGE_ADDRESS: &str =
            "0xb9d617f24c84826bf660a2f4031951678cc80c264aebc4413459fb2a95ada9ba";
        const OBJECT_ID: &str =
            "0x07c59b37bd7d036bf78fa30561a2ab9f7a970837487656ec29466e817f879342";
        const PAYMENTS_PACKAGE_ADDRESS: &str =
            "0x98b9b33b7c2347a8f4e8b8716fb4c7e6e1af846ec2ea063a47bba81ffe03b440";
        const REGISTRY_ID: &str =
            "0xe00b2f2400c33b4dbd3081c4dcf2e289d0544caba23a3d130b264bd756403c07";
        const REVERSE_REGISTRY_ID: &str =
            "0x1c1da17843cc453ad4079b05ce55e103b7a8cdd4db6ab42dc367b47ed6d8994d";

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
