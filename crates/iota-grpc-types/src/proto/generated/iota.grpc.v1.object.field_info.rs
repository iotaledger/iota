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
    use crate::v1::types::ObjectReference;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectReferenceFieldPathBuilder;
    impl Object {
        pub const REFERENCE_FIELD: &'static MessageField = &MessageField {
            name: "reference",
            json_name: "reference",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectReference::FIELDS),
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
    impl MessageFields for Object {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::REFERENCE_FIELD,
            Self::BCS_FIELD,
        ];
    }
    impl Object {
        pub fn path_builder() -> ObjectFieldPathBuilder {
            ObjectFieldPathBuilder::new()
        }
    }
    pub struct ObjectFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ObjectFieldPathBuilder {
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
        pub fn reference(mut self) -> ObjectReferenceFieldPathBuilder {
            self.path.push(Object::REFERENCE_FIELD.name);
            ObjectReferenceFieldPathBuilder::new_with_base(self.path)
        }
        pub fn bcs(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(Object::BCS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl Objects {
        pub const OBJECTS_FIELD: &'static MessageField = &MessageField {
            name: "objects",
            json_name: "objects",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Object::FIELDS),
        };
    }
    impl MessageFields for Objects {
        const FIELDS: &'static [&'static MessageField] = &[Self::OBJECTS_FIELD];
    }
    impl Objects {
        pub fn path_builder() -> ObjectsFieldPathBuilder {
            ObjectsFieldPathBuilder::new()
        }
    }
    pub struct ObjectsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ObjectsFieldPathBuilder {
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
        pub fn objects(mut self) -> ObjectFieldPathBuilder {
            self.path.push(Objects::OBJECTS_FIELD.name);
            ObjectFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
