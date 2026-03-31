// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    impl BcsData {
        pub const DATA_FIELD: &'static MessageField = &MessageField {
            name: "data",
            json_name: "data",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for BcsData {
        const FIELDS: &'static [&'static MessageField] = &[Self::DATA_FIELD];
    }
    impl BcsData {
        pub fn path_builder() -> BcsDataFieldPathBuilder {
            BcsDataFieldPathBuilder::new()
        }
    }
    pub struct BcsDataFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl BcsDataFieldPathBuilder {
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
        pub fn data(mut self) -> String {
            self.path.push(BcsData::DATA_FIELD.name);
            self.finish()
        }
    }
}
pub use _field_impls::*;
