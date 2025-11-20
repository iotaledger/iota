// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Checkpoint {
        pub const fn const_default() -> Self {
            Self {
                sequence_number: None,
                summary: None,
                contents: None,
                signature: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Checkpoint = super::Checkpoint::const_default();
            &DEFAULT
        }
        ///If `sequence_number` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn sequence_number_opt_mut(&mut self) -> Option<&mut u64> {
            self.sequence_number.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `sequence_number`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn sequence_number_mut(&mut self) -> &mut u64 {
            self.sequence_number.get_or_insert_default()
        }
        ///If `sequence_number` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn sequence_number_opt(&self) -> Option<u64> {
            self.sequence_number.as_ref().map(|field| *field)
        }
        ///Sets `sequence_number` with the provided value.
        pub fn set_sequence_number(&mut self, field: u64) {
            self.sequence_number = Some(field);
        }
        ///Sets `sequence_number` with the provided value.
        pub fn with_sequence_number(mut self, field: u64) -> Self {
            self.set_sequence_number(field);
            self
        }
        ///Returns the value of `summary`, or the default value if `summary` is unset.
        pub fn summary(&self) -> &super::CheckpointSummary {
            self.summary
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::CheckpointSummary::default_instance() as _)
        }
        ///If `summary` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn summary_opt_mut(&mut self) -> Option<&mut super::CheckpointSummary> {
            self.summary.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `summary`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn summary_mut(&mut self) -> &mut super::CheckpointSummary {
            self.summary.get_or_insert_default()
        }
        ///If `summary` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn summary_opt(&self) -> Option<&super::CheckpointSummary> {
            self.summary.as_ref().map(|field| field as _)
        }
        ///Sets `summary` with the provided value.
        pub fn set_summary<T: Into<super::CheckpointSummary>>(&mut self, field: T) {
            self.summary = Some(field.into().into());
        }
        ///Sets `summary` with the provided value.
        pub fn with_summary<T: Into<super::CheckpointSummary>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_summary(field.into());
            self
        }
        ///Returns the value of `contents`, or the default value if `contents` is unset.
        pub fn contents(&self) -> &super::CheckpointContents {
            self.contents
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::CheckpointContents::default_instance() as _)
        }
        ///If `contents` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn contents_opt_mut(&mut self) -> Option<&mut super::CheckpointContents> {
            self.contents.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `contents`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn contents_mut(&mut self) -> &mut super::CheckpointContents {
            self.contents.get_or_insert_default()
        }
        ///If `contents` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn contents_opt(&self) -> Option<&super::CheckpointContents> {
            self.contents.as_ref().map(|field| field as _)
        }
        ///Sets `contents` with the provided value.
        pub fn set_contents<T: Into<super::CheckpointContents>>(&mut self, field: T) {
            self.contents = Some(field.into().into());
        }
        ///Sets `contents` with the provided value.
        pub fn with_contents<T: Into<super::CheckpointContents>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_contents(field.into());
            self
        }
        ///Returns the value of `signature`, or the default value if `signature` is unset.
        pub fn signature(
            &self,
        ) -> &super::super::signatures::ValidatorAggregatedSignature {
            self.signature
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::signatures::ValidatorAggregatedSignature::default_instance()
                        as _
                })
        }
        ///If `signature` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn signature_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::signatures::ValidatorAggregatedSignature> {
            self.signature.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `signature`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn signature_mut(
            &mut self,
        ) -> &mut super::super::signatures::ValidatorAggregatedSignature {
            self.signature.get_or_insert_default()
        }
        ///If `signature` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn signature_opt(
            &self,
        ) -> Option<&super::super::signatures::ValidatorAggregatedSignature> {
            self.signature.as_ref().map(|field| field as _)
        }
        ///Sets `signature` with the provided value.
        pub fn set_signature<
            T: Into<super::super::signatures::ValidatorAggregatedSignature>,
        >(&mut self, field: T) {
            self.signature = Some(field.into().into());
        }
        ///Sets `signature` with the provided value.
        pub fn with_signature<
            T: Into<super::super::signatures::ValidatorAggregatedSignature>,
        >(mut self, field: T) -> Self {
            self.set_signature(field.into());
            self
        }
    }
    impl super::CheckpointContents {
        pub const fn const_default() -> Self {
            Self { digest: None, bcs: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::CheckpointContents = super::CheckpointContents::const_default();
            &DEFAULT
        }
        ///Returns the value of `digest`, or the default value if `digest` is unset.
        pub fn digest(&self) -> &super::super::types::Digest {
            self.digest
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Digest::default_instance() as _)
        }
        ///If `digest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn digest_opt_mut(&mut self) -> Option<&mut super::super::types::Digest> {
            self.digest.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `digest`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn digest_mut(&mut self) -> &mut super::super::types::Digest {
            self.digest.get_or_insert_default()
        }
        ///If `digest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn digest_opt(&self) -> Option<&super::super::types::Digest> {
            self.digest.as_ref().map(|field| field as _)
        }
        ///Sets `digest` with the provided value.
        pub fn set_digest<T: Into<super::super::types::Digest>>(&mut self, field: T) {
            self.digest = Some(field.into().into());
        }
        ///Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_digest(field.into());
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
    impl super::CheckpointSummary {
        pub const fn const_default() -> Self {
            Self { digest: None, bcs: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::CheckpointSummary = super::CheckpointSummary::const_default();
            &DEFAULT
        }
        ///Returns the value of `digest`, or the default value if `digest` is unset.
        pub fn digest(&self) -> &super::super::types::Digest {
            self.digest
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Digest::default_instance() as _)
        }
        ///If `digest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn digest_opt_mut(&mut self) -> Option<&mut super::super::types::Digest> {
            self.digest.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `digest`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn digest_mut(&mut self) -> &mut super::super::types::Digest {
            self.digest.get_or_insert_default()
        }
        ///If `digest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn digest_opt(&self) -> Option<&super::super::types::Digest> {
            self.digest.as_ref().map(|field| field as _)
        }
        ///Sets `digest` with the provided value.
        pub fn set_digest<T: Into<super::super::types::Digest>>(&mut self, field: T) {
            self.digest = Some(field.into().into());
        }
        ///Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_digest(field.into());
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
}
