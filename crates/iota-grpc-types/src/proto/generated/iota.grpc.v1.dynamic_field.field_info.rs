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
    use crate::v1::object::Object;
    #[allow(unused_imports)]
    use crate::v1::object::ObjectFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectId;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectIdFieldPathBuilder;
    impl DynamicField {
        pub const KIND_FIELD: &'static MessageField = &MessageField {
            name: "kind",
            json_name: "kind",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const PARENT_FIELD: &'static MessageField = &MessageField {
            name: "parent",
            json_name: "parent",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const FIELD_ID_FIELD: &'static MessageField = &MessageField {
            name: "field_id",
            json_name: "fieldId",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const FIELD_OBJECT_FIELD: &'static MessageField = &MessageField {
            name: "field_object",
            json_name: "fieldObject",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Object::FIELDS),
        };
        pub const NAME_FIELD: &'static MessageField = &MessageField {
            name: "name",
            json_name: "name",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
        pub const VALUE_FIELD: &'static MessageField = &MessageField {
            name: "value",
            json_name: "value",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
        pub const VALUE_TYPE_FIELD: &'static MessageField = &MessageField {
            name: "value_type",
            json_name: "valueType",
            number: 7i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const CHILD_ID_FIELD: &'static MessageField = &MessageField {
            name: "child_id",
            json_name: "childId",
            number: 8i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const CHILD_OBJECT_FIELD: &'static MessageField = &MessageField {
            name: "child_object",
            json_name: "childObject",
            number: 9i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Object::FIELDS),
        };
    }
    impl MessageFields for DynamicField {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::KIND_FIELD,
            Self::PARENT_FIELD,
            Self::FIELD_ID_FIELD,
            Self::FIELD_OBJECT_FIELD,
            Self::NAME_FIELD,
            Self::VALUE_FIELD,
            Self::VALUE_TYPE_FIELD,
            Self::CHILD_ID_FIELD,
            Self::CHILD_OBJECT_FIELD,
        ];
    }
    impl DynamicField {
        pub fn path_builder() -> DynamicFieldFieldPathBuilder {
            DynamicFieldFieldPathBuilder::new()
        }
    }
    pub struct DynamicFieldFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl DynamicFieldFieldPathBuilder {
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
        pub fn kind(mut self) -> String {
            self.path.push(DynamicField::KIND_FIELD.name);
            self.finish()
        }
        pub fn parent(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(DynamicField::PARENT_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn field_id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(DynamicField::FIELD_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn field_object(mut self) -> ObjectFieldPathBuilder {
            self.path.push(DynamicField::FIELD_OBJECT_FIELD.name);
            ObjectFieldPathBuilder::new_with_base(self.path)
        }
        pub fn name(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(DynamicField::NAME_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
        pub fn value(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(DynamicField::VALUE_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
        pub fn value_type(mut self) -> String {
            self.path.push(DynamicField::VALUE_TYPE_FIELD.name);
            self.finish()
        }
        pub fn child_id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(DynamicField::CHILD_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn child_object(mut self) -> ObjectFieldPathBuilder {
            self.path.push(DynamicField::CHILD_OBJECT_FIELD.name);
            ObjectFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
