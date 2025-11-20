// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::UserSignature {
        pub const fn const_default() -> Self {
            Self { bcs: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::UserSignature = super::UserSignature::const_default();
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
    }
    impl super::UserSignatures {
        pub const fn const_default() -> Self {
            Self { signatures: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::UserSignatures = super::UserSignatures::const_default();
            &DEFAULT
        }
        ///Returns the value of `signatures`, or the default value if `signatures` is unset.
        pub fn signatures(&self) -> &[super::UserSignature] {
            &self.signatures
        }
        ///Returns a mutable reference to `signatures`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn signatures_mut(&mut self) -> &mut Vec<super::UserSignature> {
            &mut self.signatures
        }
        ///Sets `signatures` with the provided value.
        pub fn set_signatures(&mut self, field: Vec<super::UserSignature>) {
            self.signatures = field;
        }
        ///Sets `signatures` with the provided value.
        pub fn with_signatures(mut self, field: Vec<super::UserSignature>) -> Self {
            self.set_signatures(field);
            self
        }
    }
    impl super::ValidatorAggregatedSignature {
        pub const fn const_default() -> Self {
            Self { bcs: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ValidatorAggregatedSignature = super::ValidatorAggregatedSignature::const_default();
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
    }
}
