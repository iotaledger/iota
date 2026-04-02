// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _field_impls {
    #![allow(clippy::wrong_self_convention)]
    use super::*;
    use crate::field::MessageFields;
    use crate::field::MessageField;
    #[allow(unused_imports)]
    use crate::v1::checkpoint::Checkpoint;
    #[allow(unused_imports)]
    use crate::v1::checkpoint::CheckpointFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::epoch::Epoch;
    #[allow(unused_imports)]
    use crate::v1::epoch::EpochFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::event::Event;
    #[allow(unused_imports)]
    use crate::v1::event::EventFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::filter::EventFilter;
    #[allow(unused_imports)]
    use crate::v1::filter::EventFilterFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::filter::TransactionFilter;
    #[allow(unused_imports)]
    use crate::v1::filter::TransactionFilterFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::object::Object;
    #[allow(unused_imports)]
    use crate::v1::object::ObjectFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::transaction::ExecutedTransaction;
    #[allow(unused_imports)]
    use crate::v1::transaction::ExecutedTransactionFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::transaction::ExecutedTransactions;
    #[allow(unused_imports)]
    use crate::v1::transaction::ExecutedTransactionsFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::Digest;
    #[allow(unused_imports)]
    use crate::v1::types::DigestFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectReference;
    #[allow(unused_imports)]
    use crate::v1::types::ObjectReferenceFieldPathBuilder;
    #[allow(unused_imports)]
    use crate::v1::ledger_service::checkpoint_data::Progress;
    #[allow(unused_imports)]
    use crate::v1::ledger_service::checkpoint_data::EndMarker;
    pub mod checkpoint_data {
        use super::*;
        impl Progress {
            pub const LATEST_SCANNED_SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
                name: "latest_scanned_sequence_number",
                json_name: "latestScannedSequenceNumber",
                number: 1i32,
                is_optional: false,
                is_map: false,
                message_fields: None,
            };
        }
        impl MessageFields for Progress {
            const FIELDS: &'static [&'static MessageField] = &[
                Self::LATEST_SCANNED_SEQUENCE_NUMBER_FIELD,
            ];
        }
        impl Progress {
            pub fn path_builder() -> ProgressFieldPathBuilder {
                ProgressFieldPathBuilder::new()
            }
        }
        pub struct ProgressFieldPathBuilder {
            path: Vec<&'static str>,
        }
        impl ProgressFieldPathBuilder {
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
            pub fn latest_scanned_sequence_number(mut self) -> String {
                self.path.push(Progress::LATEST_SCANNED_SEQUENCE_NUMBER_FIELD.name);
                self.finish()
            }
        }
        impl EndMarker {
            pub const SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
                name: "sequence_number",
                json_name: "sequenceNumber",
                number: 1i32,
                is_optional: true,
                is_map: false,
                message_fields: None,
            };
        }
        impl MessageFields for EndMarker {
            const FIELDS: &'static [&'static MessageField] = &[
                Self::SEQUENCE_NUMBER_FIELD,
            ];
        }
        impl EndMarker {
            pub fn path_builder() -> EndMarkerFieldPathBuilder {
                EndMarkerFieldPathBuilder::new()
            }
        }
        pub struct EndMarkerFieldPathBuilder {
            path: Vec<&'static str>,
        }
        impl EndMarkerFieldPathBuilder {
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
            pub fn sequence_number(mut self) -> String {
                self.path.push(EndMarker::SEQUENCE_NUMBER_FIELD.name);
                self.finish()
            }
        }
    }
    impl GetHealthRequest {
        pub const THRESHOLD_MS_FIELD: &'static MessageField = &MessageField {
            name: "threshold_ms",
            json_name: "thresholdMs",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for GetHealthRequest {
        const FIELDS: &'static [&'static MessageField] = &[Self::THRESHOLD_MS_FIELD];
    }
    impl GetHealthRequest {
        pub fn path_builder() -> GetHealthRequestFieldPathBuilder {
            GetHealthRequestFieldPathBuilder::new()
        }
    }
    pub struct GetHealthRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetHealthRequestFieldPathBuilder {
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
        pub fn threshold_ms(mut self) -> String {
            self.path.push(GetHealthRequest::THRESHOLD_MS_FIELD.name);
            self.finish()
        }
    }
    impl GetHealthResponse {
        pub const EXECUTED_CHECKPOINT_HEIGHT_FIELD: &'static MessageField = &MessageField {
            name: "executed_checkpoint_height",
            json_name: "executedCheckpointHeight",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const ESTIMATED_VALIDATOR_LATENCY_MS_FIELD: &'static MessageField = &MessageField {
            name: "estimated_validator_latency_ms",
            json_name: "estimatedValidatorLatencyMs",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for GetHealthResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::EXECUTED_CHECKPOINT_HEIGHT_FIELD,
            Self::ESTIMATED_VALIDATOR_LATENCY_MS_FIELD,
        ];
    }
    impl GetHealthResponse {
        pub fn path_builder() -> GetHealthResponseFieldPathBuilder {
            GetHealthResponseFieldPathBuilder::new()
        }
    }
    pub struct GetHealthResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetHealthResponseFieldPathBuilder {
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
        pub fn executed_checkpoint_height(mut self) -> String {
            self.path.push(GetHealthResponse::EXECUTED_CHECKPOINT_HEIGHT_FIELD.name);
            self.finish()
        }
        pub fn estimated_validator_latency_ms(mut self) -> String {
            self.path.push(GetHealthResponse::ESTIMATED_VALIDATOR_LATENCY_MS_FIELD.name);
            self.finish()
        }
    }
    impl GetServiceInfoRequest {
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for GetServiceInfoRequest {
        const FIELDS: &'static [&'static MessageField] = &[Self::READ_MASK_FIELD];
    }
    impl GetServiceInfoRequest {
        pub fn path_builder() -> GetServiceInfoRequestFieldPathBuilder {
            GetServiceInfoRequestFieldPathBuilder::new()
        }
    }
    pub struct GetServiceInfoRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetServiceInfoRequestFieldPathBuilder {
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
        pub fn read_mask(mut self) -> String {
            self.path.push(GetServiceInfoRequest::READ_MASK_FIELD.name);
            self.finish()
        }
    }
    impl GetServiceInfoResponse {
        pub const CHAIN_ID_FIELD: &'static MessageField = &MessageField {
            name: "chain_id",
            json_name: "chainId",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Digest::FIELDS),
        };
        pub const CHAIN_FIELD: &'static MessageField = &MessageField {
            name: "chain",
            json_name: "chain",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const EPOCH_FIELD: &'static MessageField = &MessageField {
            name: "epoch",
            json_name: "epoch",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const EXECUTED_CHECKPOINT_HEIGHT_FIELD: &'static MessageField = &MessageField {
            name: "executed_checkpoint_height",
            json_name: "executedCheckpointHeight",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const EXECUTED_CHECKPOINT_TIMESTAMP_FIELD: &'static MessageField = &MessageField {
            name: "executed_checkpoint_timestamp",
            json_name: "executedCheckpointTimestamp",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const LOWEST_AVAILABLE_CHECKPOINT_FIELD: &'static MessageField = &MessageField {
            name: "lowest_available_checkpoint",
            json_name: "lowestAvailableCheckpoint",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const LOWEST_AVAILABLE_CHECKPOINT_OBJECTS_FIELD: &'static MessageField = &MessageField {
            name: "lowest_available_checkpoint_objects",
            json_name: "lowestAvailableCheckpointObjects",
            number: 7i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const SERVER_FIELD: &'static MessageField = &MessageField {
            name: "server",
            json_name: "server",
            number: 8i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for GetServiceInfoResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::CHAIN_ID_FIELD,
            Self::CHAIN_FIELD,
            Self::EPOCH_FIELD,
            Self::EXECUTED_CHECKPOINT_HEIGHT_FIELD,
            Self::EXECUTED_CHECKPOINT_TIMESTAMP_FIELD,
            Self::LOWEST_AVAILABLE_CHECKPOINT_FIELD,
            Self::LOWEST_AVAILABLE_CHECKPOINT_OBJECTS_FIELD,
            Self::SERVER_FIELD,
        ];
    }
    impl GetServiceInfoResponse {
        pub fn path_builder() -> GetServiceInfoResponseFieldPathBuilder {
            GetServiceInfoResponseFieldPathBuilder::new()
        }
    }
    pub struct GetServiceInfoResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetServiceInfoResponseFieldPathBuilder {
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
        pub fn chain_id(mut self) -> DigestFieldPathBuilder {
            self.path.push(GetServiceInfoResponse::CHAIN_ID_FIELD.name);
            DigestFieldPathBuilder::new_with_base(self.path)
        }
        pub fn chain(mut self) -> String {
            self.path.push(GetServiceInfoResponse::CHAIN_FIELD.name);
            self.finish()
        }
        pub fn epoch(mut self) -> String {
            self.path.push(GetServiceInfoResponse::EPOCH_FIELD.name);
            self.finish()
        }
        pub fn executed_checkpoint_height(mut self) -> String {
            self.path
                .push(GetServiceInfoResponse::EXECUTED_CHECKPOINT_HEIGHT_FIELD.name);
            self.finish()
        }
        pub fn executed_checkpoint_timestamp(mut self) -> String {
            self.path
                .push(GetServiceInfoResponse::EXECUTED_CHECKPOINT_TIMESTAMP_FIELD.name);
            self.finish()
        }
        pub fn lowest_available_checkpoint(mut self) -> String {
            self.path
                .push(GetServiceInfoResponse::LOWEST_AVAILABLE_CHECKPOINT_FIELD.name);
            self.finish()
        }
        pub fn lowest_available_checkpoint_objects(mut self) -> String {
            self.path
                .push(
                    GetServiceInfoResponse::LOWEST_AVAILABLE_CHECKPOINT_OBJECTS_FIELD
                        .name,
                );
            self.finish()
        }
        pub fn server(mut self) -> String {
            self.path.push(GetServiceInfoResponse::SERVER_FIELD.name);
            self.finish()
        }
    }
    impl ObjectRequest {
        pub const OBJECT_REF_FIELD: &'static MessageField = &MessageField {
            name: "object_ref",
            json_name: "objectRef",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectReference::FIELDS),
        };
    }
    impl MessageFields for ObjectRequest {
        const FIELDS: &'static [&'static MessageField] = &[Self::OBJECT_REF_FIELD];
    }
    impl ObjectRequest {
        pub fn path_builder() -> ObjectRequestFieldPathBuilder {
            ObjectRequestFieldPathBuilder::new()
        }
    }
    pub struct ObjectRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ObjectRequestFieldPathBuilder {
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
            self.path.push(ObjectRequest::OBJECT_REF_FIELD.name);
            ObjectReferenceFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl ObjectRequests {
        pub const REQUESTS_FIELD: &'static MessageField = &MessageField {
            name: "requests",
            json_name: "requests",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ObjectRequest::FIELDS),
        };
    }
    impl MessageFields for ObjectRequests {
        const FIELDS: &'static [&'static MessageField] = &[Self::REQUESTS_FIELD];
    }
    impl ObjectRequests {
        pub fn path_builder() -> ObjectRequestsFieldPathBuilder {
            ObjectRequestsFieldPathBuilder::new()
        }
    }
    pub struct ObjectRequestsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ObjectRequestsFieldPathBuilder {
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
        pub fn requests(mut self) -> ObjectRequestFieldPathBuilder {
            self.path.push(ObjectRequests::REQUESTS_FIELD.name);
            ObjectRequestFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl GetObjectsRequest {
        pub const REQUESTS_FIELD: &'static MessageField = &MessageField {
            name: "requests",
            json_name: "requests",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(ObjectRequests::FIELDS),
        };
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
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
    impl MessageFields for GetObjectsRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::REQUESTS_FIELD,
            Self::READ_MASK_FIELD,
            Self::MAX_MESSAGE_SIZE_BYTES_FIELD,
        ];
    }
    impl GetObjectsRequest {
        pub fn path_builder() -> GetObjectsRequestFieldPathBuilder {
            GetObjectsRequestFieldPathBuilder::new()
        }
    }
    pub struct GetObjectsRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetObjectsRequestFieldPathBuilder {
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
        pub fn requests(mut self) -> ObjectRequestsFieldPathBuilder {
            self.path.push(GetObjectsRequest::REQUESTS_FIELD.name);
            ObjectRequestsFieldPathBuilder::new_with_base(self.path)
        }
        pub fn read_mask(mut self) -> String {
            self.path.push(GetObjectsRequest::READ_MASK_FIELD.name);
            self.finish()
        }
        pub fn max_message_size_bytes(mut self) -> String {
            self.path.push(GetObjectsRequest::MAX_MESSAGE_SIZE_BYTES_FIELD.name);
            self.finish()
        }
    }
    impl ObjectResult {
        pub const OBJECT_FIELD: &'static MessageField = &MessageField {
            name: "object",
            json_name: "object",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Object::FIELDS),
        };
        pub const ERROR_FIELD: &'static MessageField = &MessageField {
            name: "error",
            json_name: "error",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl ObjectResult {
        pub const RESULT_ONEOF: &'static str = "result";
    }
    impl MessageFields for ObjectResult {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::OBJECT_FIELD,
            Self::ERROR_FIELD,
        ];
        const ONEOFS: &'static [&'static str] = &["result"];
    }
    impl ObjectResult {
        pub fn path_builder() -> ObjectResultFieldPathBuilder {
            ObjectResultFieldPathBuilder::new()
        }
    }
    pub struct ObjectResultFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl ObjectResultFieldPathBuilder {
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
        pub fn object(mut self) -> ObjectFieldPathBuilder {
            self.path.push(ObjectResult::OBJECT_FIELD.name);
            ObjectFieldPathBuilder::new_with_base(self.path)
        }
        pub fn error(mut self) -> String {
            self.path.push(ObjectResult::ERROR_FIELD.name);
            self.finish()
        }
    }
    impl GetObjectsResponse {
        pub const OBJECTS_FIELD: &'static MessageField = &MessageField {
            name: "objects",
            json_name: "objects",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ObjectResult::FIELDS),
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
    impl MessageFields for GetObjectsResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::OBJECTS_FIELD,
            Self::HAS_NEXT_FIELD,
        ];
    }
    impl GetObjectsResponse {
        pub fn path_builder() -> GetObjectsResponseFieldPathBuilder {
            GetObjectsResponseFieldPathBuilder::new()
        }
    }
    pub struct GetObjectsResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetObjectsResponseFieldPathBuilder {
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
        pub fn objects(mut self) -> ObjectResultFieldPathBuilder {
            self.path.push(GetObjectsResponse::OBJECTS_FIELD.name);
            ObjectResultFieldPathBuilder::new_with_base(self.path)
        }
        pub fn has_next(mut self) -> String {
            self.path.push(GetObjectsResponse::HAS_NEXT_FIELD.name);
            self.finish()
        }
    }
    impl TransactionRequest {
        pub const DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "digest",
            json_name: "digest",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Digest::FIELDS),
        };
    }
    impl MessageFields for TransactionRequest {
        const FIELDS: &'static [&'static MessageField] = &[Self::DIGEST_FIELD];
    }
    impl TransactionRequest {
        pub fn path_builder() -> TransactionRequestFieldPathBuilder {
            TransactionRequestFieldPathBuilder::new()
        }
    }
    pub struct TransactionRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TransactionRequestFieldPathBuilder {
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
            self.path.push(TransactionRequest::DIGEST_FIELD.name);
            DigestFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl TransactionRequests {
        pub const REQUESTS_FIELD: &'static MessageField = &MessageField {
            name: "requests",
            json_name: "requests",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(TransactionRequest::FIELDS),
        };
    }
    impl MessageFields for TransactionRequests {
        const FIELDS: &'static [&'static MessageField] = &[Self::REQUESTS_FIELD];
    }
    impl TransactionRequests {
        pub fn path_builder() -> TransactionRequestsFieldPathBuilder {
            TransactionRequestsFieldPathBuilder::new()
        }
    }
    pub struct TransactionRequestsFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TransactionRequestsFieldPathBuilder {
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
        pub fn requests(mut self) -> TransactionRequestFieldPathBuilder {
            self.path.push(TransactionRequests::REQUESTS_FIELD.name);
            TransactionRequestFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl GetTransactionsRequest {
        pub const REQUESTS_FIELD: &'static MessageField = &MessageField {
            name: "requests",
            json_name: "requests",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(TransactionRequests::FIELDS),
        };
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
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
    impl MessageFields for GetTransactionsRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::REQUESTS_FIELD,
            Self::READ_MASK_FIELD,
            Self::MAX_MESSAGE_SIZE_BYTES_FIELD,
        ];
    }
    impl GetTransactionsRequest {
        pub fn path_builder() -> GetTransactionsRequestFieldPathBuilder {
            GetTransactionsRequestFieldPathBuilder::new()
        }
    }
    pub struct GetTransactionsRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetTransactionsRequestFieldPathBuilder {
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
        pub fn requests(mut self) -> TransactionRequestsFieldPathBuilder {
            self.path.push(GetTransactionsRequest::REQUESTS_FIELD.name);
            TransactionRequestsFieldPathBuilder::new_with_base(self.path)
        }
        pub fn read_mask(mut self) -> String {
            self.path.push(GetTransactionsRequest::READ_MASK_FIELD.name);
            self.finish()
        }
        pub fn max_message_size_bytes(mut self) -> String {
            self.path.push(GetTransactionsRequest::MAX_MESSAGE_SIZE_BYTES_FIELD.name);
            self.finish()
        }
    }
    impl TransactionResult {
        pub const EXECUTED_TRANSACTION_FIELD: &'static MessageField = &MessageField {
            name: "executed_transaction",
            json_name: "executedTransaction",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ExecutedTransaction::FIELDS),
        };
        pub const ERROR_FIELD: &'static MessageField = &MessageField {
            name: "error",
            json_name: "error",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
    }
    impl TransactionResult {
        pub const RESULT_ONEOF: &'static str = "result";
    }
    impl MessageFields for TransactionResult {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::EXECUTED_TRANSACTION_FIELD,
            Self::ERROR_FIELD,
        ];
        const ONEOFS: &'static [&'static str] = &["result"];
    }
    impl TransactionResult {
        pub fn path_builder() -> TransactionResultFieldPathBuilder {
            TransactionResultFieldPathBuilder::new()
        }
    }
    pub struct TransactionResultFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl TransactionResultFieldPathBuilder {
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
        pub fn executed_transaction(mut self) -> ExecutedTransactionFieldPathBuilder {
            self.path.push(TransactionResult::EXECUTED_TRANSACTION_FIELD.name);
            ExecutedTransactionFieldPathBuilder::new_with_base(self.path)
        }
        pub fn error(mut self) -> String {
            self.path.push(TransactionResult::ERROR_FIELD.name);
            self.finish()
        }
    }
    impl GetTransactionsResponse {
        pub const TRANSACTION_RESULTS_FIELD: &'static MessageField = &MessageField {
            name: "transaction_results",
            json_name: "transactionResults",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(TransactionResult::FIELDS),
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
    impl MessageFields for GetTransactionsResponse {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::TRANSACTION_RESULTS_FIELD,
            Self::HAS_NEXT_FIELD,
        ];
    }
    impl GetTransactionsResponse {
        pub fn path_builder() -> GetTransactionsResponseFieldPathBuilder {
            GetTransactionsResponseFieldPathBuilder::new()
        }
    }
    pub struct GetTransactionsResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetTransactionsResponseFieldPathBuilder {
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
        pub fn transaction_results(mut self) -> TransactionResultFieldPathBuilder {
            self.path.push(GetTransactionsResponse::TRANSACTION_RESULTS_FIELD.name);
            TransactionResultFieldPathBuilder::new_with_base(self.path)
        }
        pub fn has_next(mut self) -> String {
            self.path.push(GetTransactionsResponse::HAS_NEXT_FIELD.name);
            self.finish()
        }
    }
    impl GetCheckpointRequest {
        pub const LATEST_FIELD: &'static MessageField = &MessageField {
            name: "latest",
            json_name: "latest",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
            name: "sequence_number",
            json_name: "sequenceNumber",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: None,
        };
        pub const DIGEST_FIELD: &'static MessageField = &MessageField {
            name: "digest",
            json_name: "digest",
            number: 3i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Digest::FIELDS),
        };
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const TRANSACTIONS_FILTER_FIELD: &'static MessageField = &MessageField {
            name: "transactions_filter",
            json_name: "transactionsFilter",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(TransactionFilter::FIELDS),
        };
        pub const EVENTS_FILTER_FIELD: &'static MessageField = &MessageField {
            name: "events_filter",
            json_name: "eventsFilter",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(EventFilter::FIELDS),
        };
        pub const MAX_MESSAGE_SIZE_BYTES_FIELD: &'static MessageField = &MessageField {
            name: "max_message_size_bytes",
            json_name: "maxMessageSizeBytes",
            number: 7i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl GetCheckpointRequest {
        pub const CHECKPOINT_ID_ONEOF: &'static str = "checkpoint_id";
    }
    impl MessageFields for GetCheckpointRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::LATEST_FIELD,
            Self::SEQUENCE_NUMBER_FIELD,
            Self::DIGEST_FIELD,
            Self::READ_MASK_FIELD,
            Self::TRANSACTIONS_FILTER_FIELD,
            Self::EVENTS_FILTER_FIELD,
            Self::MAX_MESSAGE_SIZE_BYTES_FIELD,
        ];
        const ONEOFS: &'static [&'static str] = &["checkpoint_id"];
    }
    impl GetCheckpointRequest {
        pub fn path_builder() -> GetCheckpointRequestFieldPathBuilder {
            GetCheckpointRequestFieldPathBuilder::new()
        }
    }
    pub struct GetCheckpointRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetCheckpointRequestFieldPathBuilder {
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
        pub fn latest(mut self) -> String {
            self.path.push(GetCheckpointRequest::LATEST_FIELD.name);
            self.finish()
        }
        pub fn sequence_number(mut self) -> String {
            self.path.push(GetCheckpointRequest::SEQUENCE_NUMBER_FIELD.name);
            self.finish()
        }
        pub fn digest(mut self) -> DigestFieldPathBuilder {
            self.path.push(GetCheckpointRequest::DIGEST_FIELD.name);
            DigestFieldPathBuilder::new_with_base(self.path)
        }
        pub fn read_mask(mut self) -> String {
            self.path.push(GetCheckpointRequest::READ_MASK_FIELD.name);
            self.finish()
        }
        pub fn transactions_filter(mut self) -> TransactionFilterFieldPathBuilder {
            self.path.push(GetCheckpointRequest::TRANSACTIONS_FILTER_FIELD.name);
            TransactionFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn events_filter(mut self) -> EventFilterFieldPathBuilder {
            self.path.push(GetCheckpointRequest::EVENTS_FILTER_FIELD.name);
            EventFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn max_message_size_bytes(mut self) -> String {
            self.path.push(GetCheckpointRequest::MAX_MESSAGE_SIZE_BYTES_FIELD.name);
            self.finish()
        }
    }
    impl StreamCheckpointsRequest {
        pub const START_SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
            name: "start_sequence_number",
            json_name: "startSequenceNumber",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const END_SEQUENCE_NUMBER_FIELD: &'static MessageField = &MessageField {
            name: "end_sequence_number",
            json_name: "endSequenceNumber",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
            number: 3i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const TRANSACTIONS_FILTER_FIELD: &'static MessageField = &MessageField {
            name: "transactions_filter",
            json_name: "transactionsFilter",
            number: 4i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(TransactionFilter::FIELDS),
        };
        pub const EVENTS_FILTER_FIELD: &'static MessageField = &MessageField {
            name: "events_filter",
            json_name: "eventsFilter",
            number: 5i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(EventFilter::FIELDS),
        };
        pub const FILTER_CHECKPOINTS_FIELD: &'static MessageField = &MessageField {
            name: "filter_checkpoints",
            json_name: "filterCheckpoints",
            number: 6i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const PROGRESS_INTERVAL_MS_FIELD: &'static MessageField = &MessageField {
            name: "progress_interval_ms",
            json_name: "progressIntervalMs",
            number: 7i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const MAX_MESSAGE_SIZE_BYTES_FIELD: &'static MessageField = &MessageField {
            name: "max_message_size_bytes",
            json_name: "maxMessageSizeBytes",
            number: 8i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for StreamCheckpointsRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::START_SEQUENCE_NUMBER_FIELD,
            Self::END_SEQUENCE_NUMBER_FIELD,
            Self::READ_MASK_FIELD,
            Self::TRANSACTIONS_FILTER_FIELD,
            Self::EVENTS_FILTER_FIELD,
            Self::FILTER_CHECKPOINTS_FIELD,
            Self::PROGRESS_INTERVAL_MS_FIELD,
            Self::MAX_MESSAGE_SIZE_BYTES_FIELD,
        ];
    }
    impl StreamCheckpointsRequest {
        pub fn path_builder() -> StreamCheckpointsRequestFieldPathBuilder {
            StreamCheckpointsRequestFieldPathBuilder::new()
        }
    }
    pub struct StreamCheckpointsRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl StreamCheckpointsRequestFieldPathBuilder {
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
        pub fn start_sequence_number(mut self) -> String {
            self.path.push(StreamCheckpointsRequest::START_SEQUENCE_NUMBER_FIELD.name);
            self.finish()
        }
        pub fn end_sequence_number(mut self) -> String {
            self.path.push(StreamCheckpointsRequest::END_SEQUENCE_NUMBER_FIELD.name);
            self.finish()
        }
        pub fn read_mask(mut self) -> String {
            self.path.push(StreamCheckpointsRequest::READ_MASK_FIELD.name);
            self.finish()
        }
        pub fn transactions_filter(mut self) -> TransactionFilterFieldPathBuilder {
            self.path.push(StreamCheckpointsRequest::TRANSACTIONS_FILTER_FIELD.name);
            TransactionFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn events_filter(mut self) -> EventFilterFieldPathBuilder {
            self.path.push(StreamCheckpointsRequest::EVENTS_FILTER_FIELD.name);
            EventFilterFieldPathBuilder::new_with_base(self.path)
        }
        pub fn filter_checkpoints(mut self) -> String {
            self.path.push(StreamCheckpointsRequest::FILTER_CHECKPOINTS_FIELD.name);
            self.finish()
        }
        pub fn progress_interval_ms(mut self) -> String {
            self.path.push(StreamCheckpointsRequest::PROGRESS_INTERVAL_MS_FIELD.name);
            self.finish()
        }
        pub fn max_message_size_bytes(mut self) -> String {
            self.path.push(StreamCheckpointsRequest::MAX_MESSAGE_SIZE_BYTES_FIELD.name);
            self.finish()
        }
    }
    impl CheckpointData {
        pub const CHECKPOINT_FIELD: &'static MessageField = &MessageField {
            name: "checkpoint",
            json_name: "checkpoint",
            number: 1i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Checkpoint::FIELDS),
        };
        pub const EXECUTED_TRANSACTIONS_FIELD: &'static MessageField = &MessageField {
            name: "executed_transactions",
            json_name: "executedTransactions",
            number: 2i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(ExecutedTransactions::FIELDS),
        };
        pub const EVENTS_FIELD: &'static MessageField = &MessageField {
            name: "events",
            json_name: "events",
            number: 3i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Event::FIELDS),
        };
        pub const PROGRESS_FIELD: &'static MessageField = &MessageField {
            name: "progress",
            json_name: "progress",
            number: 4i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(Progress::FIELDS),
        };
        pub const END_MARKER_FIELD: &'static MessageField = &MessageField {
            name: "end_marker",
            json_name: "endMarker",
            number: 5i32,
            is_optional: false,
            is_map: false,
            message_fields: Some(EndMarker::FIELDS),
        };
    }
    impl CheckpointData {
        pub const PAYLOAD_ONEOF: &'static str = "payload";
    }
    impl MessageFields for CheckpointData {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::CHECKPOINT_FIELD,
            Self::EXECUTED_TRANSACTIONS_FIELD,
            Self::EVENTS_FIELD,
            Self::PROGRESS_FIELD,
            Self::END_MARKER_FIELD,
        ];
        const ONEOFS: &'static [&'static str] = &["payload"];
    }
    impl CheckpointData {
        pub fn path_builder() -> CheckpointDataFieldPathBuilder {
            CheckpointDataFieldPathBuilder::new()
        }
    }
    pub struct CheckpointDataFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl CheckpointDataFieldPathBuilder {
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
        pub fn checkpoint(mut self) -> CheckpointFieldPathBuilder {
            self.path.push(CheckpointData::CHECKPOINT_FIELD.name);
            CheckpointFieldPathBuilder::new_with_base(self.path)
        }
        pub fn executed_transactions(mut self) -> ExecutedTransactionsFieldPathBuilder {
            self.path.push(CheckpointData::EXECUTED_TRANSACTIONS_FIELD.name);
            ExecutedTransactionsFieldPathBuilder::new_with_base(self.path)
        }
        pub fn events(mut self) -> EventFieldPathBuilder {
            self.path.push(CheckpointData::EVENTS_FIELD.name);
            EventFieldPathBuilder::new_with_base(self.path)
        }
        pub fn progress(mut self) -> checkpoint_data::ProgressFieldPathBuilder {
            self.path.push(CheckpointData::PROGRESS_FIELD.name);
            checkpoint_data::ProgressFieldPathBuilder::new_with_base(self.path)
        }
        pub fn end_marker(mut self) -> checkpoint_data::EndMarkerFieldPathBuilder {
            self.path.push(CheckpointData::END_MARKER_FIELD.name);
            checkpoint_data::EndMarkerFieldPathBuilder::new_with_base(self.path)
        }
    }
    impl GetEpochRequest {
        pub const EPOCH_FIELD: &'static MessageField = &MessageField {
            name: "epoch",
            json_name: "epoch",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
        pub const READ_MASK_FIELD: &'static MessageField = &MessageField {
            name: "read_mask",
            json_name: "readMask",
            number: 2i32,
            is_optional: true,
            is_map: false,
            message_fields: None,
        };
    }
    impl MessageFields for GetEpochRequest {
        const FIELDS: &'static [&'static MessageField] = &[
            Self::EPOCH_FIELD,
            Self::READ_MASK_FIELD,
        ];
    }
    impl GetEpochRequest {
        pub fn path_builder() -> GetEpochRequestFieldPathBuilder {
            GetEpochRequestFieldPathBuilder::new()
        }
    }
    pub struct GetEpochRequestFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetEpochRequestFieldPathBuilder {
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
        pub fn epoch(mut self) -> String {
            self.path.push(GetEpochRequest::EPOCH_FIELD.name);
            self.finish()
        }
        pub fn read_mask(mut self) -> String {
            self.path.push(GetEpochRequest::READ_MASK_FIELD.name);
            self.finish()
        }
    }
    impl GetEpochResponse {
        pub const EPOCH_FIELD: &'static MessageField = &MessageField {
            name: "epoch",
            json_name: "epoch",
            number: 1i32,
            is_optional: true,
            is_map: false,
            message_fields: Some(Epoch::FIELDS),
        };
    }
    impl MessageFields for GetEpochResponse {
        const FIELDS: &'static [&'static MessageField] = &[Self::EPOCH_FIELD];
    }
    impl GetEpochResponse {
        pub fn path_builder() -> GetEpochResponseFieldPathBuilder {
            GetEpochResponseFieldPathBuilder::new()
        }
    }
    pub struct GetEpochResponseFieldPathBuilder {
        path: Vec<&'static str>,
    }
    impl GetEpochResponseFieldPathBuilder {
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
        pub fn epoch(mut self) -> EpochFieldPathBuilder {
            self.path.push(GetEpochResponse::EPOCH_FIELD.name);
            EpochFieldPathBuilder::new_with_base(self.path)
        }
    }
}
pub use _field_impls::*;
