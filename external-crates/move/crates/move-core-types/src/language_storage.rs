// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use once_cell::sync::Lazy;
#[cfg(any(test, feature = "fuzzing"))]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};

use crate::{
    account_address::{AccountAddress, address_abstract_size_for_gas_metering},
    gas_algebra::{AbstractMemorySize, BOX_ABSTRACT_SIZE, ENUM_BASE_ABSTRACT_SIZE},
    identifier::{IdentStr, Identifier, identifier_abstract_size_for_gas_metering},
    parsing::types::ParsedModuleId,
};

pub const CODE_TAG: u8 = 0;
pub const RESOURCE_TAG: u8 = 1;

/// Hex address: 0x1
pub const CORE_CODE_ADDRESS: AccountAddress = AccountAddress::STD;

/// Rough estimate of abstract size for TypeTag
pub static TYPETAG_ENUM_ABSTRACT_SIZE: Lazy<AbstractMemorySize> =
    Lazy::new(|| ENUM_BASE_ABSTRACT_SIZE + BOX_ABSTRACT_SIZE);

pub use iota_sdk_types::{StructTag, TypeTag};

/// Represents the initial key into global storage where we first index by the
/// address, and then the struct tag
#[derive(Serialize, Deserialize, Debug, PartialEq, Hash, Eq, Clone, PartialOrd, Ord)]
#[cfg_attr(any(test, feature = "fuzzing"), derive(Arbitrary))]
#[cfg_attr(any(test, feature = "fuzzing"), proptest(no_params))]
pub struct ModuleId {
    address: AccountAddress,
    name: Identifier,
}

impl From<ModuleId> for (AccountAddress, Identifier) {
    fn from(module_id: ModuleId) -> Self {
        (module_id.address, module_id.name)
    }
}

impl ModuleId {
    pub fn new(address: AccountAddress, name: Identifier) -> Self {
        ModuleId { address, name }
    }

    pub fn name(&self) -> &IdentStr {
        &self.name
    }

    pub fn address(&self) -> &AccountAddress {
        &self.address
    }

    pub fn access_vector(&self) -> Vec<u8> {
        let mut key = vec![CODE_TAG];
        key.append(&mut bcs::to_bytes(self).unwrap());
        key
    }

    pub fn to_canonical_string(&self, with_prefix: bool) -> String {
        self.to_canonical_display(with_prefix).to_string()
    }

    /// Proxy type for overriding `ModuleId`'s display implementation, to use a
    /// canonical form (full-width addresses), with an optional "0x" prefix
    /// (controlled by the `with_prefix` flag).
    pub fn to_canonical_display(&self, with_prefix: bool) -> impl Display + '_ {
        struct IdDisplay<'a> {
            id: &'a ModuleId,
            with_prefix: bool,
        }

        impl Display for IdDisplay<'_> {
            fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
                write!(
                    f,
                    "{}::{}",
                    self.id.address.to_canonical_string(self.with_prefix),
                    self.id.name,
                )
            }
        }

        IdDisplay {
            id: self,
            with_prefix,
        }
    }
}

impl Display for ModuleId {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_canonical_display(/* with_prefix */ false))
    }
}

impl FromStr for ModuleId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ParsedModuleId::parse(s)?.into_module_id(&|_| None)
    }
}

impl ModuleId {
    pub fn short_str_lossless(&self) -> String {
        format!("{}::{}", self.address.to_short_string(true), self.name)
    }
}

/// Return the abstract size we use for gas metering
/// This size might be imperfect but should be consistent across platforms
/// TODO (ade): use macro to enforce determinism
pub fn type_tag_abstract_size_for_gas_metering(type_tag: &TypeTag) -> AbstractMemorySize {
    *TYPETAG_ENUM_ABSTRACT_SIZE
        + match type_tag {
            TypeTag::Bool
            | TypeTag::U8
            | TypeTag::U64
            | TypeTag::U128
            | TypeTag::Address
            | TypeTag::Signer
            | TypeTag::U16
            | TypeTag::U32
            | TypeTag::U256 => AbstractMemorySize::new(0),
            TypeTag::Vector(x) => type_tag_abstract_size_for_gas_metering(x),
            TypeTag::Struct(y) => struct_tag_abstract_size_for_gas_metering(y),
        }
}

/// Return the abstract size we use for gas metering
/// This size might be imperfect but should be consistent across platforms
/// TODO (ade): use macro to enforce determinism
pub fn struct_tag_abstract_size_for_gas_metering(struct_tag: &StructTag) -> AbstractMemorySize {
    // TODO: make this more robust as struct size changes
    address_abstract_size_for_gas_metering()
        + identifier_abstract_size_for_gas_metering(struct_tag.module())
        + identifier_abstract_size_for_gas_metering(struct_tag.name())
        + struct_tag
            .type_params()
            .iter()
            .fold(AbstractMemorySize::new(0), |accum, val| {
                accum + type_tag_abstract_size_for_gas_metering(val)
            })
}

pub fn access_vector(struct_tag: &StructTag) -> Vec<u8> {
    let mut key = vec![RESOURCE_TAG];
    key.append(&mut bcs::to_bytes(struct_tag).unwrap());
    key
}

#[cfg(test)]
mod tests {

    use super::ModuleId;
    use crate::{account_address::AccountAddress, ident_str};

    #[test]
    fn test_module_id_display() {
        let id = ModuleId::new(AccountAddress::STD, ident_str!("foo").to_owned());

        assert_eq!(
            format!("{id}"),
            "0000000000000000000000000000000000000000000000000000000000000001::foo",
        );

        assert_eq!(
            format!("{}", id.to_canonical_display(/* with_prefix */ false)),
            "0000000000000000000000000000000000000000000000000000000000000001::foo",
        );

        assert_eq!(
            format!("{}", id.to_canonical_display(/* with_prefix */ true)),
            "0x0000000000000000000000000000000000000000000000000000000000000001::foo",
        );
    }
}
