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
    use crate::v1::event::Event;
    #[allow(unused_imports)]
    use crate::v1::event::EventFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::object::Object;
    #[allow(unused_imports)]
    use crate::v1::object::ObjectFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::signatures::UserSignature;
    #[allow(unused_imports)]
    use crate::v1::signatures::UserSignatureFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::Digest;
    #[allow(unused_imports)]
    use crate::v1::types::DigestFieldPathBuilder;
    impl Transaction {
        pub const DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "digest",
            json_name: "digest",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Digest::FIELDS),
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
    impl MessageFields for Transaction {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::DIGEST_FIELD,
            Self::BCS_FIELD,
        ];
    }
    impl Transaction {
        pub fn path_builder() -> TransactionFieldPathBuilder {
            TransactionFieldPathBuilder::new()
        }
    }
    pub struct TransactionFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TransactionFieldPathBuilder {
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
        pub fn digest(mut self) -> DigestFieldPathBuilder {
            self.path.push(Transaction::DIGEST_FIELD.name);
            DigestFieldPathBuilder::new_with_base(self.path)
        }
        pub fn bcs(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(Transaction::BCS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl TransactionEffects {
        pub const DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "digest",
            json_name: "digest",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Digest::FIELDS),
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
    impl MessageFields for TransactionEffects {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::DIGEST_FIELD,
            Self::BCS_FIELD,
        ];
    }
    impl TransactionEffects {
        pub fn path_builder() -> TransactionEffectsFieldPathBuilder {
            TransactionEffectsFieldPathBuilder::new()
        }
    }
    pub struct TransactionEffectsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TransactionEffectsFieldPathBuilder {
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
        pub fn digest(mut self) -> DigestFieldPathBuilder {
            self.path.push(TransactionEffects::DIGEST_FIELD.name);
            DigestFieldPathBuilder::new_with_base(self.path)
        }
        pub fn bcs(mut self) -> BcsDataFieldPathBuilder {
            self.path.push(TransactionEffects::BCS_FIELD.name);
            BcsDataFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl TransactionEvents {
        pub const DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "digest",
            json_name: "digest",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Digest::FIELDS),
        };
        pub const EVENTS_FIELD: &'static MessageField = &MessageField {
            name: "events",
            json_name: "events",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Event::FIELDS),
        };
    }
    impl MessageFields for TransactionEvents {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::DIGEST_FIELD,
            Self::EVENTS_FIELD,
        ];
    }
    impl TransactionEvents {
        pub fn path_builder() -> TransactionEventsFieldPathBuilder {
            TransactionEventsFieldPathBuilder::new()
        }
    }
    pub struct TransactionEventsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TransactionEventsFieldPathBuilder {
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
        pub fn digest(mut self) -> DigestFieldPathBuilder {
            self.path.push(TransactionEvents::DIGEST_FIELD.name);
            DigestFieldPathBuilder::new_with_base(self.path)
        }
        pub fn events(mut self) -> EventFieldPathBuilder {
            self.path.push(TransactionEvents::EVENTS_FIELD.name);
            EventFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl ExecutedTransaction {
        pub const TRANSACTION_FIELD: &'static MessageField = &MessageField {
            name: "transaction",
            json_name: "transaction",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Transaction::FIELDS),
        };
        pub const SIGNATURES_FIELD: &'static MessageField = &MessageField {
            name: "signatures",
            json_name: "signatures",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(UserSignature::FIELDS),
        };
        pub const EFFECTS_FIELD: &'static MessageField = &MessageField {
            name: "effects",
            json_name: "effects",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(TransactionEffects::FIELDS),
        };
        pub const EVENTS_FIELD: &'static MessageField = &MessageField {
            name: "events",
            json_name: "events",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(TransactionEvents::FIELDS),
        };
        pub const CHECKPOINT_FIELD: &'static MessageField = &MessageField {
            name: "checkpoint",
            json_name: "checkpoint",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const TIMESTAMP_FIELD: &'static MessageField = &MessageField {
            name: "timestamp",
            json_name: "timestamp",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const INPUT_OBJECTS_FIELD: &'static MessageField = &MessageField {
            name: "input_objects",
            json_name: "inputObjects",
            number: 7i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Object::FIELDS),
        };
        pub const OUTPUT_OBJECTS_FIELD: &'static MessageField = &MessageField {
            name: "output_objects",
            json_name: "outputObjects",
            number: 8i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Object::FIELDS),
        };
    }
    impl MessageFields for ExecutedTransaction {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::TRANSACTION_FIELD,
            Self::SIGNATURES_FIELD,
            Self::EFFECTS_FIELD,
            Self::EVENTS_FIELD,
            Self::CHECKPOINT_FIELD,
            Self::TIMESTAMP_FIELD,
            Self::INPUT_OBJECTS_FIELD,
            Self::OUTPUT_OBJECTS_FIELD,
        ];
    }
    impl ExecutedTransaction {
        pub fn path_builder() -> ExecutedTransactionFieldPathBuilder {
            ExecutedTransactionFieldPathBuilder::new()
        }
    }
    pub struct ExecutedTransactionFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ExecutedTransactionFieldPathBuilder {
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
        pub fn transaction(mut self) -> TransactionFieldPathBuilder {
            self.path.push(ExecutedTransaction::TRANSACTION_FIELD.name);
            TransactionFieldPathBuilder::new_with_base(self.path)
        }
        pub fn signatures(mut self) -> UserSignatureFieldPathBuilder {
            self.path.push(ExecutedTransaction::SIGNATURES_FIELD.name);
            UserSignatureFieldPathBuilder::new_with_base(self.path)
        }
        pub fn effects(mut self) -> TransactionEffectsFieldPathBuilder {
            self.path.push(ExecutedTransaction::EFFECTS_FIELD.name);
            TransactionEffectsFieldPathBuilder::new_with_base(self.path)
        }
        pub fn events(mut self) -> TransactionEventsFieldPathBuilder {
            self.path.push(ExecutedTransaction::EVENTS_FIELD.name);
            TransactionEventsFieldPathBuilder::new_with_base(self.path)
        }
        pub fn checkpoint(mut self) -> String {
            self.path.push(ExecutedTransaction::CHECKPOINT_FIELD.name);
            self.finish()
        }
        pub fn timestamp(mut self) -> String {
            self.path.push(ExecutedTransaction::TIMESTAMP_FIELD.name);
            self.finish()
        }
        pub fn input_objects(mut self) -> ObjectFieldPathBuilder {
            self.path.push(ExecutedTransaction::INPUT_OBJECTS_FIELD.name);
            ObjectFieldPathBuilder::new_with_base(self.path)
        }
        pub fn output_objects(mut self) -> ObjectFieldPathBuilder {
            self.path.push(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name);
            ObjectFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl ExecutedTransactions {
        pub const EXECUTED_TRANSACTIONS_FIELD: &'static MessageField = &MessageField {
            name: "executed_transactions",
            json_name: "executedTransactions",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ExecutedTransaction::FIELDS),
        };
    }
    impl MessageFields for ExecutedTransactions {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::EXECUTED_TRANSACTIONS_FIELD,
        ];
    }
    impl ExecutedTransactions {
        pub fn path_builder() -> ExecutedTransactionsFieldPathBuilder {
            ExecutedTransactionsFieldPathBuilder::new()
        }
    }
    pub struct ExecutedTransactionsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ExecutedTransactionsFieldPathBuilder {
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
        pub fn executed_transactions(mut self) -> ExecutedTransactionFieldPathBuilder {
            self.path.push(ExecutedTransactions::EXECUTED_TRANSACTIONS_FIELD.name);
            ExecutedTransactionFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
