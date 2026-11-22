// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use iota_sdk_types::iota_names::config::*;
use iota_sdk_types::{Address, ObjectId, StructTag, TypeTag};
use iota_types::supported_protocol_versions::Chain;

use crate::name::Name;

mod iota_names_config_ext {
    pub trait Sealed {}
    impl Sealed for super::IotaNamesConfig {}
}

pub trait IotaNamesConfigExt: iota_names_config_ext::Sealed {
    fn from_chain(chain: &Chain) -> Self;
    fn record_field_id(&self, name: &Name) -> ObjectId;
    fn reverse_record_field_id(&self, address: &Address) -> ObjectId;
}

impl IotaNamesConfigExt for IotaNamesConfig {
    fn from_chain(chain: &Chain) -> Self {
        match chain {
            Chain::Mainnet => IotaNamesConfig::mainnet(),
            Chain::Testnet => IotaNamesConfig::testnet(),
            Chain::Unknown => IotaNamesConfig::devnet(),
        }
    }

    fn record_field_id(&self, name: &Name) -> ObjectId {
        let name_type_tag = StructTag::new_name(self.package_address);
        let name_bytes = bcs::to_bytes(name).unwrap();

        iota_types::dynamic_field::derive_dynamic_field_id(
            self.registry_id,
            &TypeTag::Struct(Box::new(name_type_tag)),
            &name_bytes,
        )
        .unwrap()
    }

    fn reverse_record_field_id(&self, address: &Address) -> ObjectId {
        iota_types::dynamic_field::derive_dynamic_field_id(
            self.reverse_registry_id,
            &TypeTag::Address,
            address.as_ref(),
        )
        .unwrap()
    }
}
