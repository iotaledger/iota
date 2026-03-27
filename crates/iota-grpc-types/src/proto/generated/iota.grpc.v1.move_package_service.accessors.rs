// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::ListPackageVersionsRequest {
        /// Sets `package_id` with the provided value.
        pub fn with_package_id<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.package_id = Some(field.into());
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
        /// Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.max_message_size_bytes = Some(field);
            self
        }
    }
    impl super::ListPackageVersionsResponse {
        /// Sets `versions` with the provided value.
        pub fn with_versions(mut self, field: Vec<super::PackageVersion>) -> Self {
            self.versions = field;
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
    impl super::PackageVersion {
        /// Sets `original_id` with the provided value.
        pub fn with_original_id<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.original_id = Some(field.into());
            self
        }
        /// Sets `storage_id` with the provided value.
        pub fn with_storage_id<T: Into<super::super::types::ObjectId>>(
            mut self,
            field: T,
        ) -> Self {
            self.storage_id = Some(field.into());
            self
        }
        /// Sets `version` with the provided value.
        pub fn with_version(mut self, field: u64) -> Self {
            self.version = Some(field);
            self
        }
    }
}
