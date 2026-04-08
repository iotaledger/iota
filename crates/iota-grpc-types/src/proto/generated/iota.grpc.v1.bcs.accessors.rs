// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::BcsData {
        /// Sets `data` with the provided value.
        pub fn with_data<T: Into<::prost::bytes::Bytes>>(mut self, field: T) -> Self {
            self.data = field.into();
            self
        }
    }
}
