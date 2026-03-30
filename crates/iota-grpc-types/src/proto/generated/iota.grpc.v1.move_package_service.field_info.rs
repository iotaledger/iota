// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectId;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectIdFieldPathBuilder;
    impl ListPackageVersionsRequest {
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
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
        pub const MAX_MESSAGE_SIZE_BYTES_FIELD: &'static MessageField = &MessageField {
            name: "max_message_size_bytes",
            json_name: "maxMessageSizeBytes",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ListPackageVersionsRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PACKAGE_ID_FIELD,
            Self::PAGE_SIZE_FIELD,
            Self::PAGE_TOKEN_FIELD,
            Self::MAX_MESSAGE_SIZE_BYTES_FIELD,
        ];
    }
    impl ListPackageVersionsRequest {
        pub fn path_builder() -> ListPackageVersionsRequestFieldPathBuilder {
            ListPackageVersionsRequestFieldPathBuilder::new()
        }
    }
    pub struct ListPackageVersionsRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ListPackageVersionsRequestFieldPathBuilder {
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
        pub fn package_id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(ListPackageVersionsRequest::PACKAGE_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn page_size(mut self) -> String {
            self.path.push(ListPackageVersionsRequest::PAGE_SIZE_FIELD.name);
            self.finish()
        }
        pub fn page_token(mut self) -> String {
            self.path.push(ListPackageVersionsRequest::PAGE_TOKEN_FIELD.name);
            self.finish()
        }
        pub fn max_message_size_bytes(mut self) -> String {
            self.path
                .push(ListPackageVersionsRequest::MAX_MESSAGE_SIZE_BYTES_FIELD.name);
            self.finish()
        }
    }
    impl ListPackageVersionsResponse {
        pub const VERSIONS_FIELD: &'static MessageField = &MessageField {
            name: "versions",
            json_name: "versions",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(PackageVersion::FIELDS),
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
    impl MessageFields for ListPackageVersionsResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::VERSIONS_FIELD,
            Self::NEXT_PAGE_TOKEN_FIELD,
        ];
    }
    impl ListPackageVersionsResponse {
        pub fn path_builder() -> ListPackageVersionsResponseFieldPathBuilder {
            ListPackageVersionsResponseFieldPathBuilder::new()
        }
    }
    pub struct ListPackageVersionsResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ListPackageVersionsResponseFieldPathBuilder {
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
        pub fn versions(mut self) -> PackageVersionFieldPathBuilder {
            self.path.push(ListPackageVersionsResponse::VERSIONS_FIELD.name);
            PackageVersionFieldPathBuilder::new_with_base(self.path)
        }
        pub fn next_page_token(mut self) -> String {
            self.path.push(ListPackageVersionsResponse::NEXT_PAGE_TOKEN_FIELD.name);
            self.finish()
        }
    }
    impl PackageVersion {
        pub const ORIGINAL_ID_FIELD: &'static MessageField = &MessageField {
            name: "original_id",
            json_name: "originalId",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const STORAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "storage_id",
            json_name: "storageId",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const VERSION_FIELD: &'static MessageField = &MessageField {
            name: "version",
            json_name: "version",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for PackageVersion {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::ORIGINAL_ID_FIELD,
            Self::STORAGE_ID_FIELD,
            Self::VERSION_FIELD,
        ];
    }
    impl PackageVersion {
        pub fn path_builder() -> PackageVersionFieldPathBuilder {
            PackageVersionFieldPathBuilder::new()
        }
    }
    pub struct PackageVersionFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl PackageVersionFieldPathBuilder {
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
        pub fn original_id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(PackageVersion::ORIGINAL_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn storage_id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(PackageVersion::STORAGE_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn version(mut self) -> String {
            self.path.push(PackageVersion::VERSION_FIELD.name);
            self.finish()
        }
    }
}
pub use _field_impls::*;
