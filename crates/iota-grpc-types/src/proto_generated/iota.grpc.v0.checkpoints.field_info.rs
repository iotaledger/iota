// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v0::common::BcsData;
    #[allow(unused_imports)]
    use crate::v0::common::BcsDataFieldPathBuilder;
    impl CheckpointStreamRequest {
        pub const START_SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
            name: "start_sequence_number",
            json_name: "startSequenceNumber",
            number: 1i32,
            message_fields: None,
        };
        pub const END_SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
            name: "end_sequence_number",
            json_name: "endSequenceNumber",
            number: 2i32,
            message_fields: None,
        };
        pub const FULL_FIELD: &'static MessageField = &MessageField {
            name: "full",
            json_name: "full",
            number: 3i32,
            message_fields: None,
        };
    }
    impl MessageFields for CheckpointStreamRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::START_SEQUENCE_NUMBER_FIELD,
            Self::END_SEQUENCE_NUMBER_FIELD,
            Self::FULL_FIELD,
        ];
    }
    impl CheckpointStreamRequest {
        pub fn path_builder() -> CheckpointStreamRequestFieldPathBuilder {
            CheckpointStreamRequestFieldPathBuilder::new()
        }
    }
    pub struct CheckpointStreamRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CheckpointStreamRequestFieldPathBuilder {
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
        pub fn start_sequence_number(mut self) -> String {
            self.path.push(CheckpointStreamRequest::START_SEQUENCE_NUMBER_FIELD.name);
            self.finish()
        }
        pub fn end_sequence_number(mut self) -> String {
            self.path.push(CheckpointStreamRequest::END_SEQUENCE_NUMBER_FIELD.name);
            self.finish()
        }
        pub fn full(mut self) -> String {
            self.path.push(CheckpointStreamRequest::FULL_FIELD.name);
            self.finish()
        }
    }
    impl EpochRequest {
        pub const EPOCH_FIELD: &'static MessageField = &MessageField {
            name: "epoch",
            json_name: "epoch",
            number: 1i32,
            message_fields: None,
        };
    }
    impl MessageFields for EpochRequest {
        const FIELDS: &'static [&'static MessageField] = &[Self::EPOCH_FIELD];
    }
    impl EpochRequest {
        pub fn path_builder() -> EpochRequestFieldPathBuilder {
            EpochRequestFieldPathBuilder::new()
        }
    }
    pub struct EpochRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl EpochRequestFieldPathBuilder {
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
            self.path.push(EpochRequest::EPOCH_FIELD.name);
            self.finish()
        }
    }
    impl CheckpointSequenceNumberResponse {
        pub const SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
            name: "sequence_number",
            json_name: "sequenceNumber",
            number: 1i32,
            message_fields: None,
        };
    }
    impl MessageFields for CheckpointSequenceNumberResponse {
        const FIELDS: &'static [&'static MessageField] = &[Self::SEQUENCE_NUMBER_FIELD];
    }
    impl CheckpointSequenceNumberResponse {
        pub fn path_builder() -> CheckpointSequenceNumberResponseFieldPathBuilder {
            CheckpointSequenceNumberResponseFieldPathBuilder::new()
        }
    }
    pub struct CheckpointSequenceNumberResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CheckpointSequenceNumberResponseFieldPathBuilder {
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
        pub fn sequence_number(mut self) -> String {
            self.path.push(CheckpointSequenceNumberResponse::SEQUENCE_NUMBER_FIELD.name);
            self.finish()
        }
    }
    impl Checkpoint {
        pub const SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
            name: "sequence_number",
            json_name: "sequenceNumber",
            number: 1i32,
            message_fields: None,
        };
        pub const IS_FULL_FIELD: &'static MessageField = &MessageField {
            name: "is_full",
            json_name: "isFull",
            number: 2i32,
            message_fields: None,
        };
        pub const BCS_DATA_FIELD: &'static MessageField = &MessageField {
            name: "bcs_data",
            json_name: "bcsData",
            number: 3i32,
            message_fields: Some(BcsData::FIELDS),
        };
    }
    impl MessageFields for Checkpoint {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::SEQUENCE_NUMBER_FIELD,
            Self::IS_FULL_FIELD,
            Self::BCS_DATA_FIELD,
        ];
    }
    impl Checkpoint {
        pub fn path_builder() -> CheckpointFieldPathBuilder {
            CheckpointFieldPathBuilder::new()
        }
    }
    pub struct CheckpointFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CheckpointFieldPathBuilder {
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
        pub fn sequence_number(mut self) -> String {
            self.path.push(Checkpoint::SEQUENCE_NUMBER_FIELD.name);
            self.finish()
        }
        pub fn is_full(mut self) -> String {
            self.path.push(Checkpoint::IS_FULL_FIELD.name);
            self.finish()
        }
        pub fn bcs_data(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(Checkpoint::BCS_DATA_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
