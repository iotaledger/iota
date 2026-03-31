// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v1::bcs::BcsData;
    #[allow(unused_imports)]
    use crate::v1::bcs::BcsDataFieldPathBuilder;
    impl ValidatorCommitteeMember {
        pub const PUBLIC_KEY_FIELD: &'static MessageField = &MessageField {
            name: "public_key",
            json_name: "publicKey",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const WEIGHT_FIELD: &'static MessageField = &MessageField {
            name: "weight",
            json_name: "weight",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ValidatorCommitteeMember {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PUBLIC_KEY_FIELD,
            Self::WEIGHT_FIELD,
        ];
    }
    impl ValidatorCommitteeMember {
        pub fn path_builder() -> ValidatorCommitteeMemberFieldPathBuilder {
            ValidatorCommitteeMemberFieldPathBuilder::new()
        }
    }
    pub struct ValidatorCommitteeMemberFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ValidatorCommitteeMemberFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn public_key(mut self) -> String {
            self.path.push(ValidatorCommitteeMember::PUBLIC_KEY_FIELD.name);
            self.finish()
        }
        pub fn weight(mut self) -> String {
            self.path.push(ValidatorCommitteeMember::WEIGHT_FIELD.name);
            self.finish()
        }
    }
    impl ValidatorCommitteeMembers {
        pub const MEMBERS_FIELD: &'static MessageField = &MessageField {
            name: "members",
            json_name: "members",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ValidatorCommitteeMember::FIELDS),
        };
    }
    impl MessageFields for ValidatorCommitteeMembers {
        const FIELDS: &'static [&'static MessageField] = &[Self::MEMBERS_FIELD];
    }
    impl ValidatorCommitteeMembers {
        pub fn path_builder() -> ValidatorCommitteeMembersFieldPathBuilder {
            ValidatorCommitteeMembersFieldPathBuilder::new()
        }
    }
    pub struct ValidatorCommitteeMembersFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ValidatorCommitteeMembersFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn members(mut self) -> ValidatorCommitteeMemberFieldPathBuilder {
            self.path.push(ValidatorCommitteeMembers::MEMBERS_FIELD.name);
            ValidatorCommitteeMemberFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl ValidatorCommittee {
        pub const EPOCH_FIELD: &'static MessageField = &MessageField {
            name: "epoch",
            json_name: "epoch",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const MEMBERS_FIELD: &'static MessageField = &MessageField {
            name: "members",
            json_name: "members",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ValidatorCommitteeMembers::FIELDS),
        };
    }
    impl MessageFields for ValidatorCommittee {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::EPOCH_FIELD,
            Self::MEMBERS_FIELD,
        ];
    }
    impl ValidatorCommittee {
        pub fn path_builder() -> ValidatorCommitteeFieldPathBuilder {
            ValidatorCommitteeFieldPathBuilder::new()
        }
    }
    pub struct ValidatorCommitteeFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ValidatorCommitteeFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn epoch(mut self) -> String {
            self.path.push(ValidatorCommittee::EPOCH_FIELD.name);
            self.finish()
        }
        pub fn members(mut self) -> ValidatorCommitteeMembersFieldPathBuilder {
            self.path.push(ValidatorCommittee::MEMBERS_FIELD.name);
            ValidatorCommitteeMembersFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl ProtocolFeatureFlags {
        pub const FLAGS_FIELD: &'static MessageField = &MessageField {
            name: "flags",
            json_name: "flags",
            number: 1i32,
            is_optional: false,
            is_map: true,
            message_fields: None,
        };
    }
    impl MessageFields for ProtocolFeatureFlags {
        const FIELDS: &'static [&'static MessageField] = &[Self::FLAGS_FIELD];
    }
    impl ProtocolFeatureFlags {
        pub fn path_builder() -> ProtocolFeatureFlagsFieldPathBuilder {
            ProtocolFeatureFlagsFieldPathBuilder::new()
        }
    }
    pub struct ProtocolFeatureFlagsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ProtocolFeatureFlagsFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn flags(mut self) -> String {
            self.path.push(ProtocolFeatureFlags::FLAGS_FIELD.name);
            self.finish()
        }
    }
    impl ProtocolAttributes {
        pub const ATTRIBUTES_FIELD: &'static MessageField = &MessageField {
            name: "attributes",
            json_name: "attributes",
            number: 1i32,
            is_optional: false,
            is_map: true,
            message_fields: None,
        };
    }
    impl MessageFields for ProtocolAttributes {
        const FIELDS: &'static [&'static MessageField] = &[Self::ATTRIBUTES_FIELD];
    }
    impl ProtocolAttributes {
        pub fn path_builder() -> ProtocolAttributesFieldPathBuilder {
            ProtocolAttributesFieldPathBuilder::new()
        }
    }
    pub struct ProtocolAttributesFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ProtocolAttributesFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn attributes(mut self) -> String {
            self.path.push(ProtocolAttributes::ATTRIBUTES_FIELD.name);
            self.finish()
        }
    }
    impl ProtocolConfig {
        pub const PROTOCOL_VERSION_FIELD: &'static MessageField = &MessageField {
            name: "protocol_version",
            json_name: "protocolVersion",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const FEATURE_FLAGS_FIELD: &'static MessageField = &MessageField {
            name: "feature_flags",
            json_name: "featureFlags",
            number: 2i32,
            is_optional: true,
            is_map: true,
            message_fields: None,
        };
        pub const ATTRIBUTES_FIELD: &'static MessageField = &MessageField {
            name: "attributes",
            json_name: "attributes",
            number: 3i32,
            is_optional: true,
            is_map: true,
            message_fields: None,
        };
    }
    impl MessageFields for ProtocolConfig {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PROTOCOL_VERSION_FIELD,
            Self::FEATURE_FLAGS_FIELD,
            Self::ATTRIBUTES_FIELD,
        ];
    }
    impl ProtocolConfig {
        pub fn path_builder() -> ProtocolConfigFieldPathBuilder {
            ProtocolConfigFieldPathBuilder::new()
        }
    }
    pub struct ProtocolConfigFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ProtocolConfigFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn protocol_version(mut self) -> String {
            self.path.push(ProtocolConfig::PROTOCOL_VERSION_FIELD.name);
            self.finish()
        }
        pub fn feature_flags(mut self) -> String {
            self.path.push(ProtocolConfig::FEATURE_FLAGS_FIELD.name);
            self.finish()
        }
        pub fn attributes(mut self) -> String {
            self.path.push(ProtocolConfig::ATTRIBUTES_FIELD.name);
            self.finish()
        }
    }
    impl Epoch {
        pub const EPOCH_FIELD: &'static MessageField = &MessageField {
            name: "epoch",
            json_name: "epoch",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const COMMITTEE_FIELD: &'static MessageField = &MessageField {
            name: "committee",
            json_name: "committee",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ValidatorCommittee::FIELDS),
        };
        pub const BCS_SYSTEM_STATE_FIELD: &'static MessageField = &MessageField {
            name: "bcs_system_state",
            json_name: "bcsSystemState",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
        pub const FIRST_CHECKPOINT_FIELD: &'static MessageField = &MessageField {
            name: "first_checkpoint",
            json_name: "firstCheckpoint",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const LAST_CHECKPOINT_FIELD: &'static MessageField = &MessageField {
            name: "last_checkpoint",
            json_name: "lastCheckpoint",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const START_FIELD: &'static MessageField = &MessageField {
            name: "start",
            json_name: "start",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const END_FIELD: &'static MessageField = &MessageField {
            name: "end",
            json_name: "end",
            number: 7i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const REFERENCE_GAS_PRICE_FIELD: &'static MessageField = &MessageField {
            name: "reference_gas_price",
            json_name: "referenceGasPrice",
            number: 8i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const PROTOCOL_CONFIG_FIELD: &'static MessageField = &MessageField {
            name: "protocol_config",
            json_name: "protocolConfig",
            number: 9i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ProtocolConfig::FIELDS),
        };
    }
    impl MessageFields for Epoch {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::EPOCH_FIELD,
            Self::COMMITTEE_FIELD,
            Self::BCS_SYSTEM_STATE_FIELD,
            Self::FIRST_CHECKPOINT_FIELD,
            Self::LAST_CHECKPOINT_FIELD,
            Self::START_FIELD,
            Self::END_FIELD,
            Self::REFERENCE_GAS_PRICE_FIELD,
            Self::PROTOCOL_CONFIG_FIELD,
        ];
    }
    impl Epoch {
        pub fn path_builder() -> EpochFieldPathBuilder {
            EpochFieldPathBuilder::new()
        }
    }
    pub struct EpochFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl EpochFieldPathBuilder {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self { path: Default::default() }
        }
        #[doc(hidden)]
        pub fn new_with_base(base: Vec<&'static str>) -> Self {
            Self { path: base }
        }
        pub fn finish(self) -> String {
            self.path.join(".")
        }
        pub fn epoch(mut self) -> String {
            self.path.push(Epoch::EPOCH_FIELD.name);
            self.finish()
        }
        pub fn committee(mut self) -> ValidatorCommitteeFieldPathBuilder {
            self.path.push(Epoch::COMMITTEE_FIELD.name);
            ValidatorCommitteeFieldPathBuilder::new_with_base(self.path)
        }
        pub fn bcs_system_state(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(Epoch::BCS_SYSTEM_STATE_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
        pub fn first_checkpoint(mut self) -> String {
            self.path.push(Epoch::FIRST_CHECKPOINT_FIELD.name);
            self.finish()
        }
        pub fn last_checkpoint(mut self) -> String {
            self.path.push(Epoch::LAST_CHECKPOINT_FIELD.name);
            self.finish()
        }
        pub fn start(mut self) -> String {
            self.path.push(Epoch::START_FIELD.name);
            self.finish()
        }
        pub fn end(mut self) -> String {
            self.path.push(Epoch::END_FIELD.name);
            self.finish()
        }
        pub fn reference_gas_price(mut self) -> String {
            self.path.push(Epoch::REFERENCE_GAS_PRICE_FIELD.name);
            self.finish()
        }
        pub fn protocol_config(mut self) -> ProtocolConfigFieldPathBuilder {
            self.path.push(Epoch::PROTOCOL_CONFIG_FIELD.name);
            ProtocolConfigFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
