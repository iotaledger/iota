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
    #[allow(unused_imports)]
    use crate::v1::signatures::ValidatorAggregatedSignature;
    #[allow(unused_imports)]
    use crate::v1::signatures::ValidatorAggregatedSignatureFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::Digest;
    #[allow(unused_imports)]
    use crate::v1::types::DigestFieldPathBuilder;
    impl CheckpointSummary {
        pub const DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "digest",
            json_name: "digest",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Digest::FIELDS),
        };
        pub const BCS_FIELD: &'static MessageField = &MessageField {
            name: "bcs",
            json_name: "bcs",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
    }
    impl MessageFields for CheckpointSummary {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::DIGEST_FIELD,
            Self::BCS_FIELD,
        ];
    }
    impl CheckpointSummary {
        pub fn path_builder() -> CheckpointSummaryFieldPathBuilder {
            CheckpointSummaryFieldPathBuilder::new()
        }
    }
    pub struct CheckpointSummaryFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CheckpointSummaryFieldPathBuilder {
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
        pub fn digest(mut self) -> DigestFieldPathBuilder {
            self.path.push(CheckpointSummary::DIGEST_FIELD.name);
            DigestFieldPathBuilder::new_with_base(self.path)
        }
        pub fn bcs(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(CheckpointSummary::BCS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl CheckpointContents {
        pub const DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "digest",
            json_name: "digest",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Digest::FIELDS),
        };
        pub const BCS_FIELD: &'static MessageField = &MessageField {
            name: "bcs",
            json_name: "bcs",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
    }
    impl MessageFields for CheckpointContents {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::DIGEST_FIELD,
            Self::BCS_FIELD,
        ];
    }
    impl CheckpointContents {
        pub fn path_builder() -> CheckpointContentsFieldPathBuilder {
            CheckpointContentsFieldPathBuilder::new()
        }
    }
    pub struct CheckpointContentsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CheckpointContentsFieldPathBuilder {
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
        pub fn digest(mut self) -> DigestFieldPathBuilder {
            self.path.push(CheckpointContents::DIGEST_FIELD.name);
            DigestFieldPathBuilder::new_with_base(self.path)
        }
        pub fn bcs(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(CheckpointContents::BCS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl Checkpoint {
        pub const SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
            name: "sequence_number",
            json_name: "sequenceNumber",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const SUMMARY_FIELD: &'static MessageField = &MessageField {
            name: "summary",
            json_name: "summary",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(CheckpointSummary::FIELDS),
        };
        pub const CONTENTS_FIELD: &'static MessageField = &MessageField {
            name: "contents",
            json_name: "contents",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(CheckpointContents::FIELDS),
        };
        pub const SIGNATURE_FIELD: &'static MessageField = &MessageField {
            name: "signature",
            json_name: "signature",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ValidatorAggregatedSignature::FIELDS),
        };
    }
    impl MessageFields for Checkpoint {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::SEQUENCE_NUMBER_FIELD,
            Self::SUMMARY_FIELD,
            Self::CONTENTS_FIELD,
            Self::SIGNATURE_FIELD,
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
        pub fn summary(mut self) -> CheckpointSummaryFieldPathBuilder {
            self.path.push(Checkpoint::SUMMARY_FIELD.name);
            CheckpointSummaryFieldPathBuilder::new_with_base(self.path)
        }
        pub fn contents(mut self) -> CheckpointContentsFieldPathBuilder {
            self.path.push(Checkpoint::CONTENTS_FIELD.name);
            CheckpointContentsFieldPathBuilder::new_with_base(self.path)
        }
        pub fn signature(mut self) -> ValidatorAggregatedSignatureFieldPathBuilder {
            self.path.push(Checkpoint::SIGNATURE_FIELD.name);
            ValidatorAggregatedSignatureFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
