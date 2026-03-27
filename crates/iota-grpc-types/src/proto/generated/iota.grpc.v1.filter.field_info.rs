// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v1::types::Address;
    #[allow(unused_imports)]
    use crate::v1::types::AddressFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectId;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectIdFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectReference;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectReferenceFieldPathBuilder;
    impl AllEventFilter {
        pub const FILTERS_FIELD: &'static MessageField = &MessageField {
            name: "filters",
            json_name: "filters",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for AllEventFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::FILTERS_FIELD];
    }
    impl AllEventFilter {
        pub fn path_builder() -> AllEventFilterFieldPathBuilder {
            AllEventFilterFieldPathBuilder::new()
        }
    }
    pub struct AllEventFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl AllEventFilterFieldPathBuilder {
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
        pub fn filters(mut self) -> EventFilterFieldPathBuilder {
            self.path.push(AllEventFilter::FILTERS_FIELD.name);
            EventFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl AnyEventFilter {
        pub const FILTERS_FIELD: &'static MessageField = &MessageField {
            name: "filters",
            json_name: "filters",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for AnyEventFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::FILTERS_FIELD];
    }
    impl AnyEventFilter {
        pub fn path_builder() -> AnyEventFilterFieldPathBuilder {
            AnyEventFilterFieldPathBuilder::new()
        }
    }
    pub struct AnyEventFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl AnyEventFilterFieldPathBuilder {
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
        pub fn filters(mut self) -> EventFilterFieldPathBuilder {
            self.path.push(AnyEventFilter::FILTERS_FIELD.name);
            EventFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl NotEventFilter {
        pub const FILTER_FIELD: &'static MessageField = &MessageField {
            name: "filter",
            json_name: "filter",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for NotEventFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::FILTER_FIELD];
    }
    impl NotEventFilter {
        pub fn path_builder() -> NotEventFilterFieldPathBuilder {
            NotEventFilterFieldPathBuilder::new()
        }
    }
    pub struct NotEventFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl NotEventFilterFieldPathBuilder {
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
        pub fn filter(mut self) -> EventFilterFieldPathBuilder {
            self.path.push(NotEventFilter::FILTER_FIELD.name);
            EventFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl AddressFilter {
        pub const ADDRESS_FIELD: &'static MessageField = &MessageField {
            name: "address",
            json_name: "address",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Address::FIELDS),
        };
    }
    impl MessageFields for AddressFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::ADDRESS_FIELD];
    }
    impl AddressFilter {
        pub fn path_builder() -> AddressFilterFieldPathBuilder {
            AddressFilterFieldPathBuilder::new()
        }
    }
    pub struct AddressFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl AddressFilterFieldPathBuilder {
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
        pub fn address(mut self) -> AddressFieldPathBuilder {
            self.path.push(AddressFilter::ADDRESS_FIELD.name);
            AddressFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl MovePackageAndModuleFilter {
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const MODULE_FIELD: &'static MessageField = &MessageField {
            name: "module",
            json_name: "module",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for MovePackageAndModuleFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PACKAGE_ID_FIELD,
            Self::MODULE_FIELD,
        ];
    }
    impl MovePackageAndModuleFilter {
        pub fn path_builder() -> MovePackageAndModuleFilterFieldPathBuilder {
            MovePackageAndModuleFilterFieldPathBuilder::new()
        }
    }
    pub struct MovePackageAndModuleFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl MovePackageAndModuleFilterFieldPathBuilder {
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
            self.path.push(MovePackageAndModuleFilter::PACKAGE_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn module(mut self) -> String {
            self.path.push(MovePackageAndModuleFilter::MODULE_FIELD.name);
            self.finish()
        }
    }
    impl MoveEventTypeFilter {
        pub const STRUCT_TAG_FIELD: &'static MessageField = &MessageField {
            name: "struct_tag",
            json_name: "structTag",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for MoveEventTypeFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::STRUCT_TAG_FIELD];
    }
    impl MoveEventTypeFilter {
        pub fn path_builder() -> MoveEventTypeFilterFieldPathBuilder {
            MoveEventTypeFilterFieldPathBuilder::new()
        }
    }
    pub struct MoveEventTypeFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl MoveEventTypeFilterFieldPathBuilder {
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
            self.path.push(MoveEventTypeFilter::STRUCT_TAG_FIELD.name);
            self.finish()
        }
    }
    impl EventFilter {
        pub const ALL_FIELD: &'static MessageField = &MessageField {
            name: "all",
            json_name: "all",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(AllEventFilter::FIELDS),
        };
        pub const ANY_FIELD: &'static MessageField = &MessageField {
            name: "any",
            json_name: "any",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(AnyEventFilter::FIELDS),
        };
        pub const NEGATION_FIELD: &'static MessageField = &MessageField {
            name: "negation",
            json_name: "negation",
            number: 3i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(NotEventFilter::FIELDS),
        };
        pub const SENDER_FIELD: &'static MessageField = &MessageField {
            name: "sender",
            json_name: "sender",
            number: 4i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(AddressFilter::FIELDS),
        };
        pub const MOVE_PACKAGE_AND_MODULE_FIELD: &'static MessageField = &MessageField {
            name: "move_package_and_module",
            json_name: "movePackageAndModule",
            number: 5i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(MovePackageAndModuleFilter::FIELDS),
        };
        pub const MOVE_EVENT_PACKAGE_AND_MODULE_FIELD: &'static MessageField = &MessageField {
            name: "move_event_package_and_module",
            json_name: "moveEventPackageAndModule",
            number: 6i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(MovePackageAndModuleFilter::FIELDS),
        };
        pub const MOVE_EVENT_TYPE_FIELD: &'static MessageField = &MessageField {
            name: "move_event_type",
            json_name: "moveEventType",
            number: 7i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(MoveEventTypeFilter::FIELDS),
        };
    }
    impl EventFilter {
        pub const FILTER_ONEOF: &'static str = "filter";
    }
    impl MessageFields for EventFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::ALL_FIELD,
            Self::ANY_FIELD,
            Self::NEGATION_FIELD,
            Self::SENDER_FIELD,
            Self::MOVE_PACKAGE_AND_MODULE_FIELD,
            Self::MOVE_EVENT_PACKAGE_AND_MODULE_FIELD,
            Self::MOVE_EVENT_TYPE_FIELD,
        ];
        const ONEOFS: &'static [&'static str] = &["filter"];
    }
    impl EventFilter {
        pub fn path_builder() -> EventFilterFieldPathBuilder {
            EventFilterFieldPathBuilder::new()
        }
    }
    pub struct EventFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl EventFilterFieldPathBuilder {
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
        pub fn all(mut self) -> AllEventFilterFieldPathBuilder {
            self.path.push(EventFilter::ALL_FIELD.name);
            AllEventFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn any(mut self) -> AnyEventFilterFieldPathBuilder {
            self.path.push(EventFilter::ANY_FIELD.name);
            AnyEventFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn negation(mut self) -> NotEventFilterFieldPathBuilder {
            self.path.push(EventFilter::NEGATION_FIELD.name);
            NotEventFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn sender(mut self) -> AddressFilterFieldPathBuilder {
            self.path.push(EventFilter::SENDER_FIELD.name);
            AddressFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn move_package_and_module(
            mut self,
        ) -> MovePackageAndModuleFilterFieldPathBuilder {
            self.path.push(EventFilter::MOVE_PACKAGE_AND_MODULE_FIELD.name);
            MovePackageAndModuleFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn move_event_package_and_module(
            mut self,
        ) -> MovePackageAndModuleFilterFieldPathBuilder {
            self.path.push(EventFilter::MOVE_EVENT_PACKAGE_AND_MODULE_FIELD.name);
            MovePackageAndModuleFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn move_event_type(mut self) -> MoveEventTypeFilterFieldPathBuilder {
            self.path.push(EventFilter::MOVE_EVENT_TYPE_FIELD.name);
            MoveEventTypeFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl AllTransactionFilter {
        pub const FILTERS_FIELD: &'static MessageField = &MessageField {
            name: "filters",
            json_name: "filters",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for AllTransactionFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::FILTERS_FIELD];
    }
    impl AllTransactionFilter {
        pub fn path_builder() -> AllTransactionFilterFieldPathBuilder {
            AllTransactionFilterFieldPathBuilder::new()
        }
    }
    pub struct AllTransactionFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl AllTransactionFilterFieldPathBuilder {
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
        pub fn filters(mut self) -> TransactionFilterFieldPathBuilder {
            self.path.push(AllTransactionFilter::FILTERS_FIELD.name);
            TransactionFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl AnyTransactionFilter {
        pub const FILTERS_FIELD: &'static MessageField = &MessageField {
            name: "filters",
            json_name: "filters",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for AnyTransactionFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::FILTERS_FIELD];
    }
    impl AnyTransactionFilter {
        pub fn path_builder() -> AnyTransactionFilterFieldPathBuilder {
            AnyTransactionFilterFieldPathBuilder::new()
        }
    }
    pub struct AnyTransactionFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl AnyTransactionFilterFieldPathBuilder {
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
        pub fn filters(mut self) -> TransactionFilterFieldPathBuilder {
            self.path.push(AnyTransactionFilter::FILTERS_FIELD.name);
            TransactionFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl NotTransactionFilter {
        pub const FILTER_FIELD: &'static MessageField = &MessageField {
            name: "filter",
            json_name: "filter",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for NotTransactionFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::FILTER_FIELD];
    }
    impl NotTransactionFilter {
        pub fn path_builder() -> NotTransactionFilterFieldPathBuilder {
            NotTransactionFilterFieldPathBuilder::new()
        }
    }
    pub struct NotTransactionFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl NotTransactionFilterFieldPathBuilder {
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
        pub fn filter(mut self) -> TransactionFilterFieldPathBuilder {
            self.path.push(NotTransactionFilter::FILTER_FIELD.name);
            TransactionFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl TransactionKindsFilter {
        pub const KINDS_FIELD: &'static MessageField = &MessageField {
            name: "kinds",
            json_name: "kinds",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for TransactionKindsFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::KINDS_FIELD];
    }
    impl TransactionKindsFilter {
        pub fn path_builder() -> TransactionKindsFilterFieldPathBuilder {
            TransactionKindsFilterFieldPathBuilder::new()
        }
    }
    pub struct TransactionKindsFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TransactionKindsFilterFieldPathBuilder {
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
        pub fn kinds(mut self) -> String {
            self.path.push(TransactionKindsFilter::KINDS_FIELD.name);
            self.finish()
        }
    }
    impl ObjectIdFilter {
        pub const OBJECT_REF_FIELD: &'static MessageField = &MessageField {
            name: "object_ref",
            json_name: "objectRef",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ObjectReference::FIELDS),
        };
    }
    impl MessageFields for ObjectIdFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::OBJECT_REF_FIELD];
    }
    impl ObjectIdFilter {
        pub fn path_builder() -> ObjectIdFilterFieldPathBuilder {
            ObjectIdFilterFieldPathBuilder::new()
        }
    }
    pub struct ObjectIdFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ObjectIdFilterFieldPathBuilder {
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
        pub fn object_ref(mut self) -> ObjectReferenceFieldPathBuilder {
            self.path.push(ObjectIdFilter::OBJECT_REF_FIELD.name);
            ObjectReferenceFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl MoveCallCommandFilter {
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const MODULE_FIELD: &'static MessageField = &MessageField {
            name: "module",
            json_name: "module",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const FUNCTION_FIELD: &'static MessageField = &MessageField {
            name: "function",
            json_name: "function",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for MoveCallCommandFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PACKAGE_ID_FIELD,
            Self::MODULE_FIELD,
            Self::FUNCTION_FIELD,
        ];
    }
    impl MoveCallCommandFilter {
        pub fn path_builder() -> MoveCallCommandFilterFieldPathBuilder {
            MoveCallCommandFilterFieldPathBuilder::new()
        }
    }
    pub struct MoveCallCommandFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl MoveCallCommandFilterFieldPathBuilder {
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
            self.path.push(MoveCallCommandFilter::PACKAGE_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn module(mut self) -> String {
            self.path.push(MoveCallCommandFilter::MODULE_FIELD.name);
            self.finish()
        }
        pub fn function(mut self) -> String {
            self.path.push(MoveCallCommandFilter::FUNCTION_FIELD.name);
            self.finish()
        }
    }
    impl TransferObjectsCommandFilter {}
    impl MessageFields for TransferObjectsCommandFilter {
        const FIELDS: &'static [&'static MessageField] = &[];
    }
    impl TransferObjectsCommandFilter {
        pub fn path_builder() -> TransferObjectsCommandFilterFieldPathBuilder {
            TransferObjectsCommandFilterFieldPathBuilder::new()
        }
    }
    pub struct TransferObjectsCommandFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TransferObjectsCommandFilterFieldPathBuilder {
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
    }
    impl SplitCoinsCommandFilter {}
    impl MessageFields for SplitCoinsCommandFilter {
        const FIELDS: &'static [&'static MessageField] = &[];
    }
    impl SplitCoinsCommandFilter {
        pub fn path_builder() -> SplitCoinsCommandFilterFieldPathBuilder {
            SplitCoinsCommandFilterFieldPathBuilder::new()
        }
    }
    pub struct SplitCoinsCommandFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl SplitCoinsCommandFilterFieldPathBuilder {
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
    }
    impl MergeCoinsCommandFilter {}
    impl MessageFields for MergeCoinsCommandFilter {
        const FIELDS: &'static [&'static MessageField] = &[];
    }
    impl MergeCoinsCommandFilter {
        pub fn path_builder() -> MergeCoinsCommandFilterFieldPathBuilder {
            MergeCoinsCommandFilterFieldPathBuilder::new()
        }
    }
    pub struct MergeCoinsCommandFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl MergeCoinsCommandFilterFieldPathBuilder {
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
    }
    impl PublishCommandFilter {}
    impl MessageFields for PublishCommandFilter {
        const FIELDS: &'static [&'static MessageField] = &[];
    }
    impl PublishCommandFilter {
        pub fn path_builder() -> PublishCommandFilterFieldPathBuilder {
            PublishCommandFilterFieldPathBuilder::new()
        }
    }
    pub struct PublishCommandFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl PublishCommandFilterFieldPathBuilder {
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
    }
    impl MakeMoveVecCommandFilter {}
    impl MessageFields for MakeMoveVecCommandFilter {
        const FIELDS: &'static [&'static MessageField] = &[];
    }
    impl MakeMoveVecCommandFilter {
        pub fn path_builder() -> MakeMoveVecCommandFilterFieldPathBuilder {
            MakeMoveVecCommandFilterFieldPathBuilder::new()
        }
    }
    pub struct MakeMoveVecCommandFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl MakeMoveVecCommandFilterFieldPathBuilder {
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
    }
    impl UpgradeCommandFilter {
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
    }
    impl MessageFields for UpgradeCommandFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::PACKAGE_ID_FIELD];
    }
    impl UpgradeCommandFilter {
        pub fn path_builder() -> UpgradeCommandFilterFieldPathBuilder {
            UpgradeCommandFilterFieldPathBuilder::new()
        }
    }
    pub struct UpgradeCommandFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl UpgradeCommandFilterFieldPathBuilder {
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
            self.path.push(UpgradeCommandFilter::PACKAGE_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl CommandFilter {
        pub const MOVE_CALL_FIELD: &'static MessageField = &MessageField {
            name: "move_call",
            json_name: "moveCall",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(MoveCallCommandFilter::FIELDS),
        };
        pub const TRANSFER_OBJECTS_FIELD: &'static MessageField = &MessageField {
            name: "transfer_objects",
            json_name: "transferObjects",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(TransferObjectsCommandFilter::FIELDS),
        };
        pub const SPLIT_COINS_FIELD: &'static MessageField = &MessageField {
            name: "split_coins",
            json_name: "splitCoins",
            number: 3i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(SplitCoinsCommandFilter::FIELDS),
        };
        pub const MERGE_COINS_FIELD: &'static MessageField = &MessageField {
            name: "merge_coins",
            json_name: "mergeCoins",
            number: 4i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(MergeCoinsCommandFilter::FIELDS),
        };
        pub const PUBLISH_FIELD: &'static MessageField = &MessageField {
            name: "publish",
            json_name: "publish",
            number: 5i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(PublishCommandFilter::FIELDS),
        };
        pub const MAKE_MOVE_VEC_FIELD: &'static MessageField = &MessageField {
            name: "make_move_vec",
            json_name: "makeMoveVec",
            number: 6i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(MakeMoveVecCommandFilter::FIELDS),
        };
        pub const UPGRADE_FIELD: &'static MessageField = &MessageField {
            name: "upgrade",
            json_name: "upgrade",
            number: 7i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(UpgradeCommandFilter::FIELDS),
        };
    }
    impl CommandFilter {
        pub const FILTER_ONEOF: &'static str = "filter";
    }
    impl MessageFields for CommandFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::MOVE_CALL_FIELD,
            Self::TRANSFER_OBJECTS_FIELD,
            Self::SPLIT_COINS_FIELD,
            Self::MERGE_COINS_FIELD,
            Self::PUBLISH_FIELD,
            Self::MAKE_MOVE_VEC_FIELD,
            Self::UPGRADE_FIELD,
        ];
        const ONEOFS: &'static [&'static str] = &["filter"];
    }
    impl CommandFilter {
        pub fn path_builder() -> CommandFilterFieldPathBuilder {
            CommandFilterFieldPathBuilder::new()
        }
    }
    pub struct CommandFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CommandFilterFieldPathBuilder {
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
        pub fn move_call(mut self) -> MoveCallCommandFilterFieldPathBuilder {
            self.path.push(CommandFilter::MOVE_CALL_FIELD.name);
            MoveCallCommandFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn transfer_objects(
            mut self,
        ) -> TransferObjectsCommandFilterFieldPathBuilder {
            self.path.push(CommandFilter::TRANSFER_OBJECTS_FIELD.name);
            TransferObjectsCommandFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn split_coins(mut self) -> SplitCoinsCommandFilterFieldPathBuilder {
            self.path.push(CommandFilter::SPLIT_COINS_FIELD.name);
            SplitCoinsCommandFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn merge_coins(mut self) -> MergeCoinsCommandFilterFieldPathBuilder {
            self.path.push(CommandFilter::MERGE_COINS_FIELD.name);
            MergeCoinsCommandFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn publish(mut self) -> PublishCommandFilterFieldPathBuilder {
            self.path.push(CommandFilter::PUBLISH_FIELD.name);
            PublishCommandFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn make_move_vec(mut self) -> MakeMoveVecCommandFilterFieldPathBuilder {
            self.path.push(CommandFilter::MAKE_MOVE_VEC_FIELD.name);
            MakeMoveVecCommandFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn upgrade(mut self) -> UpgradeCommandFilterFieldPathBuilder {
            self.path.push(CommandFilter::UPGRADE_FIELD.name);
            UpgradeCommandFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl ExecutionStatusFilter {
        pub const SUCCESS_FIELD: &'static MessageField = &MessageField {
            name: "success",
            json_name: "success",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for ExecutionStatusFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::SUCCESS_FIELD];
    }
    impl ExecutionStatusFilter {
        pub fn path_builder() -> ExecutionStatusFilterFieldPathBuilder {
            ExecutionStatusFilterFieldPathBuilder::new()
        }
    }
    pub struct ExecutionStatusFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ExecutionStatusFilterFieldPathBuilder {
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
        pub fn success(mut self) -> String {
            self.path.push(ExecutionStatusFilter::SUCCESS_FIELD.name);
            self.finish()
        }
    }
    impl TransactionFilter {
        pub const ALL_FIELD: &'static MessageField = &MessageField {
            name: "all",
            json_name: "all",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(AllTransactionFilter::FIELDS),
        };
        pub const ANY_FIELD: &'static MessageField = &MessageField {
            name: "any",
            json_name: "any",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(AnyTransactionFilter::FIELDS),
        };
        pub const NEGATION_FIELD: &'static MessageField = &MessageField {
            name: "negation",
            json_name: "negation",
            number: 3i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(NotTransactionFilter::FIELDS),
        };
        pub const TRANSACTION_KINDS_FIELD: &'static MessageField = &MessageField {
            name: "transaction_kinds",
            json_name: "transactionKinds",
            number: 4i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(TransactionKindsFilter::FIELDS),
        };
        pub const EXECUTION_STATUS_FIELD: &'static MessageField = &MessageField {
            name: "execution_status",
            json_name: "executionStatus",
            number: 5i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ExecutionStatusFilter::FIELDS),
        };
        pub const SENDER_FIELD: &'static MessageField = &MessageField {
            name: "sender",
            json_name: "sender",
            number: 6i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(AddressFilter::FIELDS),
        };
        pub const RECEIVER_FIELD: &'static MessageField = &MessageField {
            name: "receiver",
            json_name: "receiver",
            number: 7i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(AddressFilter::FIELDS),
        };
        pub const AFFECTED_OBJECT_FIELD: &'static MessageField = &MessageField {
            name: "affected_object",
            json_name: "affectedObject",
            number: 8i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ObjectIdFilter::FIELDS),
        };
        pub const COMMAND_FIELD: &'static MessageField = &MessageField {
            name: "command",
            json_name: "command",
            number: 9i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(CommandFilter::FIELDS),
        };
        pub const EVENT_FIELD: &'static MessageField = &MessageField {
            name: "event",
            json_name: "event",
            number: 10i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(EventFilter::FIELDS),
        };
    }
    impl TransactionFilter {
        pub const FILTER_ONEOF: &'static str = "filter";
    }
    impl MessageFields for TransactionFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::ALL_FIELD,
            Self::ANY_FIELD,
            Self::NEGATION_FIELD,
            Self::TRANSACTION_KINDS_FIELD,
            Self::EXECUTION_STATUS_FIELD,
            Self::SENDER_FIELD,
            Self::RECEIVER_FIELD,
            Self::AFFECTED_OBJECT_FIELD,
            Self::COMMAND_FIELD,
            Self::EVENT_FIELD,
        ];
        const ONEOFS: &'static [&'static str] = &["filter"];
    }
    impl TransactionFilter {
        pub fn path_builder() -> TransactionFilterFieldPathBuilder {
            TransactionFilterFieldPathBuilder::new()
        }
    }
    pub struct TransactionFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TransactionFilterFieldPathBuilder {
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
        pub fn all(mut self) -> AllTransactionFilterFieldPathBuilder {
            self.path.push(TransactionFilter::ALL_FIELD.name);
            AllTransactionFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn any(mut self) -> AnyTransactionFilterFieldPathBuilder {
            self.path.push(TransactionFilter::ANY_FIELD.name);
            AnyTransactionFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn negation(mut self) -> NotTransactionFilterFieldPathBuilder {
            self.path.push(TransactionFilter::NEGATION_FIELD.name);
            NotTransactionFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn transaction_kinds(mut self) -> TransactionKindsFilterFieldPathBuilder {
            self.path.push(TransactionFilter::TRANSACTION_KINDS_FIELD.name);
            TransactionKindsFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn execution_status(mut self) -> ExecutionStatusFilterFieldPathBuilder {
            self.path.push(TransactionFilter::EXECUTION_STATUS_FIELD.name);
            ExecutionStatusFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn sender(mut self) -> AddressFilterFieldPathBuilder {
            self.path.push(TransactionFilter::SENDER_FIELD.name);
            AddressFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn receiver(mut self) -> AddressFilterFieldPathBuilder {
            self.path.push(TransactionFilter::RECEIVER_FIELD.name);
            AddressFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn affected_object(mut self) -> ObjectIdFilterFieldPathBuilder {
            self.path.push(TransactionFilter::AFFECTED_OBJECT_FIELD.name);
            ObjectIdFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn command(mut self) -> CommandFilterFieldPathBuilder {
            self.path.push(TransactionFilter::COMMAND_FIELD.name);
            CommandFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn event(mut self) -> EventFilterFieldPathBuilder {
            self.path.push(TransactionFilter::EVENT_FIELD.name);
            EventFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
