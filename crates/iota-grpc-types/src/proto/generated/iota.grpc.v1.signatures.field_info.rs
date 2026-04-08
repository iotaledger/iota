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
    impl UserSignature {
        pub const BCS_FIELD: &'static MessageField = &MessageField {
            name: "bcs",
            json_name: "bcs",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
    }
    impl MessageFields for UserSignature {
        const FIELDS: &'static [&'static MessageField] = &[Self::BCS_FIELD];
    }
    impl UserSignature {
        pub fn path_builder() -> UserSignatureFieldPathBuilder {
            UserSignatureFieldPathBuilder::new()
        }
    }
    pub struct UserSignatureFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl UserSignatureFieldPathBuilder {
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
        pub fn bcs(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(UserSignature::BCS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl UserSignatures {
        pub const SIGNATURES_FIELD: &'static MessageField = &MessageField {
            name: "signatures",
            json_name: "signatures",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(UserSignature::FIELDS),
        };
    }
    impl MessageFields for UserSignatures {
        const FIELDS: &'static [&'static MessageField] = &[Self::SIGNATURES_FIELD];
    }
    impl UserSignatures {
        pub fn path_builder() -> UserSignaturesFieldPathBuilder {
            UserSignaturesFieldPathBuilder::new()
        }
    }
    pub struct UserSignaturesFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl UserSignaturesFieldPathBuilder {
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
        pub fn signatures(mut self) -> UserSignatureFieldPathBuilder {
            self.path.push(UserSignatures::SIGNATURES_FIELD.name);
            UserSignatureFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl ValidatorAggregatedSignature {
        pub const BCS_FIELD: &'static MessageField = &MessageField {
            name: "bcs",
            json_name: "bcs",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
    }
    impl MessageFields for ValidatorAggregatedSignature {
        const FIELDS: &'static [&'static MessageField] = &[Self::BCS_FIELD];
    }
    impl ValidatorAggregatedSignature {
        pub fn path_builder() -> ValidatorAggregatedSignatureFieldPathBuilder {
            ValidatorAggregatedSignatureFieldPathBuilder::new()
        }
    }
    pub struct ValidatorAggregatedSignatureFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ValidatorAggregatedSignatureFieldPathBuilder {
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
        pub fn bcs(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(ValidatorAggregatedSignature::BCS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
