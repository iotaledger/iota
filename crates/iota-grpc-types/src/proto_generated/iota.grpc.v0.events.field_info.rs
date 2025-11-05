// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v0::common::Address;
    #[allow(unused_imports)]
    use crate::v0::common::AddressFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v0::common::BcsData;
    #[allow(unused_imports)]
    use crate::v0::common::BcsDataFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v0::common::TransactionDigest;
    #[allow(unused_imports)]
    use crate::v0::common::TransactionDigestFieldPathBuilder;
    impl EventStreamRequest {
        pub const FILTER_FIELD: &'static MessageField = &MessageField {
            name: "filter",
            json_name: "filter",
            number: 1i32,
            message_fields: Some(EventFilter::FIELDS),
        };
    }
    impl MessageFields for EventStreamRequest {
        const FIELDS: &'static [&'static MessageField] = &[Self::FILTER_FIELD];
    }
    impl EventStreamRequest {
        pub fn path_builder() -> EventStreamRequestFieldPathBuilder {
            EventStreamRequestFieldPathBuilder::new()
        }
    }
    pub struct EventStreamRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl EventStreamRequestFieldPathBuilder {
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
            self.path.push(EventStreamRequest::FILTER_FIELD.name);
            EventFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl Event {
        pub const EVENT_ID_FIELD: &'static MessageField = &MessageField {
            name: "event_id",
            json_name: "eventId",
            number: 1i32,
            message_fields: Some(EventId::FIELDS),
        };
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
            number: 2i32,
            message_fields: Some(Address::FIELDS),
        };
        pub const TRANSACTION_MODULE_FIELD: &'static MessageField = &MessageField {
            name: "transaction_module",
            json_name: "transactionModule",
            number: 3i32,
            message_fields: None,
        };
        pub const SENDER_FIELD: &'static MessageField = &MessageField {
            name: "sender",
            json_name: "sender",
            number: 4i32,
            message_fields: Some(Address::FIELDS),
        };
        pub const TYPE_NAME_FIELD: &'static MessageField = &MessageField {
            name: "type_name",
            json_name: "typeName",
            number: 5i32,
            message_fields: None,
        };
        pub const PARSED_JSON_FIELD: &'static MessageField = &MessageField {
            name: "parsed_json",
            json_name: "parsedJson",
            number: 6i32,
            message_fields: None,
        };
        pub const TIMESTAMP_MS_FIELD: &'static MessageField = &MessageField {
            name: "timestamp_ms",
            json_name: "timestampMs",
            number: 7i32,
            message_fields: None,
        };
        pub const EVENT_DATA_FIELD: &'static MessageField = &MessageField {
            name: "event_data",
            json_name: "eventData",
            number: 8i32,
            message_fields: Some(BcsData::FIELDS),
        };
    }
    impl MessageFields for Event {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::EVENT_ID_FIELD,
            Self::PACKAGE_ID_FIELD,
            Self::TRANSACTION_MODULE_FIELD,
            Self::SENDER_FIELD,
            Self::TYPE_NAME_FIELD,
            Self::PARSED_JSON_FIELD,
            Self::TIMESTAMP_MS_FIELD,
            Self::EVENT_DATA_FIELD,
        ];
    }
    impl Event {
        pub fn path_builder() -> EventFieldPathBuilder {
            EventFieldPathBuilder::new()
        }
    }
    pub struct EventFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl EventFieldPathBuilder {
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
        pub fn event_id(mut self) -> EventIdFieldPathBuilder {
            self.path.push(Event::EVENT_ID_FIELD.name);
            EventIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn package_id(mut self) -> AddressFieldPathBuilder {
            self.path.push(Event::PACKAGE_ID_FIELD.name);
            AddressFieldPathBuilder::new_with_base(self.path)
        }
        pub fn transaction_module(mut self) -> String {
            self.path.push(Event::TRANSACTION_MODULE_FIELD.name);
            self.finish()
        }
        pub fn sender(mut self) -> AddressFieldPathBuilder {
            self.path.push(Event::SENDER_FIELD.name);
            AddressFieldPathBuilder::new_with_base(self.path)
        }
        pub fn type_name(mut self) -> String {
            self.path.push(Event::TYPE_NAME_FIELD.name);
            self.finish()
        }
        pub fn parsed_json(mut self) -> String {
            self.path.push(Event::PARSED_JSON_FIELD.name);
            self.finish()
        }
        pub fn timestamp_ms(mut self) -> String {
            self.path.push(Event::TIMESTAMP_MS_FIELD.name);
            self.finish()
        }
        pub fn event_data(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(Event::EVENT_DATA_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl EventId {
        pub const EVENT_SEQ_FIELD: &'static MessageField = &MessageField {
            name: "event_seq",
            json_name: "eventSeq",
            number: 1i32,
            message_fields: None,
        };
        pub const TX_DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "tx_digest",
            json_name: "txDigest",
            number: 2i32,
            message_fields: Some(TransactionDigest::FIELDS),
        };
    }
    impl MessageFields for EventId {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::EVENT_SEQ_FIELD,
            Self::TX_DIGEST_FIELD,
        ];
    }
    impl EventId {
        pub fn path_builder() -> EventIdFieldPathBuilder {
            EventIdFieldPathBuilder::new()
        }
    }
    pub struct EventIdFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl EventIdFieldPathBuilder {
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
        pub fn event_seq(mut self) -> String {
            self.path.push(EventId::EVENT_SEQ_FIELD.name);
            self.finish()
        }
        pub fn tx_digest(mut self) -> TransactionDigestFieldPathBuilder {
            self.path.push(EventId::TX_DIGEST_FIELD.name);
            TransactionDigestFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl EventFilter {
        pub const ALL_FIELD: &'static MessageField = &MessageField {
            name: "all",
            json_name: "all",
            number: 1i32,
            message_fields: Some(AllFilter::FIELDS),
        };
        pub const SENDER_FIELD: &'static MessageField = &MessageField {
            name: "sender",
            json_name: "sender",
            number: 2i32,
            message_fields: Some(SenderFilter::FIELDS),
        };
        pub const TRANSACTION_FIELD: &'static MessageField = &MessageField {
            name: "transaction",
            json_name: "transaction",
            number: 3i32,
            message_fields: Some(TransactionFilter::FIELDS),
        };
        pub const MOVE_MODULE_FIELD: &'static MessageField = &MessageField {
            name: "move_module",
            json_name: "moveModule",
            number: 4i32,
            message_fields: Some(MoveModuleFilter::FIELDS),
        };
        pub const MOVE_EVENT_TYPE_FIELD: &'static MessageField = &MessageField {
            name: "move_event_type",
            json_name: "moveEventType",
            number: 5i32,
            message_fields: Some(MoveEventTypeFilter::FIELDS),
        };
        pub const MOVE_EVENT_MODULE_FIELD: &'static MessageField = &MessageField {
            name: "move_event_module",
            json_name: "moveEventModule",
            number: 6i32,
            message_fields: Some(MoveEventModuleFilter::FIELDS),
        };
        pub const TIME_RANGE_FIELD: &'static MessageField = &MessageField {
            name: "time_range",
            json_name: "timeRange",
            number: 7i32,
            message_fields: Some(TimeRangeFilter::FIELDS),
        };
    }
    impl MessageFields for EventFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::ALL_FIELD,
            Self::SENDER_FIELD,
            Self::TRANSACTION_FIELD,
            Self::MOVE_MODULE_FIELD,
            Self::MOVE_EVENT_TYPE_FIELD,
            Self::MOVE_EVENT_MODULE_FIELD,
            Self::TIME_RANGE_FIELD,
        ];
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
        pub fn all(mut self) -> AllFilterFieldPathBuilder {
            self.path.push(EventFilter::ALL_FIELD.name);
            AllFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn sender(mut self) -> SenderFilterFieldPathBuilder {
            self.path.push(EventFilter::SENDER_FIELD.name);
            SenderFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn transaction(mut self) -> TransactionFilterFieldPathBuilder {
            self.path.push(EventFilter::TRANSACTION_FIELD.name);
            TransactionFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn move_module(mut self) -> MoveModuleFilterFieldPathBuilder {
            self.path.push(EventFilter::MOVE_MODULE_FIELD.name);
            MoveModuleFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn move_event_type(mut self) -> MoveEventTypeFilterFieldPathBuilder {
            self.path.push(EventFilter::MOVE_EVENT_TYPE_FIELD.name);
            MoveEventTypeFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn move_event_module(mut self) -> MoveEventModuleFilterFieldPathBuilder {
            self.path.push(EventFilter::MOVE_EVENT_MODULE_FIELD.name);
            MoveEventModuleFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn time_range(mut self) -> TimeRangeFilterFieldPathBuilder {
            self.path.push(EventFilter::TIME_RANGE_FIELD.name);
            TimeRangeFilterFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl AllFilter {}
    impl MessageFields for AllFilter {
        const FIELDS: &'static [&'static MessageField] = &[];
    }
    impl AllFilter {
        pub fn path_builder() -> AllFilterFieldPathBuilder {
            AllFilterFieldPathBuilder::new()
        }
    }
    pub struct AllFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl AllFilterFieldPathBuilder {
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
    impl SenderFilter {
        pub const SENDER_FIELD: &'static MessageField = &MessageField {
            name: "sender",
            json_name: "sender",
            number: 1i32,
            message_fields: Some(Address::FIELDS),
        };
    }
    impl MessageFields for SenderFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::SENDER_FIELD];
    }
    impl SenderFilter {
        pub fn path_builder() -> SenderFilterFieldPathBuilder {
            SenderFilterFieldPathBuilder::new()
        }
    }
    pub struct SenderFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl SenderFilterFieldPathBuilder {
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
        pub fn sender(mut self) -> AddressFieldPathBuilder {
            self.path.push(SenderFilter::SENDER_FIELD.name);
            AddressFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl TransactionFilter {
        pub const TX_DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "tx_digest",
            json_name: "txDigest",
            number: 1i32,
            message_fields: Some(TransactionDigest::FIELDS),
        };
    }
    impl MessageFields for TransactionFilter {
        const FIELDS: &'static [&'static MessageField] = &[Self::TX_DIGEST_FIELD];
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
        pub fn tx_digest(mut self) -> TransactionDigestFieldPathBuilder {
            self.path.push(TransactionFilter::TX_DIGEST_FIELD.name);
            TransactionDigestFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl MoveModuleFilter {
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
            number: 1i32,
            message_fields: Some(Address::FIELDS),
        };
        pub const MODULE_FIELD: &'static MessageField = &MessageField {
            name: "module",
            json_name: "module",
            number: 2i32,
            message_fields: None,
        };
    }
    impl MessageFields for MoveModuleFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PACKAGE_ID_FIELD,
            Self::MODULE_FIELD,
        ];
    }
    impl MoveModuleFilter {
        pub fn path_builder() -> MoveModuleFilterFieldPathBuilder {
            MoveModuleFilterFieldPathBuilder::new()
        }
    }
    pub struct MoveModuleFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl MoveModuleFilterFieldPathBuilder {
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
        pub fn package_id(mut self) -> AddressFieldPathBuilder {
            self.path.push(MoveModuleFilter::PACKAGE_ID_FIELD.name);
            AddressFieldPathBuilder::new_with_base(self.path)
        }
        pub fn module(mut self) -> String {
            self.path.push(MoveModuleFilter::MODULE_FIELD.name);
            self.finish()
        }
    }
    impl MoveEventTypeFilter {
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
            number: 1i32,
            message_fields: Some(Address::FIELDS),
        };
        pub const MODULE_FIELD: &'static MessageField = &MessageField {
            name: "module",
            json_name: "module",
            number: 2i32,
            message_fields: None,
        };
        pub const NAME_FIELD: &'static MessageField = &MessageField {
            name: "name",
            json_name: "name",
            number: 3i32,
            message_fields: None,
        };
    }
    impl MessageFields for MoveEventTypeFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PACKAGE_ID_FIELD,
            Self::MODULE_FIELD,
            Self::NAME_FIELD,
        ];
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
        pub fn package_id(mut self) -> AddressFieldPathBuilder {
            self.path.push(MoveEventTypeFilter::PACKAGE_ID_FIELD.name);
            AddressFieldPathBuilder::new_with_base(self.path)
        }
        pub fn module(mut self) -> String {
            self.path.push(MoveEventTypeFilter::MODULE_FIELD.name);
            self.finish()
        }
        pub fn name(mut self) -> String {
            self.path.push(MoveEventTypeFilter::NAME_FIELD.name);
            self.finish()
        }
    }
    impl MoveEventModuleFilter {
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
            number: 1i32,
            message_fields: Some(Address::FIELDS),
        };
        pub const MODULE_FIELD: &'static MessageField = &MessageField {
            name: "module",
            json_name: "module",
            number: 2i32,
            message_fields: None,
        };
    }
    impl MessageFields for MoveEventModuleFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::PACKAGE_ID_FIELD,
            Self::MODULE_FIELD,
        ];
    }
    impl MoveEventModuleFilter {
        pub fn path_builder() -> MoveEventModuleFilterFieldPathBuilder {
            MoveEventModuleFilterFieldPathBuilder::new()
        }
    }
    pub struct MoveEventModuleFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl MoveEventModuleFilterFieldPathBuilder {
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
        pub fn package_id(mut self) -> AddressFieldPathBuilder {
            self.path.push(MoveEventModuleFilter::PACKAGE_ID_FIELD.name);
            AddressFieldPathBuilder::new_with_base(self.path)
        }
        pub fn module(mut self) -> String {
            self.path.push(MoveEventModuleFilter::MODULE_FIELD.name);
            self.finish()
        }
    }
    impl TimeRangeFilter {
        pub const START_TIME_FIELD: &'static MessageField = &MessageField {
            name: "start_time",
            json_name: "startTime",
            number: 1i32,
            message_fields: None,
        };
        pub const END_TIME_FIELD: &'static MessageField = &MessageField {
            name: "end_time",
            json_name: "endTime",
            number: 2i32,
            message_fields: None,
        };
    }
    impl MessageFields for TimeRangeFilter {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::START_TIME_FIELD,
            Self::END_TIME_FIELD,
        ];
    }
    impl TimeRangeFilter {
        pub fn path_builder() -> TimeRangeFilterFieldPathBuilder {
            TimeRangeFilterFieldPathBuilder::new()
        }
    }
    pub struct TimeRangeFilterFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TimeRangeFilterFieldPathBuilder {
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
        pub fn start_time(mut self) -> String {
            self.path.push(TimeRangeFilter::START_TIME_FIELD.name);
            self.finish()
        }
        pub fn end_time(mut self) -> String {
            self.path.push(TimeRangeFilter::END_TIME_FIELD.name);
            self.finish()
        }
    }
}
pub use _field_impls::*;
