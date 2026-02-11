// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Event {
        /// Sets `bcs` with the provided value.
        pub fn set_bcs<T: Into<super::super::bcs::BcsData>>(&mut self, field: T) {
            self.bcs = Some(field.into().into());
        }
        /// Sets `bcs` with the provided value.
        pub fn with_bcs<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_bcs(field.into());
            self
        }
        /// Sets `package_id` with the provided value.
        pub fn set_package_id<T: Into<super::super::types::Address>>(
            &mut self,
            field: T,
        ) {
            self.package_id = Some(field.into().into());
        }
        /// Sets `package_id` with the provided value.
        pub fn with_package_id<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_package_id(field.into());
            self
        }
        /// Sets `module` with the provided value.
        pub fn set_module<T: Into<String>>(&mut self, field: T) {
            self.module = Some(field.into().into());
        }
        /// Sets `module` with the provided value.
        pub fn with_module<T: Into<String>>(mut self, field: T) -> Self {
            self.set_module(field.into());
            self
        }
        /// Sets `sender` with the provided value.
        pub fn set_sender<T: Into<super::super::types::Address>>(&mut self, field: T) {
            self.sender = Some(field.into().into());
        }
        /// Sets `sender` with the provided value.
        pub fn with_sender<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_sender(field.into());
            self
        }
        /// Sets `event_type` with the provided value.
        pub fn set_event_type<T: Into<String>>(&mut self, field: T) {
            self.event_type = Some(field.into().into());
        }
        /// Sets `event_type` with the provided value.
        pub fn with_event_type<T: Into<String>>(mut self, field: T) -> Self {
            self.set_event_type(field.into());
            self
        }
        /// Sets `bcs_contents` with the provided value.
        pub fn set_bcs_contents<T: Into<super::super::bcs::BcsData>>(
            &mut self,
            field: T,
        ) {
            self.bcs_contents = Some(field.into().into());
        }
        /// Sets `bcs_contents` with the provided value.
        pub fn with_bcs_contents<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_bcs_contents(field.into());
            self
        }
        /// Sets `json_contents` with the provided value.
        pub fn set_json_contents<T: Into<::prost_types::Value>>(&mut self, field: T) {
            self.json_contents = Some(field.into().into());
        }
        /// Sets `json_contents` with the provided value.
        pub fn with_json_contents<T: Into<::prost_types::Value>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_json_contents(field.into());
            self
        }
    }
    impl super::Events {
        /// Sets `events` with the provided value.
        pub fn with_events(mut self, field: Vec<super::Event>) -> Self {
            self.events = field;
            self
        }
    }
}
