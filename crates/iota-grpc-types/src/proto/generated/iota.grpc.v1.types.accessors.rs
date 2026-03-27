// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Address {
        /// Sets `address` with the provided value.
        pub fn with_address<T: Into<::prost::bytes::Bytes>>(mut self, field: T) -> Self {
            self.address = field.into();
            self
        }
    }
    impl super::Digest {
        /// Sets `digest` with the provided value.
        pub fn with_digest<T: Into<::prost::bytes::Bytes>>(mut self, field: T) -> Self {
            self.digest = field.into();
            self
        }
    }
    impl super::ObjectId {
        /// Sets `object_id` with the provided value.
        pub fn with_object_id<T: Into<::prost::bytes::Bytes>>(
            mut self,
            field: T,
        ) -> Self {
            self.object_id = field.into();
            self
        }
    }
    impl super::ObjectReference {
        /// Sets `object_id` with the provided value.
        pub fn with_object_id<T: Into<super::ObjectId>>(mut self, field: T) -> Self {
            self.object_id = Some(field.into());
            self
        }
        /// Sets `version` with the provided value.
        pub fn with_version(mut self, field: u64) -> Self {
            self.version = Some(field);
            self
        }
        /// Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::Digest>>(mut self, field: T) -> Self {
            self.digest = Some(field.into());
            self
        }
    }
}
