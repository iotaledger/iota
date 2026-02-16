// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Epoch {
        /// Sets `epoch` with the provided value.
        pub fn with_epoch(mut self, field: u64) -> Self {
            self.epoch = Some(field);
            self
        }
        /// Sets `committee` with the provided value.
        pub fn with_committee<T: Into<super::ValidatorCommittee>>(
            mut self,
            field: T,
        ) -> Self {
            self.committee = Some(field.into());
            self
        }
        /// Sets `bcs_system_state` with the provided value.
        pub fn with_bcs_system_state<T: Into<super::super::bcs::BcsData>>(
            mut self,
            field: T,
        ) -> Self {
            self.bcs_system_state = Some(field.into());
            self
        }
        /// Sets `first_checkpoint` with the provided value.
        pub fn with_first_checkpoint(mut self, field: u64) -> Self {
            self.first_checkpoint = Some(field);
            self
        }
        /// Sets `last_checkpoint` with the provided value.
        pub fn with_last_checkpoint(mut self, field: u64) -> Self {
            self.last_checkpoint = Some(field);
            self
        }
        /// Sets `start` with the provided value.
        pub fn with_start<T: Into<::prost_types::Timestamp>>(
            mut self,
            field: T,
        ) -> Self {
            self.start = Some(field.into());
            self
        }
        /// Sets `end` with the provided value.
        pub fn with_end<T: Into<::prost_types::Timestamp>>(mut self, field: T) -> Self {
            self.end = Some(field.into());
            self
        }
        /// Sets `reference_gas_price` with the provided value.
        pub fn with_reference_gas_price(mut self, field: u64) -> Self {
            self.reference_gas_price = Some(field);
            self
        }
        /// Sets `protocol_config` with the provided value.
        pub fn with_protocol_config<T: Into<super::ProtocolConfig>>(
            mut self,
            field: T,
        ) -> Self {
            self.protocol_config = Some(field.into());
            self
        }
    }
    impl super::ProtocolAttributes {
        /// Sets `attributes` with the provided value.
        pub fn with_attributes(
            mut self,
            field: ::std::collections::BTreeMap<String, String>,
        ) -> Self {
            self.attributes = field;
            self
        }
    }
    impl super::ProtocolConfig {
        /// Sets `protocol_version` with the provided value.
        pub fn with_protocol_version(mut self, field: u64) -> Self {
            self.protocol_version = Some(field);
            self
        }
        /// Sets `feature_flags` with the provided value.
        pub fn with_feature_flags<T: Into<super::ProtocolFeatureFlags>>(
            mut self,
            field: T,
        ) -> Self {
            self.feature_flags = Some(field.into());
            self
        }
        /// Sets `attributes` with the provided value.
        pub fn with_attributes<T: Into<super::ProtocolAttributes>>(
            mut self,
            field: T,
        ) -> Self {
            self.attributes = Some(field.into());
            self
        }
    }
    impl super::ProtocolFeatureFlags {
        /// Sets `flags` with the provided value.
        pub fn with_flags(
            mut self,
            field: ::std::collections::BTreeMap<String, bool>,
        ) -> Self {
            self.flags = field;
            self
        }
    }
    impl super::ValidatorCommittee {
        /// Sets `epoch` with the provided value.
        pub fn with_epoch(mut self, field: u64) -> Self {
            self.epoch = Some(field);
            self
        }
        /// Sets `members` with the provided value.
        pub fn with_members<T: Into<super::ValidatorCommitteeMembers>>(
            mut self,
            field: T,
        ) -> Self {
            self.members = Some(field.into());
            self
        }
    }
    impl super::ValidatorCommitteeMember {
        /// Sets `public_key` with the provided value.
        pub fn with_public_key<T: Into<::prost::bytes::Bytes>>(
            mut self,
            field: T,
        ) -> Self {
            self.public_key = Some(field.into());
            self
        }
        /// Sets `weight` with the provided value.
        pub fn with_weight(mut self, field: u64) -> Self {
            self.weight = Some(field);
            self
        }
    }
    impl super::ValidatorCommitteeMembers {
        /// Sets `members` with the provided value.
        pub fn with_members(
            mut self,
            field: Vec<super::ValidatorCommitteeMember>,
        ) -> Self {
            self.members = field;
            self
        }
    }
}
