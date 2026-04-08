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
    use crate::v1::types::Address;
    #[allow(unused_imports)]
    use crate::v1::types::AddressFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectId;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectIdFieldPathBuilder;
    impl Event {
        pub const BCS_FIELD: &'static MessageField = &MessageField {
            name: "bcs",
            json_name: "bcs",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
        pub const PACKAGE_ID_FIELD: &'static MessageField = &MessageField {
            name: "package_id",
            json_name: "packageId",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectId::FIELDS),
        };
        pub const MODULE_FIELD: &'static MessageField = &MessageField {
            name: "module",
            json_name: "module",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const SENDER_FIELD: &'static MessageField = &MessageField {
            name: "sender",
            json_name: "sender",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Address::FIELDS),
        };
        pub const EVENT_TYPE_FIELD: &'static MessageField = &MessageField {
            name: "event_type",
            json_name: "eventType",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const BCS_CONTENTS_FIELD: &'static MessageField = &MessageField {
            name: "bcs_contents",
            json_name: "bcsContents",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(BcsData::FIELDS),
        };
        pub const JSON_CONTENTS_FIELD: &'static MessageField = &MessageField {
            name: "json_contents",
            json_name: "jsonContents",
            number: 7i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for Event {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::BCS_FIELD,
            Self::PACKAGE_ID_FIELD,
            Self::MODULE_FIELD,
            Self::SENDER_FIELD,
            Self::EVENT_TYPE_FIELD,
            Self::BCS_CONTENTS_FIELD,
            Self::JSON_CONTENTS_FIELD,
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
        pub fn bcs(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(Event::BCS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
        pub fn package_id(mut self) -> ObjectIdFieldPathBuilder {
            self.path.push(Event::PACKAGE_ID_FIELD.name);
            ObjectIdFieldPathBuilder::new_with_base(self.path)
        }
        pub fn module(mut self) -> String {
            self.path.push(Event::MODULE_FIELD.name);
            self.finish()
        }
        pub fn sender(mut self) -> AddressFieldPathBuilder {
            self.path.push(Event::SENDER_FIELD.name);
            AddressFieldPathBuilder::new_with_base(self.path)
        }
        pub fn event_type(mut self) -> String {
            self.path.push(Event::EVENT_TYPE_FIELD.name);
            self.finish()
        }
        pub fn bcs_contents(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(Event::BCS_CONTENTS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
        pub fn json_contents(mut self) -> String {
            self.path.push(Event::JSON_CONTENTS_FIELD.name);
            self.finish()
        }
    }
    impl Events {
        pub const EVENTS_FIELD: &'static MessageField = &MessageField {
            name: "events",
            json_name: "events",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Event::FIELDS),
        };
    }
    impl MessageFields for Events {
        const FIELDS: &'static [&'static MessageField] = &[Self::EVENTS_FIELD];
    }
    impl Events {
        pub fn path_builder() -> EventsFieldPathBuilder {
            EventsFieldPathBuilder::new()
        }
    }
    pub struct EventsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl EventsFieldPathBuilder {
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
        pub fn events(mut self) -> EventFieldPathBuilder {
            self.path.push(Events::EVENTS_FIELD.name);
            EventFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
