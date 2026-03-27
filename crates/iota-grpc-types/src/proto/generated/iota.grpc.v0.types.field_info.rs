// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    impl Address {
        pub const ADDRESS_FIELD: &'static MessageField = &MessageField {
            name: "address",
            json_name: "address",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for Address {
        const FIELDS: &'static [&'static MessageField] = &[Self::ADDRESS_FIELD];
    }
    impl Address {
        pub fn path_builder() -> AddressFieldPathBuilder {
            AddressFieldPathBuilder::new()
        }
    }
    pub struct AddressFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl AddressFieldPathBuilder {
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
        pub fn address(mut self) -> String {
            self.path.push(Address::ADDRESS_FIELD.name);
            self.finish()
        }
    }
    impl ObjectId {
        pub const OBJECT_ID_FIELD: &'static MessageField = &MessageField {
            name: "object_id",
            json_name: "objectId",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ObjectId {
        const FIELDS: &'static [&'static MessageField] = &[Self::OBJECT_ID_FIELD];
    }
    impl ObjectId {
        pub fn path_builder() -> ObjectIdFieldPathBuilder {
            ObjectIdFieldPathBuilder::new()
        }
    }
    pub struct ObjectIdFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ObjectIdFieldPathBuilder {
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
        pub fn object_id(mut self) -> String {
            self.path.push(ObjectId::OBJECT_ID_FIELD.name);
            self.finish()
        }
    }
    impl Digest {
        pub const DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "digest",
            json_name: "digest",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for Digest {
        const FIELDS: &'static [&'static MessageField] = &[Self::DIGEST_FIELD];
    }
    impl Digest {
        pub fn path_builder() -> DigestFieldPathBuilder {
            DigestFieldPathBuilder::new()
        }
    }
    pub struct DigestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl DigestFieldPathBuilder {
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
        pub fn digest(mut self) -> String {
            self.path.push(Digest::DIGEST_FIELD.name);
            self.finish()
        }
    }
    impl ObjectReference {
        pub const OBJECT_ID_FIELD: &'static MessageField = &MessageField {
            name: "object_id",
            json_name: "objectId",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const VERSION_FIELD: &'static MessageField = &MessageField {
            name: "version",
            json_name: "version",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "digest",
            json_name: "digest",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Digest::FIELDS),
        };
    }
    impl MessageFields for ObjectReference {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::OBJECT_ID_FIELD,
            Self::VERSION_FIELD,
            Self::DIGEST_FIELD,
        ];
    }
    impl ObjectReference {
        pub fn path_builder() -> ObjectReferenceFieldPathBuilder {
            ObjectReferenceFieldPathBuilder::new()
        }
    }
    pub struct ObjectReferenceFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ObjectReferenceFieldPathBuilder {
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
        pub fn object_id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(ObjectReference::OBJECT_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn version(mut self) -> String {
            self.path.push(ObjectReference::VERSION_FIELD.name);
            self.finish()
        }
        pub fn digest(mut self) -> DigestFieldPathBuilder {
            self.path.push(ObjectReference::DIGEST_FIELD.name);
            DigestFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl TypeTagVector {
        pub const INNER_TYPE_FIELD: &'static MessageField = &MessageField {
            name: "inner_type",
            json_name: "innerType",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for TypeTagVector {
        const FIELDS: &'static [&'static MessageField] = &[Self::INNER_TYPE_FIELD];
    }
    impl TypeTagVector {
        pub fn path_builder() -> TypeTagVectorFieldPathBuilder {
            TypeTagVectorFieldPathBuilder::new()
        }
    }
    pub struct TypeTagVectorFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TypeTagVectorFieldPathBuilder {
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
        pub fn inner_type(mut self) -> TypeTagFieldPathBuilder {
            self.path.push(TypeTagVector::INNER_TYPE_FIELD.name);
            TypeTagFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl TypeTagStruct {
        pub const STRUCT_TAG_FIELD: &'static MessageField = &MessageField {
            name: "struct_tag",
            json_name: "structTag",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for TypeTagStruct {
        const FIELDS: &'static [&'static MessageField] = &[Self::STRUCT_TAG_FIELD];
    }
    impl TypeTagStruct {
        pub fn path_builder() -> TypeTagStructFieldPathBuilder {
            TypeTagStructFieldPathBuilder::new()
        }
    }
    pub struct TypeTagStructFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TypeTagStructFieldPathBuilder {
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
        pub fn struct_tag(mut self) -> String {
            self.path.push(TypeTagStruct::STRUCT_TAG_FIELD.name);
            self.finish()
        }
    }
    impl TypeTag {
        pub const BOOL_TAG_FIELD: &'static MessageField = &MessageField {
            name: "bool_tag",
            json_name: "boolTag",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const U8_TAG_FIELD: &'static MessageField = &MessageField {
            name: "u8_tag",
            json_name: "u8Tag",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const U16_TAG_FIELD: &'static MessageField = &MessageField {
            name: "u16_tag",
            json_name: "u16Tag",
            number: 3i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const U32_TAG_FIELD: &'static MessageField = &MessageField {
            name: "u32_tag",
            json_name: "u32Tag",
            number: 4i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const U64_TAG_FIELD: &'static MessageField = &MessageField {
            name: "u64_tag",
            json_name: "u64Tag",
            number: 5i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const U128_TAG_FIELD: &'static MessageField = &MessageField {
            name: "u128_tag",
            json_name: "u128Tag",
            number: 6i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const U256_TAG_FIELD: &'static MessageField = &MessageField {
            name: "u256_tag",
            json_name: "u256Tag",
            number: 7i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const ADDRESS_TAG_FIELD: &'static MessageField = &MessageField {
            name: "address_tag",
            json_name: "addressTag",
            number: 8i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const SIGNER_TAG_FIELD: &'static MessageField = &MessageField {
            name: "signer_tag",
            json_name: "signerTag",
            number: 9i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const VECTOR_TAG_FIELD: &'static MessageField = &MessageField {
            name: "vector_tag",
            json_name: "vectorTag",
            number: 10i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(TypeTagVector::FIELDS),
        };
        pub const STRUCT_TAG_FIELD: &'static MessageField = &MessageField {
            name: "struct_tag",
            json_name: "structTag",
            number: 11i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(TypeTagStruct::FIELDS),
        };
    }
    impl TypeTag {
        pub const TYPE_TAG_ONEOF: &'static str = "type_tag";
    }
    impl MessageFields for TypeTag {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::BOOL_TAG_FIELD,
            Self::U8_TAG_FIELD,
            Self::U16_TAG_FIELD,
            Self::U32_TAG_FIELD,
            Self::U64_TAG_FIELD,
            Self::U128_TAG_FIELD,
            Self::U256_TAG_FIELD,
            Self::ADDRESS_TAG_FIELD,
            Self::SIGNER_TAG_FIELD,
            Self::VECTOR_TAG_FIELD,
            Self::STRUCT_TAG_FIELD,
        ];
        const ONEOFS: &'static [&'static str] = &["type_tag"];
    }
    impl TypeTag {
        pub fn path_builder() -> TypeTagFieldPathBuilder {
            TypeTagFieldPathBuilder::new()
        }
    }
    pub struct TypeTagFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TypeTagFieldPathBuilder {
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
        pub fn bool_tag(mut self) -> String {
            self.path.push(TypeTag::BOOL_TAG_FIELD.name);
            self.finish()
        }
        pub fn u8_tag(mut self) -> String {
            self.path.push(TypeTag::U8_TAG_FIELD.name);
            self.finish()
        }
        pub fn u16_tag(mut self) -> String {
            self.path.push(TypeTag::U16_TAG_FIELD.name);
            self.finish()
        }
        pub fn u32_tag(mut self) -> String {
            self.path.push(TypeTag::U32_TAG_FIELD.name);
            self.finish()
        }
        pub fn u64_tag(mut self) -> String {
            self.path.push(TypeTag::U64_TAG_FIELD.name);
            self.finish()
        }
        pub fn u128_tag(mut self) -> String {
            self.path.push(TypeTag::U128_TAG_FIELD.name);
            self.finish()
        }
        pub fn u256_tag(mut self) -> String {
            self.path.push(TypeTag::U256_TAG_FIELD.name);
            self.finish()
        }
        pub fn address_tag(mut self) -> String {
            self.path.push(TypeTag::ADDRESS_TAG_FIELD.name);
            self.finish()
        }
        pub fn signer_tag(mut self) -> String {
            self.path.push(TypeTag::SIGNER_TAG_FIELD.name);
            self.finish()
        }
        pub fn vector_tag(mut self) -> TypeTagVectorFieldPathBuilder {
            self.path.push(TypeTag::VECTOR_TAG_FIELD.name);
            TypeTagVectorFieldPathBuilder::new_with_base(self.path)
        }
        pub fn struct_tag(mut self) -> TypeTagStructFieldPathBuilder {
            self.path.push(TypeTag::STRUCT_TAG_FIELD.name);
            TypeTagStructFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl TypeTags {
        pub const TYPE_TAGS_FIELD: &'static MessageField = &MessageField {
            name: "type_tags",
            json_name: "typeTags",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(TypeTag::FIELDS),
        };
    }
    impl MessageFields for TypeTags {
        const FIELDS: &'static [&'static MessageField] = &[Self::TYPE_TAGS_FIELD];
    }
    impl TypeTags {
        pub fn path_builder() -> TypeTagsFieldPathBuilder {
            TypeTagsFieldPathBuilder::new()
        }
    }
    pub struct TypeTagsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TypeTagsFieldPathBuilder {
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
        pub fn type_tags(mut self) -> TypeTagFieldPathBuilder {
            self.path.push(TypeTags::TYPE_TAGS_FIELD.name);
            TypeTagFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
