// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Event {
        pub const fn const_default() -> Self {
            Self {
                bcs: None,
                package_id: None,
                module: None,
                sender: None,
                event_type: None,
                bcs_contents: None,
                json_contents: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Event = super::Event::const_default();
            &DEFAULT
        }
        ///Returns the value of `bcs`, or the default value if `bcs` is unset.
        pub fn bcs(&self) -> &super::super::bcs::BcsData {
            self.bcs
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::bcs::BcsData::default_instance() as _)
        }
        ///If `bcs` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn bcs_opt_mut(&mut self) -> Option<&mut super::super::bcs::BcsData> {
            self.bcs.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `bcs`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn bcs_mut(&mut self) -> &mut super::super::bcs::BcsData {
            self.bcs.get_or_insert_default()
        }
        ///If `bcs` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn bcs_opt(&self) -> Option<&super::super::bcs::BcsData> {
            self.bcs.as_ref().map(|field| field as _)
        }
        ///Sets `bcs` with the provided value.
        pub fn set_bcs<T: Into<super::super::bcs::BcsData>>(&mut self, field: T) {
            self.bcs = Some(field.into().into());
        }
        ///Sets `bcs` with the provided value.
        pub fn with_bcs<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_bcs(field.into());
            self
        }
        ///Returns the value of `package_id`, or the default value if `package_id` is unset.
        pub fn package_id(&self) -> &super::super::types::Address {
            self.package_id
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Address::default_instance() as _)
        }
        ///If `package_id` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn package_id_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::types::Address> {
            self.package_id.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `package_id`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn package_id_mut(&mut self) -> &mut super::super::types::Address {
            self.package_id.get_or_insert_default()
        }
        ///If `package_id` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn package_id_opt(&self) -> Option<&super::super::types::Address> {
            self.package_id.as_ref().map(|field| field as _)
        }
        ///Sets `package_id` with the provided value.
        pub fn set_package_id<T: Into<super::super::types::Address>>(
            &mut self,
            field: T,
        ) {
            self.package_id = Some(field.into().into());
        }
        ///Sets `package_id` with the provided value.
        pub fn with_package_id<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_package_id(field.into());
            self
        }
        ///If `module` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn module_opt_mut(&mut self) -> Option<&mut String> {
            self.module.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `module`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn module_mut(&mut self) -> &mut String {
            self.module.get_or_insert_default()
        }
        ///If `module` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn module_opt(&self) -> Option<&str> {
            self.module.as_ref().map(|field| field as _)
        }
        ///Sets `module` with the provided value.
        pub fn set_module<T: Into<String>>(&mut self, field: T) {
            self.module = Some(field.into().into());
        }
        ///Sets `module` with the provided value.
        pub fn with_module<T: Into<String>>(mut self, field: T) -> Self {
            self.set_module(field.into());
            self
        }
        ///Returns the value of `sender`, or the default value if `sender` is unset.
        pub fn sender(&self) -> &super::super::types::Address {
            self.sender
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Address::default_instance() as _)
        }
        ///If `sender` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn sender_opt_mut(&mut self) -> Option<&mut super::super::types::Address> {
            self.sender.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `sender`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn sender_mut(&mut self) -> &mut super::super::types::Address {
            self.sender.get_or_insert_default()
        }
        ///If `sender` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn sender_opt(&self) -> Option<&super::super::types::Address> {
            self.sender.as_ref().map(|field| field as _)
        }
        ///Sets `sender` with the provided value.
        pub fn set_sender<T: Into<super::super::types::Address>>(&mut self, field: T) {
            self.sender = Some(field.into().into());
        }
        ///Sets `sender` with the provided value.
        pub fn with_sender<T: Into<super::super::types::Address>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_sender(field.into());
            self
        }
        ///If `event_type` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn event_type_opt_mut(&mut self) -> Option<&mut String> {
            self.event_type.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `event_type`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn event_type_mut(&mut self) -> &mut String {
            self.event_type.get_or_insert_default()
        }
        ///If `event_type` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn event_type_opt(&self) -> Option<&str> {
            self.event_type.as_ref().map(|field| field as _)
        }
        ///Sets `event_type` with the provided value.
        pub fn set_event_type<T: Into<String>>(&mut self, field: T) {
            self.event_type = Some(field.into().into());
        }
        ///Sets `event_type` with the provided value.
        pub fn with_event_type<T: Into<String>>(mut self, field: T) -> Self {
            self.set_event_type(field.into());
            self
        }
        ///Returns the value of `bcs_contents`, or the default value if `bcs_contents` is unset.
        pub fn bcs_contents(&self) -> &super::super::bcs::BcsData {
            self.bcs_contents
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::bcs::BcsData::default_instance() as _)
        }
        ///If `bcs_contents` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn bcs_contents_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::bcs::BcsData> {
            self.bcs_contents.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `bcs_contents`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn bcs_contents_mut(&mut self) -> &mut super::super::bcs::BcsData {
            self.bcs_contents.get_or_insert_default()
        }
        ///If `bcs_contents` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn bcs_contents_opt(&self) -> Option<&super::super::bcs::BcsData> {
            self.bcs_contents.as_ref().map(|field| field as _)
        }
        ///Sets `bcs_contents` with the provided value.
        pub fn set_bcs_contents<T: Into<super::super::bcs::BcsData>>(
            &mut self,
            field: T,
        ) {
            self.bcs_contents = Some(field.into().into());
        }
        ///Sets `bcs_contents` with the provided value.
        pub fn with_bcs_contents<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_bcs_contents(field.into());
            self
        }
        ///If `json_contents` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn json_contents_opt_mut(&mut self) -> Option<&mut ::prost_types::Value> {
            self.json_contents.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `json_contents`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn json_contents_mut(&mut self) -> &mut ::prost_types::Value {
            self.json_contents.get_or_insert_default()
        }
        ///If `json_contents` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn json_contents_opt(&self) -> Option<&::prost_types::Value> {
            self.json_contents.as_ref().map(|field| field as _)
        }
        ///Sets `json_contents` with the provided value.
        pub fn set_json_contents<T: Into<::prost_types::Value>>(&mut self, field: T) {
            self.json_contents = Some(field.into().into());
        }
        ///Sets `json_contents` with the provided value.
        pub fn with_json_contents<T: Into<::prost_types::Value>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_json_contents(field.into());
            self
        }
    }
    impl super::Events {
        pub const fn const_default() -> Self {
            Self { events: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Events = super::Events::const_default();
            &DEFAULT
        }
        ///Returns the value of `events`, or the default value if `events` is unset.
        pub fn events(&self) -> &[super::Event] {
            &self.events
        }
        ///Returns a mutable reference to `events`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn events_mut(&mut self) -> &mut Vec<super::Event> {
            &mut self.events
        }
        ///Sets `events` with the provided value.
        pub fn set_events(&mut self, field: Vec<super::Event>) {
            self.events = field;
        }
        ///Sets `events` with the provided value.
        pub fn with_events(mut self, field: Vec<super::Event>) -> Self {
            self.set_events(field);
            self
        }
    }
}
