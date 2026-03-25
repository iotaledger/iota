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
        /// Sets `limit` with the provided value.
        pub fn with_limit(mut self, field: u32) -> Self {
            self.limit = Some(field);
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
        /// Sets `has_next` with the provided value.
        pub fn with_has_next(mut self, field: bool) -> Self {
            self.has_next = field;
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
        /// Sets `version` with the provided value.
        pub fn with_version(mut self, field: u64) -> Self {
            self.version = Some(field);
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
    }
}
