// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::GetCoinInfoRequest {
        /// Sets `coin_type` with the provided value.
        pub fn with_coin_type<T: Into<String>>(mut self, field: T) -> Self {
            self.coin_type = Some(field.into());
            self
        }
    }
    impl super::GetCoinInfoResponse {
        /// Sets `coin_type` with the provided value.
        pub fn with_coin_type<T: Into<String>>(mut self, field: T) -> Self {
            self.coin_type = Some(field.into());
            self
        }
        /// Sets `metadata` with the provided value.
        pub fn with_metadata<T: Into<super::super::coin::CoinMetadata>>(
            mut self,
            field: T,
        ) -> Self {
            self.metadata = Some(field.into());
            self
        }
        /// Sets `treasury` with the provided value.
        pub fn with_treasury<T: Into<super::super::coin::CoinTreasury>>(
            mut self,
            field: T,
        ) -> Self {
            self.treasury = Some(field.into());
            self
        }
        /// Sets `regulated_metadata` with the provided value.
        pub fn with_regulated_metadata<
            T: Into<super::super::coin::RegulatedCoinMetadata>,
        >(mut self, field: T) -> Self {
            self.regulated_metadata = Some(field.into());
            self
        }
    }
    impl super::ListDynamicFieldsRequest {
        /// Sets `parent` with the provided value.
        pub fn with_parent<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.parent = Some(field.into());
            self
        }
        /// Sets `page_size` with the provided value.
        pub fn with_page_size(mut self, field: u32) -> Self {
            self.page_size = Some(field);
            self
        }
        /// Sets `page_token` with the provided value.
        pub fn with_page_token<T: Into<::prost::bytes::Bytes>>(
            mut self,
            field: T,
        ) -> Self {
            self.page_token = Some(field.into());
            self
        }
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
        /// Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.max_message_size_bytes = Some(field);
            self
        }
    }
    impl super::ListDynamicFieldsResponse {
        /// Sets `dynamic_fields` with the provided value.
        pub fn with_dynamic_fields(
            mut self,
            field: Vec<super::super::dynamic_field::DynamicField>,
        ) -> Self {
            self.dynamic_fields = field;
            self
        }
        /// Sets `next_page_token` with the provided value.
        pub fn with_next_page_token<T: Into<::prost::bytes::Bytes>>(
            mut self,
            field: T,
        ) -> Self {
            self.next_page_token = Some(field.into());
            self
        }
    }
    impl super::ListOwnedObjectsRequest {
        /// Sets `owner` with the provided value.
        pub fn with_owner<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.owner = Some(field.into());
            self
        }
        /// Sets `page_size` with the provided value.
        pub fn with_page_size(mut self, field: u32) -> Self {
            self.page_size = Some(field);
            self
        }
        /// Sets `page_token` with the provided value.
        pub fn with_page_token<T: Into<::prost::bytes::Bytes>>(
            mut self,
            field: T,
        ) -> Self {
            self.page_token = Some(field.into());
            self
        }
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
        /// Sets `object_type` with the provided value.
        pub fn with_object_type<T: Into<String>>(mut self, field: T) -> Self {
            self.object_type = Some(field.into());
            self
        }
        /// Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.max_message_size_bytes = Some(field);
            self
        }
    }
    impl super::ListOwnedObjectsResponse {
        /// Sets `objects` with the provided value.
        pub fn with_objects(mut self, field: Vec<super::super::object::Object>) -> Self {
            self.objects = field;
            self
        }
        /// Sets `next_page_token` with the provided value.
        pub fn with_next_page_token<T: Into<::prost::bytes::Bytes>>(
            mut self,
            field: T,
        ) -> Self {
            self.next_page_token = Some(field.into());
            self
        }
    }
}
