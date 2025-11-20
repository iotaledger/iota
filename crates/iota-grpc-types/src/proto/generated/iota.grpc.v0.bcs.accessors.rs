// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::BcsData {
        pub const fn const_default() -> Self {
            Self {
                data: ::prost::bytes::Bytes::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::BcsData = super::BcsData::const_default();
            &DEFAULT
        }
        ///Sets `data` with the provided value.
        pub fn set_data<T: Into<::prost::bytes::Bytes>>(&mut self, field: T) {
            self.data = field.into().into();
        }
        ///Sets `data` with the provided value.
        pub fn with_data<T: Into<::prost::bytes::Bytes>>(mut self, field: T) -> Self {
            self.set_data(field.into());
            self
        }
    }
}
