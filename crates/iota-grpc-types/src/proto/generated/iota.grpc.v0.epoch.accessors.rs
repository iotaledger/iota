// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::Epoch {
        pub const fn const_default() -> Self {
            Self {
                epoch: None,
                committee: None,
                bcs_system_state: None,
                first_checkpoint: None,
                last_checkpoint: None,
                start: None,
                end: None,
                reference_gas_price: None,
                protocol_config: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::Epoch = super::Epoch::const_default();
            &DEFAULT
        }
        ///If `epoch` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn epoch_opt_mut(&mut self) -> Option<&mut u64> {
            self.epoch.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `epoch`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn epoch_mut(&mut self) -> &mut u64 {
            self.epoch.get_or_insert_default()
        }
        ///If `epoch` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn epoch_opt(&self) -> Option<u64> {
            self.epoch.as_ref().map(|field| *field)
        }
        ///Sets `epoch` with the provided value.
        pub fn set_epoch(&mut self, field: u64) {
            self.epoch = Some(field);
        }
        ///Sets `epoch` with the provided value.
        pub fn with_epoch(mut self, field: u64) -> Self {
            self.set_epoch(field);
            self
        }
        ///Returns the value of `committee`, or the default value if `committee` is unset.
        pub fn committee(&self) -> &super::ValidatorCommittee {
            self.committee
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::ValidatorCommittee::default_instance() as _)
        }
        ///If `committee` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn committee_opt_mut(&mut self) -> Option<&mut super::ValidatorCommittee> {
            self.committee.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `committee`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn committee_mut(&mut self) -> &mut super::ValidatorCommittee {
            self.committee.get_or_insert_default()
        }
        ///If `committee` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn committee_opt(&self) -> Option<&super::ValidatorCommittee> {
            self.committee.as_ref().map(|field| field as _)
        }
        ///Sets `committee` with the provided value.
        pub fn set_committee<T: Into<super::ValidatorCommittee>>(&mut self, field: T) {
            self.committee = Some(field.into().into());
        }
        ///Sets `committee` with the provided value.
        pub fn with_committee<T: Into<super::ValidatorCommittee>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_committee(field.into());
            self
        }
        ///Returns the value of `bcs_system_state`, or the default value if `bcs_system_state` is unset.
        pub fn bcs_system_state(&self) -> &super::super::bcs::BcsData {
            self.bcs_system_state
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::bcs::BcsData::default_instance() as _)
        }
        ///If `bcs_system_state` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn bcs_system_state_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost::alloc::boxed::Box<super::super::bcs::BcsData>> {
            self.bcs_system_state.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `bcs_system_state`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn bcs_system_state_mut(
            &mut self,
        ) -> &mut ::prost::alloc::boxed::Box<super::super::bcs::BcsData> {
            self.bcs_system_state.get_or_insert_default()
        }
        ///If `bcs_system_state` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn bcs_system_state_opt(&self) -> Option<&super::super::bcs::BcsData> {
            self.bcs_system_state.as_ref().map(|field| field as _)
        }
        ///Sets `bcs_system_state` with the provided value.
        pub fn set_bcs_system_state<
            T: Into<::prost::alloc::boxed::Box<super::super::bcs::BcsData>>,
        >(&mut self, field: T) {
            self.bcs_system_state = Some(field.into().into());
        }
        ///Sets `bcs_system_state` with the provided value.
        pub fn with_bcs_system_state<
            T: Into<::prost::alloc::boxed::Box<super::super::bcs::BcsData>>,
        >(mut self, field: T) -> Self {
            self.set_bcs_system_state(field.into());
            self
        }
        ///If `first_checkpoint` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn first_checkpoint_opt_mut(&mut self) -> Option<&mut u64> {
            self.first_checkpoint.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `first_checkpoint`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn first_checkpoint_mut(&mut self) -> &mut u64 {
            self.first_checkpoint.get_or_insert_default()
        }
        ///If `first_checkpoint` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn first_checkpoint_opt(&self) -> Option<u64> {
            self.first_checkpoint.as_ref().map(|field| *field)
        }
        ///Sets `first_checkpoint` with the provided value.
        pub fn set_first_checkpoint(&mut self, field: u64) {
            self.first_checkpoint = Some(field);
        }
        ///Sets `first_checkpoint` with the provided value.
        pub fn with_first_checkpoint(mut self, field: u64) -> Self {
            self.set_first_checkpoint(field);
            self
        }
        ///If `last_checkpoint` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn last_checkpoint_opt_mut(&mut self) -> Option<&mut u64> {
            self.last_checkpoint.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `last_checkpoint`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn last_checkpoint_mut(&mut self) -> &mut u64 {
            self.last_checkpoint.get_or_insert_default()
        }
        ///If `last_checkpoint` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn last_checkpoint_opt(&self) -> Option<u64> {
            self.last_checkpoint.as_ref().map(|field| *field)
        }
        ///Sets `last_checkpoint` with the provided value.
        pub fn set_last_checkpoint(&mut self, field: u64) {
            self.last_checkpoint = Some(field);
        }
        ///Sets `last_checkpoint` with the provided value.
        pub fn with_last_checkpoint(mut self, field: u64) -> Self {
            self.set_last_checkpoint(field);
            self
        }
        ///If `start` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn start_opt_mut(&mut self) -> Option<&mut ::prost_types::Timestamp> {
            self.start.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `start`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn start_mut(&mut self) -> &mut ::prost_types::Timestamp {
            self.start.get_or_insert_default()
        }
        ///If `start` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn start_opt(&self) -> Option<&::prost_types::Timestamp> {
            self.start.as_ref().map(|field| field as _)
        }
        ///Sets `start` with the provided value.
        pub fn set_start<T: Into<::prost_types::Timestamp>>(&mut self, field: T) {
            self.start = Some(field.into().into());
        }
        ///Sets `start` with the provided value.
        pub fn with_start<T: Into<::prost_types::Timestamp>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_start(field.into());
            self
        }
        ///If `end` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn end_opt_mut(&mut self) -> Option<&mut ::prost_types::Timestamp> {
            self.end.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `end`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn end_mut(&mut self) -> &mut ::prost_types::Timestamp {
            self.end.get_or_insert_default()
        }
        ///If `end` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn end_opt(&self) -> Option<&::prost_types::Timestamp> {
            self.end.as_ref().map(|field| field as _)
        }
        ///Sets `end` with the provided value.
        pub fn set_end<T: Into<::prost_types::Timestamp>>(&mut self, field: T) {
            self.end = Some(field.into().into());
        }
        ///Sets `end` with the provided value.
        pub fn with_end<T: Into<::prost_types::Timestamp>>(mut self, field: T) -> Self {
            self.set_end(field.into());
            self
        }
        ///If `reference_gas_price` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn reference_gas_price_opt_mut(&mut self) -> Option<&mut u64> {
            self.reference_gas_price.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `reference_gas_price`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn reference_gas_price_mut(&mut self) -> &mut u64 {
            self.reference_gas_price.get_or_insert_default()
        }
        ///If `reference_gas_price` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn reference_gas_price_opt(&self) -> Option<u64> {
            self.reference_gas_price.as_ref().map(|field| *field)
        }
        ///Sets `reference_gas_price` with the provided value.
        pub fn set_reference_gas_price(&mut self, field: u64) {
            self.reference_gas_price = Some(field);
        }
        ///Sets `reference_gas_price` with the provided value.
        pub fn with_reference_gas_price(mut self, field: u64) -> Self {
            self.set_reference_gas_price(field);
            self
        }
        ///Returns the value of `protocol_config`, or the default value if `protocol_config` is unset.
        pub fn protocol_config(&self) -> &super::ProtocolConfig {
            self.protocol_config
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::ProtocolConfig::default_instance() as _)
        }
        ///If `protocol_config` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn protocol_config_opt_mut(&mut self) -> Option<&mut super::ProtocolConfig> {
            self.protocol_config.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `protocol_config`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn protocol_config_mut(&mut self) -> &mut super::ProtocolConfig {
            self.protocol_config.get_or_insert_default()
        }
        ///If `protocol_config` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn protocol_config_opt(&self) -> Option<&super::ProtocolConfig> {
            self.protocol_config.as_ref().map(|field| field as _)
        }
        ///Sets `protocol_config` with the provided value.
        pub fn set_protocol_config<T: Into<super::ProtocolConfig>>(&mut self, field: T) {
            self.protocol_config = Some(field.into().into());
        }
        ///Sets `protocol_config` with the provided value.
        pub fn with_protocol_config<T: Into<super::ProtocolConfig>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_protocol_config(field.into());
            self
        }
    }
    impl super::ProtocolAttributes {
        pub const fn const_default() -> Self {
            Self {
                attributes: std::collections::BTreeMap::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ProtocolAttributes = super::ProtocolAttributes::const_default();
            &DEFAULT
        }
        ///Returns the value of `attributes`, or the default value if `attributes` is unset.
        pub fn attributes(&self) -> &::std::collections::BTreeMap<String, String> {
            &self.attributes
        }
        ///Returns a mutable reference to `attributes`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn attributes_mut(
            &mut self,
        ) -> &mut ::std::collections::BTreeMap<String, String> {
            &mut self.attributes
        }
        ///Sets `attributes` with the provided value.
        pub fn set_attributes(
            &mut self,
            field: ::std::collections::BTreeMap<String, String>,
        ) {
            self.attributes = field;
        }
        ///Sets `attributes` with the provided value.
        pub fn with_attributes(
            mut self,
            field: ::std::collections::BTreeMap<String, String>,
        ) -> Self {
            self.set_attributes(field);
            self
        }
    }
    impl super::ProtocolConfig {
        pub const fn const_default() -> Self {
            Self {
                protocol_version: None,
                feature_flags: None,
                attributes: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ProtocolConfig = super::ProtocolConfig::const_default();
            &DEFAULT
        }
        ///If `protocol_version` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn protocol_version_opt_mut(&mut self) -> Option<&mut u64> {
            self.protocol_version.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `protocol_version`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn protocol_version_mut(&mut self) -> &mut u64 {
            self.protocol_version.get_or_insert_default()
        }
        ///If `protocol_version` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn protocol_version_opt(&self) -> Option<u64> {
            self.protocol_version.as_ref().map(|field| *field)
        }
        ///Sets `protocol_version` with the provided value.
        pub fn set_protocol_version(&mut self, field: u64) {
            self.protocol_version = Some(field);
        }
        ///Sets `protocol_version` with the provided value.
        pub fn with_protocol_version(mut self, field: u64) -> Self {
            self.set_protocol_version(field);
            self
        }
        ///Returns the value of `feature_flags`, or the default value if `feature_flags` is unset.
        pub fn feature_flags(&self) -> &super::ProtocolFeatureFlags {
            self.feature_flags
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::ProtocolFeatureFlags::default_instance() as _)
        }
        ///If `feature_flags` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn feature_flags_opt_mut(
            &mut self,
        ) -> Option<&mut super::ProtocolFeatureFlags> {
            self.feature_flags.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `feature_flags`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn feature_flags_mut(&mut self) -> &mut super::ProtocolFeatureFlags {
            self.feature_flags.get_or_insert_default()
        }
        ///If `feature_flags` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn feature_flags_opt(&self) -> Option<&super::ProtocolFeatureFlags> {
            self.feature_flags.as_ref().map(|field| field as _)
        }
        ///Sets `feature_flags` with the provided value.
        pub fn set_feature_flags<T: Into<super::ProtocolFeatureFlags>>(
            &mut self,
            field: T,
        ) {
            self.feature_flags = Some(field.into().into());
        }
        ///Sets `feature_flags` with the provided value.
        pub fn with_feature_flags<T: Into<super::ProtocolFeatureFlags>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_feature_flags(field.into());
            self
        }
        ///Returns the value of `attributes`, or the default value if `attributes` is unset.
        pub fn attributes(&self) -> &super::ProtocolAttributes {
            self.attributes
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::ProtocolAttributes::default_instance() as _)
        }
        ///If `attributes` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn attributes_opt_mut(&mut self) -> Option<&mut super::ProtocolAttributes> {
            self.attributes.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `attributes`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn attributes_mut(&mut self) -> &mut super::ProtocolAttributes {
            self.attributes.get_or_insert_default()
        }
        ///If `attributes` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn attributes_opt(&self) -> Option<&super::ProtocolAttributes> {
            self.attributes.as_ref().map(|field| field as _)
        }
        ///Sets `attributes` with the provided value.
        pub fn set_attributes<T: Into<super::ProtocolAttributes>>(&mut self, field: T) {
            self.attributes = Some(field.into().into());
        }
        ///Sets `attributes` with the provided value.
        pub fn with_attributes<T: Into<super::ProtocolAttributes>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_attributes(field.into());
            self
        }
    }
    impl super::ProtocolFeatureFlags {
        pub const fn const_default() -> Self {
            Self {
                flags: std::collections::BTreeMap::new(),
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ProtocolFeatureFlags = super::ProtocolFeatureFlags::const_default();
            &DEFAULT
        }
        ///Returns the value of `flags`, or the default value if `flags` is unset.
        pub fn flags(&self) -> &::std::collections::BTreeMap<String, bool> {
            &self.flags
        }
        ///Returns a mutable reference to `flags`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn flags_mut(&mut self) -> &mut ::std::collections::BTreeMap<String, bool> {
            &mut self.flags
        }
        ///Sets `flags` with the provided value.
        pub fn set_flags(&mut self, field: ::std::collections::BTreeMap<String, bool>) {
            self.flags = field;
        }
        ///Sets `flags` with the provided value.
        pub fn with_flags(
            mut self,
            field: ::std::collections::BTreeMap<String, bool>,
        ) -> Self {
            self.set_flags(field);
            self
        }
    }
    impl super::ValidatorCommittee {
        pub const fn const_default() -> Self {
            Self { epoch: None, members: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ValidatorCommittee = super::ValidatorCommittee::const_default();
            &DEFAULT
        }
        ///If `epoch` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn epoch_opt_mut(&mut self) -> Option<&mut u64> {
            self.epoch.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `epoch`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn epoch_mut(&mut self) -> &mut u64 {
            self.epoch.get_or_insert_default()
        }
        ///If `epoch` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn epoch_opt(&self) -> Option<u64> {
            self.epoch.as_ref().map(|field| *field)
        }
        ///Sets `epoch` with the provided value.
        pub fn set_epoch(&mut self, field: u64) {
            self.epoch = Some(field);
        }
        ///Sets `epoch` with the provided value.
        pub fn with_epoch(mut self, field: u64) -> Self {
            self.set_epoch(field);
            self
        }
        ///Returns the value of `members`, or the default value if `members` is unset.
        pub fn members(&self) -> &super::ValidatorCommitteeMembers {
            self.members
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::ValidatorCommitteeMembers::default_instance() as _
                })
        }
        ///If `members` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn members_opt_mut(
            &mut self,
        ) -> Option<&mut super::ValidatorCommitteeMembers> {
            self.members.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `members`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn members_mut(&mut self) -> &mut super::ValidatorCommitteeMembers {
            self.members.get_or_insert_default()
        }
        ///If `members` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn members_opt(&self) -> Option<&super::ValidatorCommitteeMembers> {
            self.members.as_ref().map(|field| field as _)
        }
        ///Sets `members` with the provided value.
        pub fn set_members<T: Into<super::ValidatorCommitteeMembers>>(
            &mut self,
            field: T,
        ) {
            self.members = Some(field.into().into());
        }
        ///Sets `members` with the provided value.
        pub fn with_members<T: Into<super::ValidatorCommitteeMembers>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_members(field.into());
            self
        }
    }
    impl super::ValidatorCommitteeMember {
        pub const fn const_default() -> Self {
            Self {
                public_key: None,
                weight: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ValidatorCommitteeMember = super::ValidatorCommitteeMember::const_default();
            &DEFAULT
        }
        ///If `public_key` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn public_key_opt(&self) -> Option<&[u8]> {
            self.public_key.as_ref().map(|field| field as _)
        }
        ///Sets `public_key` with the provided value.
        pub fn set_public_key<T: Into<::prost::bytes::Bytes>>(&mut self, field: T) {
            self.public_key = Some(field.into().into());
        }
        ///Sets `public_key` with the provided value.
        pub fn with_public_key<T: Into<::prost::bytes::Bytes>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_public_key(field.into());
            self
        }
        ///If `weight` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn weight_opt_mut(&mut self) -> Option<&mut u64> {
            self.weight.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `weight`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn weight_mut(&mut self) -> &mut u64 {
            self.weight.get_or_insert_default()
        }
        ///If `weight` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn weight_opt(&self) -> Option<u64> {
            self.weight.as_ref().map(|field| *field)
        }
        ///Sets `weight` with the provided value.
        pub fn set_weight(&mut self, field: u64) {
            self.weight = Some(field);
        }
        ///Sets `weight` with the provided value.
        pub fn with_weight(mut self, field: u64) -> Self {
            self.set_weight(field);
            self
        }
    }
    impl super::ValidatorCommitteeMembers {
        pub const fn const_default() -> Self {
            Self { members: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ValidatorCommitteeMembers = super::ValidatorCommitteeMembers::const_default();
            &DEFAULT
        }
        ///Returns the value of `members`, or the default value if `members` is unset.
        pub fn members(&self) -> &[super::ValidatorCommitteeMember] {
            &self.members
        }
        ///Returns a mutable reference to `members`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn members_mut(&mut self) -> &mut Vec<super::ValidatorCommitteeMember> {
            &mut self.members
        }
        ///Sets `members` with the provided value.
        pub fn set_members(&mut self, field: Vec<super::ValidatorCommitteeMember>) {
            self.members = field;
        }
        ///Sets `members` with the provided value.
        pub fn with_members(
            mut self,
            field: Vec<super::ValidatorCommitteeMember>,
        ) -> Self {
            self.set_members(field);
            self
        }
    }
}
