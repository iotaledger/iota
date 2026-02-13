// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Checkpoint {
        /// Sets `sequence_number` with the provided value.
        pub fn with_sequence_number(mut self, field: u64) -> Self {
            self.sequence_number = Some(field);
            self
        }
        /// Sets `summary` with the provided value.
        pub fn with_summary<T: Into<super::CheckpointSummary>>(
            mut self,
            field: T,
        ) -> Self {
            self.summary = Some(field.into());
            self
        }
        /// Sets `contents` with the provided value.
        pub fn with_contents<T: Into<super::CheckpointContents>>(
            mut self,
            field: T,
        ) -> Self {
            self.contents = Some(field.into());
            self
        }
        /// Sets `signature` with the provided value.
        pub fn with_signature<
            T: Into<super::super::signatures::ValidatorAggregatedSignature>,
        >(mut self, field: T) -> Self {
            self.signature = Some(field.into());
            self
        }
    }
    impl super::CheckpointContents {
        /// Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.digest = Some(field.into());
            self
        }
        /// Sets `bcs` with the provided value.
        pub fn with_bcs<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.bcs = Some(field.into());
            self
        }
    }
    impl super::CheckpointSummary {
        /// Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.digest = Some(field.into());
            self
        }
        /// Sets `bcs` with the provided value.
        pub fn with_bcs<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.bcs = Some(field.into());
            self
        }
    }
}
