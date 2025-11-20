// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Object {
        pub const fn const_default() -> Self {
            Self { reference: None, bcs: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Object = super::Object::const_default();
            &DEFAULT
        }
        ///Returns the value of `reference`, or the default value if `reference` is unset.
        pub fn reference(&self) -> &super::super::types::ObjectReference {
            self.reference
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::types::ObjectReference::default_instance() as _
                })
        }
        ///If `reference` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn reference_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::types::ObjectReference> {
            self.reference.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `reference`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn reference_mut(&mut self) -> &mut super::super::types::ObjectReference {
            self.reference.get_or_insert_default()
        }
        ///If `reference` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn reference_opt(&self) -> Option<&super::super::types::ObjectReference> {
            self.reference.as_ref().map(|field| field as _)
        }
        ///Sets `reference` with the provided value.
        pub fn set_reference<T: Into<super::super::types::ObjectReference>>(
            &mut self,
            field: T,
        ) {
            self.reference = Some(field.into().into());
        }
        ///Sets `reference` with the provided value.
        pub fn with_reference<T: Into<super::super::types::ObjectReference>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_reference(field.into());
            self
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
    }
    impl super::Objects {
        pub const fn const_default() -> Self {
            Self { objects: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Objects = super::Objects::const_default();
            &DEFAULT
        }
        ///Returns the value of `objects`, or the default value if `objects` is unset.
        pub fn objects(&self) -> &[super::Object] {
            &self.objects
        }
        ///Returns a mutable reference to `objects`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn objects_mut(&mut self) -> &mut Vec<super::Object> {
            &mut self.objects
        }
        ///Sets `objects` with the provided value.
        pub fn set_objects(&mut self, field: Vec<super::Object>) {
            self.objects = field;
        }
        ///Sets `objects` with the provided value.
        pub fn with_objects(mut self, field: Vec<super::Object>) -> Self {
            self.set_objects(field);
            self
        }
    }
}
