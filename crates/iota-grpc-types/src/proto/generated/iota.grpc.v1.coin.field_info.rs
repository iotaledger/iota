// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectId;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectIdFieldPathBuilder;
    impl CoinMetadata {
        pub const ID_FIELD: &'static MessageField = &MessageField {
            name: "id",
            json_name: "id",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const DECIMALS_FIELD: &'static MessageField = &MessageField {
            name: "decimals",
            json_name: "decimals",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const NAME_FIELD: &'static MessageField = &MessageField {
            name: "name",
            json_name: "name",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const SYMBOL_FIELD: &'static MessageField = &MessageField {
            name: "symbol",
            json_name: "symbol",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const DESCRIPTION_FIELD: &'static MessageField = &MessageField {
            name: "description",
            json_name: "description",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const ICON_URL_FIELD: &'static MessageField = &MessageField {
            name: "icon_url",
            json_name: "iconUrl",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const METADATA_CAP_ID_FIELD: &'static MessageField = &MessageField {
            name: "metadata_cap_id",
            json_name: "metadataCapId",
            number: 7i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const METADATA_CAP_STATE_FIELD: &'static MessageField = &MessageField {
            name: "metadata_cap_state",
            json_name: "metadataCapState",
            number: 8i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for CoinMetadata {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::ID_FIELD,
            Self::DECIMALS_FIELD,
            Self::NAME_FIELD,
            Self::SYMBOL_FIELD,
            Self::DESCRIPTION_FIELD,
            Self::ICON_URL_FIELD,
            Self::METADATA_CAP_ID_FIELD,
            Self::METADATA_CAP_STATE_FIELD,
        ];
    }
    impl CoinMetadata {
        pub fn path_builder() -> CoinMetadataFieldPathBuilder {
            CoinMetadataFieldPathBuilder::new()
        }
    }
    pub struct CoinMetadataFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CoinMetadataFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(CoinMetadata::ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn decimals(mut self) -> String {
            self.path.push(CoinMetadata::DECIMALS_FIELD.name);
            self.finish()
        }
        pub fn name(mut self) -> String {
            self.path.push(CoinMetadata::NAME_FIELD.name);
            self.finish()
        }
        pub fn symbol(mut self) -> String {
            self.path.push(CoinMetadata::SYMBOL_FIELD.name);
            self.finish()
        }
        pub fn description(mut self) -> String {
            self.path.push(CoinMetadata::DESCRIPTION_FIELD.name);
            self.finish()
        }
        pub fn icon_url(mut self) -> String {
            self.path.push(CoinMetadata::ICON_URL_FIELD.name);
            self.finish()
        }
        pub fn metadata_cap_id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(CoinMetadata::METADATA_CAP_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn metadata_cap_state(mut self) -> String {
            self.path.push(CoinMetadata::METADATA_CAP_STATE_FIELD.name);
            self.finish()
        }
    }
    impl CoinTreasury {
        pub const ID_FIELD: &'static MessageField = &MessageField {
            name: "id",
            json_name: "id",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const TOTAL_SUPPLY_FIELD: &'static MessageField = &MessageField {
            name: "total_supply",
            json_name: "totalSupply",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const SUPPLY_STATE_FIELD: &'static MessageField = &MessageField {
            name: "supply_state",
            json_name: "supplyState",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for CoinTreasury {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::ID_FIELD,
            Self::TOTAL_SUPPLY_FIELD,
            Self::SUPPLY_STATE_FIELD,
        ];
    }
    impl CoinTreasury {
        pub fn path_builder() -> CoinTreasuryFieldPathBuilder {
            CoinTreasuryFieldPathBuilder::new()
        }
    }
    pub struct CoinTreasuryFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CoinTreasuryFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(CoinTreasury::ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn total_supply(mut self) -> String {
            self.path.push(CoinTreasury::TOTAL_SUPPLY_FIELD.name);
            self.finish()
        }
        pub fn supply_state(mut self) -> String {
            self.path.push(CoinTreasury::SUPPLY_STATE_FIELD.name);
            self.finish()
        }
    }
    impl RegulatedCoinMetadata {
        pub const ID_FIELD: &'static MessageField = &MessageField {
            name: "id",
            json_name: "id",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const COIN_METADATA_OBJECT_FIELD: &'static MessageField = &MessageField {
            name: "coin_metadata_object",
            json_name: "coinMetadataObject",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const DENY_CAP_OBJECT_FIELD: &'static MessageField = &MessageField {
            name: "deny_cap_object",
            json_name: "denyCapObject",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const ALLOW_GLOBAL_PAUSE_FIELD: &'static MessageField = &MessageField {
            name: "allow_global_pause",
            json_name: "allowGlobalPause",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const VARIANT_FIELD: &'static MessageField = &MessageField {
            name: "variant",
            json_name: "variant",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const COIN_REGULATED_STATE_FIELD: &'static MessageField = &MessageField {
            name: "coin_regulated_state",
            json_name: "coinRegulatedState",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for RegulatedCoinMetadata {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::ID_FIELD,
            Self::COIN_METADATA_OBJECT_FIELD,
            Self::DENY_CAP_OBJECT_FIELD,
            Self::ALLOW_GLOBAL_PAUSE_FIELD,
            Self::VARIANT_FIELD,
            Self::COIN_REGULATED_STATE_FIELD,
        ];
    }
    impl RegulatedCoinMetadata {
        pub fn path_builder() -> RegulatedCoinMetadataFieldPathBuilder {
            RegulatedCoinMetadataFieldPathBuilder::new()
        }
    }
    pub struct RegulatedCoinMetadataFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl RegulatedCoinMetadataFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(RegulatedCoinMetadata::ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn coin_metadata_object(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(RegulatedCoinMetadata::COIN_METADATA_OBJECT_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn deny_cap_object(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(RegulatedCoinMetadata::DENY_CAP_OBJECT_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn allow_global_pause(mut self) -> String {
            self.path.push(RegulatedCoinMetadata::ALLOW_GLOBAL_PAUSE_FIELD.name);
            self.finish()
        }
        pub fn variant(mut self) -> String {
            self.path.push(RegulatedCoinMetadata::VARIANT_FIELD.name);
            self.finish()
        }
        pub fn coin_regulated_state(mut self) -> String {
            self.path.push(RegulatedCoinMetadata::COIN_REGULATED_STATE_FIELD.name);
            self.finish()
        }
    }
}
pub use _field_impls::*;
