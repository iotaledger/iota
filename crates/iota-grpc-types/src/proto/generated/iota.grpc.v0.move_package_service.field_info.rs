// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v0::types::ObjectId;
    #[allow(unused_imports)]
    use crate::v0::types::ObjectIdFieldPathBuilder;
    impl ListPackageVersionsRequest {
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const LIMIT_FIELD: &'static MessageField = &MessageField {
            name: "limit",
            json_name: "limit",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const MAX_MESSAGE_SIZE_BYTES_FIELD: &'static MessageField = &MessageField {
            name: "max_message_size_bytes",
            json_name: "maxMessageSizeBytes",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ListPackageVersionsRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PACKAGE_ID_FIELD,
            Self::LIMIT_FIELD,
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
        pub fn limit(mut self) -> String {
            self.path.push(ListPackageVersionsRequest::LIMIT_FIELD.name);
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
        pub const HAS_NEXT_FIELD: &'static MessageField = &MessageField {
            name: "has_next",
            json_name: "hasNext",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ListPackageVersionsResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::VERSIONS_FIELD,
            Self::HAS_NEXT_FIELD,
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
        pub fn has_next(mut self) -> String {
            self.path.push(ListPackageVersionsResponse::HAS_NEXT_FIELD.name);
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
        pub const VERSION_FIELD: &'static MessageField = &MessageField {
            name: "version",
            json_name: "version",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const STORAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "storage_id",
            json_name: "storageId",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
    }
    impl MessageFields for PackageVersion {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::ORIGINAL_ID_FIELD,
            Self::VERSION_FIELD,
            Self::STORAGE_ID_FIELD,
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
        pub fn version(mut self) -> String {
            self.path.push(PackageVersion::VERSION_FIELD.name);
            self.finish()
        }
        pub fn storage_id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(PackageVersion::STORAGE_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
