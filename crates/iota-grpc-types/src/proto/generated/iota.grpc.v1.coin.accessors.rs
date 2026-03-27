// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::CoinMetadata {
        /// Sets `id` with the provided value.
        pub fn with_id<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.id = Some(field.into());
            self
        }
        /// Sets `decimals` with the provided value.
        pub fn with_decimals(mut self, field: u32) -> Self {
            self.decimals = Some(field);
            self
        }
        /// Sets `name` with the provided value.
        pub fn with_name<T: Into<String>>(mut self, field: T) -> Self {
            self.name = Some(field.into());
            self
        }
        /// Sets `symbol` with the provided value.
        pub fn with_symbol<T: Into<String>>(mut self, field: T) -> Self {
            self.symbol = Some(field.into());
            self
        }
        /// Sets `description` with the provided value.
        pub fn with_description<T: Into<String>>(mut self, field: T) -> Self {
            self.description = Some(field.into());
            self
        }
        /// Sets `icon_url` with the provided value.
        pub fn with_icon_url<T: Into<String>>(mut self, field: T) -> Self {
            self.icon_url = Some(field.into());
            self
        }
        /// Sets `metadata_cap_id` with the provided value.
        pub fn with_metadata_cap_id<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.metadata_cap_id = Some(field.into());
            self
        }
        /// Sets `metadata_cap_state` with the provided value.
        pub fn with_metadata_cap_state(
            mut self,
            field: super::coin_metadata::MetadataCapState,
        ) -> Self {
            self.metadata_cap_state = Some(field.into());
            self
        }
    }
    impl super::CoinTreasury {
        /// Sets `id` with the provided value.
        pub fn with_id<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.id = Some(field.into());
            self
        }
        /// Sets `total_supply` with the provided value.
        pub fn with_total_supply(mut self, field: u64) -> Self {
            self.total_supply = Some(field);
            self
        }
        /// Sets `supply_state` with the provided value.
        pub fn with_supply_state(
            mut self,
            field: super::coin_treasury::SupplyState,
        ) -> Self {
            self.supply_state = Some(field.into());
            self
        }
    }
    impl super::RegulatedCoinMetadata {
        /// Sets `id` with the provided value.
        pub fn with_id<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.id = Some(field.into());
            self
        }
        /// Sets `coin_metadata_object` with the provided value.
        pub fn with_coin_metadata_object<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.coin_metadata_object = Some(field.into());
            self
        }
        /// Sets `deny_cap_object` with the provided value.
        pub fn with_deny_cap_object<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.deny_cap_object = Some(field.into());
            self
        }
        /// Sets `allow_global_pause` with the provided value.
        pub fn with_allow_global_pause(mut self, field: bool) -> Self {
            self.allow_global_pause = Some(field);
            self
        }
        /// Sets `variant` with the provided value.
        pub fn with_variant(mut self, field: u32) -> Self {
            self.variant = Some(field);
            self
        }
        /// Sets `coin_regulated_state` with the provided value.
        pub fn with_coin_regulated_state(
            mut self,
            field: super::regulated_coin_metadata::CoinRegulatedState,
        ) -> Self {
            self.coin_regulated_state = Some(field.into());
            self
        }
    }
}
