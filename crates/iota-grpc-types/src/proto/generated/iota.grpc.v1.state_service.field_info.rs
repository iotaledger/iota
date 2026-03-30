// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v1::coin::CoinMetadata;
    #[allow(unused_imports)]
    use crate::v1::coin::CoinMetadataFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::coin::CoinTreasury;
    #[allow(unused_imports)]
    use crate::v1::coin::CoinTreasuryFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::coin::RegulatedCoinMetadata;
    #[allow(unused_imports)]
    use crate::v1::coin::RegulatedCoinMetadataFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::dynamic_field::DynamicField;
    #[allow(unused_imports)]
    use crate::v1::dynamic_field::DynamicFieldFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::object::Object;
    #[allow(unused_imports)]
    use crate::v1::object::ObjectFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::Address;
    #[allow(unused_imports)]
    use crate::v1::types::AddressFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectId;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectIdFieldPathBuilder;
    impl ListDynamicFieldsRequest {
        pub const PARENT_FIELD: &'static MessageField = &MessageField {
            name: "parent",
            json_name: "parent",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const PAGE_SIZE_FIELD: &'static MessageField = &MessageField {
            name: "page_size",
            json_name: "pageSize",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const PAGE_TOKEN_FIELD: &'static MessageField = &MessageField {
            name: "page_token",
            json_name: "pageToken",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const MAX_MESSAGE_SIZE_BYTES_FIELD: &'static MessageField = &MessageField {
            name: "max_message_size_bytes",
            json_name: "maxMessageSizeBytes",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ListDynamicFieldsRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PARENT_FIELD,
            Self::PAGE_SIZE_FIELD,
            Self::PAGE_TOKEN_FIELD,
            Self::READ_MASK_FIELD,
            Self::MAX_MESSAGE_SIZE_BYTES_FIELD,
        ];
    }
    impl ListDynamicFieldsRequest {
        pub fn path_builder() -> ListDynamicFieldsRequestFieldPathBuilder {
            ListDynamicFieldsRequestFieldPathBuilder::new()
        }
    }
    pub struct ListDynamicFieldsRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ListDynamicFieldsRequestFieldPathBuilder {
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
        pub fn parent(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(ListDynamicFieldsRequest::PARENT_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn page_size(mut self) -> String {
            self.path.push(ListDynamicFieldsRequest::PAGE_SIZE_FIELD.name);
            self.finish()
        }
        pub fn page_token(mut self) -> String {
            self.path.push(ListDynamicFieldsRequest::PAGE_TOKEN_FIELD.name);
            self.finish()
        }
        pub fn read_mask(mut self) -> String {
            self.path.push(ListDynamicFieldsRequest::READ_MASK_FIELD.name);
            self.finish()
        }
        pub fn max_message_size_bytes(mut self) -> String {
            self.path.push(ListDynamicFieldsRequest::MAX_MESSAGE_SIZE_BYTES_FIELD.name);
            self.finish()
        }
    }
    impl ListDynamicFieldsResponse {
        pub const DYNAMIC_FIELDS_FIELD: &'static MessageField = &MessageField {
            name: "dynamic_fields",
            json_name: "dynamicFields",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(DynamicField::FIELDS),
        };
        pub const NEXT_PAGE_TOKEN_FIELD: &'static MessageField = &MessageField {
            name: "next_page_token",
            json_name: "nextPageToken",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ListDynamicFieldsResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::DYNAMIC_FIELDS_FIELD,
            Self::NEXT_PAGE_TOKEN_FIELD,
        ];
    }
    impl ListDynamicFieldsResponse {
        pub fn path_builder() -> ListDynamicFieldsResponseFieldPathBuilder {
            ListDynamicFieldsResponseFieldPathBuilder::new()
        }
    }
    pub struct ListDynamicFieldsResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ListDynamicFieldsResponseFieldPathBuilder {
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
        pub fn dynamic_fields(mut self) -> DynamicFieldFieldPathBuilder {
            self.path.push(ListDynamicFieldsResponse::DYNAMIC_FIELDS_FIELD.name);
            DynamicFieldFieldPathBuilder::new_with_base(self.path)
        }
        pub fn next_page_token(mut self) -> String {
            self.path.push(ListDynamicFieldsResponse::NEXT_PAGE_TOKEN_FIELD.name);
            self.finish()
        }
    }
    impl ListOwnedObjectsRequest {
        pub const OWNER_FIELD: &'static MessageField = &MessageField {
            name: "owner",
            json_name: "owner",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Address::FIELDS),
        };
        pub const PAGE_SIZE_FIELD: &'static MessageField = &MessageField {
            name: "page_size",
            json_name: "pageSize",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const PAGE_TOKEN_FIELD: &'static MessageField = &MessageField {
            name: "page_token",
            json_name: "pageToken",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const OBJECT_TYPE_FIELD: &'static MessageField = &MessageField {
            name: "object_type",
            json_name: "objectType",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const MAX_MESSAGE_SIZE_BYTES_FIELD: &'static MessageField = &MessageField {
            name: "max_message_size_bytes",
            json_name: "maxMessageSizeBytes",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ListOwnedObjectsRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::OWNER_FIELD,
            Self::PAGE_SIZE_FIELD,
            Self::PAGE_TOKEN_FIELD,
            Self::READ_MASK_FIELD,
            Self::OBJECT_TYPE_FIELD,
            Self::MAX_MESSAGE_SIZE_BYTES_FIELD,
        ];
    }
    impl ListOwnedObjectsRequest {
        pub fn path_builder() -> ListOwnedObjectsRequestFieldPathBuilder {
            ListOwnedObjectsRequestFieldPathBuilder::new()
        }
    }
    pub struct ListOwnedObjectsRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ListOwnedObjectsRequestFieldPathBuilder {
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
        pub fn owner(mut self) -> AddressFieldPathBuilder {
            self.path.push(ListOwnedObjectsRequest::OWNER_FIELD.name);
            AddressFieldPathBuilder::new_with_base(self.path)
        }
        pub fn page_size(mut self) -> String {
            self.path.push(ListOwnedObjectsRequest::PAGE_SIZE_FIELD.name);
            self.finish()
        }
        pub fn page_token(mut self) -> String {
            self.path.push(ListOwnedObjectsRequest::PAGE_TOKEN_FIELD.name);
            self.finish()
        }
        pub fn read_mask(mut self) -> String {
            self.path.push(ListOwnedObjectsRequest::READ_MASK_FIELD.name);
            self.finish()
        }
        pub fn object_type(mut self) -> String {
            self.path.push(ListOwnedObjectsRequest::OBJECT_TYPE_FIELD.name);
            self.finish()
        }
        pub fn max_message_size_bytes(mut self) -> String {
            self.path.push(ListOwnedObjectsRequest::MAX_MESSAGE_SIZE_BYTES_FIELD.name);
            self.finish()
        }
    }
    impl ListOwnedObjectsResponse {
        pub const OBJECTS_FIELD: &'static MessageField = &MessageField {
            name: "objects",
            json_name: "objects",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Object::FIELDS),
        };
        pub const NEXT_PAGE_TOKEN_FIELD: &'static MessageField = &MessageField {
            name: "next_page_token",
            json_name: "nextPageToken",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ListOwnedObjectsResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::OBJECTS_FIELD,
            Self::NEXT_PAGE_TOKEN_FIELD,
        ];
    }
    impl ListOwnedObjectsResponse {
        pub fn path_builder() -> ListOwnedObjectsResponseFieldPathBuilder {
            ListOwnedObjectsResponseFieldPathBuilder::new()
        }
    }
    pub struct ListOwnedObjectsResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ListOwnedObjectsResponseFieldPathBuilder {
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
            self.path.push(ListOwnedObjectsResponse::OBJECTS_FIELD.name);
            ObjectFieldPathBuilder::new_with_base(self.path)
        }
        pub fn next_page_token(mut self) -> String {
            self.path.push(ListOwnedObjectsResponse::NEXT_PAGE_TOKEN_FIELD.name);
            self.finish()
        }
    }
    impl GetCoinInfoRequest {
        pub const COIN_TYPE_FIELD: &'static MessageField = &MessageField {
            name: "coin_type",
            json_name: "coinType",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for GetCoinInfoRequest {
        const FIELDS: &'static [&'static MessageField] = &[Self::COIN_TYPE_FIELD];
    }
    impl GetCoinInfoRequest {
        pub fn path_builder() -> GetCoinInfoRequestFieldPathBuilder {
            GetCoinInfoRequestFieldPathBuilder::new()
        }
    }
    pub struct GetCoinInfoRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetCoinInfoRequestFieldPathBuilder {
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
        pub fn coin_type(mut self) -> String {
            self.path.push(GetCoinInfoRequest::COIN_TYPE_FIELD.name);
            self.finish()
        }
    }
    impl GetCoinInfoResponse {
        pub const COIN_TYPE_FIELD: &'static MessageField = &MessageField {
            name: "coin_type",
            json_name: "coinType",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const METADATA_FIELD: &'static MessageField = &MessageField {
            name: "metadata",
            json_name: "metadata",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(CoinMetadata::FIELDS),
        };
        pub const TREASURY_FIELD: &'static MessageField = &MessageField {
            name: "treasury",
            json_name: "treasury",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(CoinTreasury::FIELDS),
        };
        pub const REGULATED_METADATA_FIELD: &'static MessageField = &MessageField {
            name: "regulated_metadata",
            json_name: "regulatedMetadata",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(RegulatedCoinMetadata::FIELDS),
        };
    }
    impl MessageFields for GetCoinInfoResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::COIN_TYPE_FIELD,
            Self::METADATA_FIELD,
            Self::TREASURY_FIELD,
            Self::REGULATED_METADATA_FIELD,
        ];
    }
    impl GetCoinInfoResponse {
        pub fn path_builder() -> GetCoinInfoResponseFieldPathBuilder {
            GetCoinInfoResponseFieldPathBuilder::new()
        }
    }
    pub struct GetCoinInfoResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetCoinInfoResponseFieldPathBuilder {
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
        pub fn coin_type(mut self) -> String {
            self.path.push(GetCoinInfoResponse::COIN_TYPE_FIELD.name);
            self.finish()
        }
        pub fn metadata(mut self) -> CoinMetadataFieldPathBuilder {
            self.path.push(GetCoinInfoResponse::METADATA_FIELD.name);
            CoinMetadataFieldPathBuilder::new_with_base(self.path)
        }
        pub fn treasury(mut self) -> CoinTreasuryFieldPathBuilder {
            self.path.push(GetCoinInfoResponse::TREASURY_FIELD.name);
            CoinTreasuryFieldPathBuilder::new_with_base(self.path)
        }
        pub fn regulated_metadata(mut self) -> RegulatedCoinMetadataFieldPathBuilder {
            self.path.push(GetCoinInfoResponse::REGULATED_METADATA_FIELD.name);
            RegulatedCoinMetadataFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
