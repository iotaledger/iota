// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::CheckpointData {
        /// Sets `checkpoint` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_checkpoint<T: Into<super::super::checkpoint::Checkpoint>>(
            mut self,
            field: T,
        ) -> Self {
            self.payload = Some(
                super::checkpoint_data::Payload::Checkpoint(field.into()),
            );
            self
        }
        /// Sets `executed_transactions` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_executed_transactions<
            T: Into<super::super::transaction::ExecutedTransactions>,
        >(mut self, field: T) -> Self {
            self.payload = Some(
                super::checkpoint_data::Payload::ExecutedTransactions(field.into()),
            );
            self
        }
        /// Sets `events` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_events<T: Into<super::super::event::Events>>(
            mut self,
            field: T,
        ) -> Self {
            self.payload = Some(super::checkpoint_data::Payload::Events(field.into()));
            self
        }
        /// Sets `progress` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_progress<T: Into<super::checkpoint_data::Progress>>(
            mut self,
            field: T,
        ) -> Self {
            self.payload = Some(super::checkpoint_data::Payload::Progress(field.into()));
            self
        }
        /// Sets `end_marker` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_end_marker<T: Into<super::checkpoint_data::EndMarker>>(
            mut self,
            field: T,
        ) -> Self {
            self.payload = Some(
                super::checkpoint_data::Payload::EndMarker(field.into()),
            );
            self
        }
    }
    impl super::checkpoint_data::EndMarker {
        /// Sets `sequence_number` with the provided value.
        pub fn with_sequence_number(mut self, field: u64) -> Self {
            self.sequence_number = Some(field);
            self
        }
    }
    impl super::checkpoint_data::Progress {
        /// Sets `latest_scanned_sequence_number` with the provided value.
        pub fn with_latest_scanned_sequence_number(mut self, field: u64) -> Self {
            self.latest_scanned_sequence_number = field;
            self
        }
    }
    impl super::GetCheckpointRequest {
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
        /// Sets `transactions_filter` with the provided value.
        pub fn with_transactions_filter<
            T: Into<super::super::filter::TransactionFilter>,
        >(mut self, field: T) -> Self {
            self.transactions_filter = Some(field.into());
            self
        }
        /// Sets `events_filter` with the provided value.
        pub fn with_events_filter<T: Into<super::super::filter::EventFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.events_filter = Some(field.into());
            self
        }
        /// Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.max_message_size_bytes = Some(field);
            self
        }
        /// Sets `latest` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_latest(mut self, field: bool) -> Self {
            self.checkpoint_id = Some(
                super::get_checkpoint_request::CheckpointId::Latest(field),
            );
            self
        }
        /// Sets `sequence_number` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_sequence_number(mut self, field: u64) -> Self {
            self.checkpoint_id = Some(
                super::get_checkpoint_request::CheckpointId::SequenceNumber(field),
            );
            self
        }
        /// Sets `digest` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.checkpoint_id = Some(
                super::get_checkpoint_request::CheckpointId::Digest(field.into()),
            );
            self
        }
    }
    impl super::GetEpochRequest {
        /// Sets `epoch` with the provided value.
        pub fn with_epoch(mut self, field: u64) -> Self {
            self.epoch = Some(field);
            self
        }
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
    }
    impl super::GetEpochResponse {
        /// Sets `epoch` with the provided value.
        pub fn with_epoch<T: Into<super::super::epoch::Epoch>>(
            mut self,
            field: T,
        ) -> Self {
            self.epoch = Some(field.into());
            self
        }
    }
    impl super::GetHealthRequest {
        /// Sets `threshold_ms` with the provided value.
        pub fn with_threshold_ms(mut self, field: u64) -> Self {
            self.threshold_ms = Some(field);
            self
        }
    }
    impl super::GetHealthResponse {
        /// Sets `executed_checkpoint_height` with the provided value.
        pub fn with_executed_checkpoint_height(mut self, field: u64) -> Self {
            self.executed_checkpoint_height = Some(field);
            self
        }
        /// Sets `estimated_validator_latency_ms` with the provided value.
        pub fn with_estimated_validator_latency_ms(mut self, field: u32) -> Self {
            self.estimated_validator_latency_ms = Some(field);
            self
        }
    }
    impl super::GetObjectsRequest {
        /// Sets `requests` with the provided value.
        pub fn with_requests<T: Into<super::ObjectRequests>>(
            mut self,
            field: T,
        ) -> Self {
            self.requests = Some(field.into());
            self
        }
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
        /// Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.max_message_size_bytes = Some(field);
            self
        }
    }
    impl super::GetObjectsResponse {
        /// Sets `objects` with the provided value.
        pub fn with_objects(mut self, field: Vec<super::ObjectResult>) -> Self {
            self.objects = field;
            self
        }
        /// Sets `has_next` with the provided value.
        pub fn with_has_next(mut self, field: bool) -> Self {
            self.has_next = field;
            self
        }
    }
    impl super::GetServiceInfoRequest {
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
    }
    impl super::GetServiceInfoResponse {
        /// Sets `chain_id` with the provided value.
        pub fn with_chain_id<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.chain_id = Some(field.into());
            self
        }
        /// Sets `chain` with the provided value.
        pub fn with_chain<T: Into<String>>(mut self, field: T) -> Self {
            self.chain = Some(field.into());
            self
        }
        /// Sets `epoch` with the provided value.
        pub fn with_epoch(mut self, field: u64) -> Self {
            self.epoch = Some(field);
            self
        }
        /// Sets `executed_checkpoint_height` with the provided value.
        pub fn with_executed_checkpoint_height(mut self, field: u64) -> Self {
            self.executed_checkpoint_height = Some(field);
            self
        }
        /// Sets `executed_checkpoint_timestamp` with the provided value.
        pub fn with_executed_checkpoint_timestamp<T: Into<::prost_types::Timestamp>>(
            mut self,
            field: T,
        ) -> Self {
            self.executed_checkpoint_timestamp = Some(field.into());
            self
        }
        /// Sets `lowest_available_checkpoint` with the provided value.
        pub fn with_lowest_available_checkpoint(mut self, field: u64) -> Self {
            self.lowest_available_checkpoint = Some(field);
            self
        }
        /// Sets `lowest_available_checkpoint_objects` with the provided value.
        pub fn with_lowest_available_checkpoint_objects(mut self, field: u64) -> Self {
            self.lowest_available_checkpoint_objects = Some(field);
            self
        }
        /// Sets `server` with the provided value.
        pub fn with_server<T: Into<String>>(mut self, field: T) -> Self {
            self.server = Some(field.into());
            self
        }
    }
    impl super::GetTransactionsRequest {
        /// Sets `requests` with the provided value.
        pub fn with_requests<T: Into<super::TransactionRequests>>(
            mut self,
            field: T,
        ) -> Self {
            self.requests = Some(field.into());
            self
        }
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
        /// Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.max_message_size_bytes = Some(field);
            self
        }
    }
    impl super::GetTransactionsResponse {
        /// Sets `transaction_results` with the provided value.
        pub fn with_transaction_results(
            mut self,
            field: Vec<super::TransactionResult>,
        ) -> Self {
            self.transaction_results = field;
            self
        }
        /// Sets `has_next` with the provided value.
        pub fn with_has_next(mut self, field: bool) -> Self {
            self.has_next = field;
            self
        }
    }
    impl super::ObjectRequest {
        /// Sets `object_ref` with the provided value.
        pub fn with_object_ref<T: Into<super::super::types::ObjectReference>>(
            mut self,
            field: T,
        ) -> Self {
            self.object_ref = Some(field.into());
            self
        }
    }
    impl super::ObjectRequests {
        /// Sets `requests` with the provided value.
        pub fn with_requests(mut self, field: Vec<super::ObjectRequest>) -> Self {
            self.requests = field;
            self
        }
    }
    impl super::ObjectResult {
        /// Sets `object` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_object<T: Into<super::super::object::Object>>(
            mut self,
            field: T,
        ) -> Self {
            self.result = Some(super::object_result::Result::Object(field.into()));
            self
        }
        /// Sets `error` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_error<
            T: Into<super::super::super::super::super::google::rpc::Status>,
        >(mut self, field: T) -> Self {
            self.result = Some(super::object_result::Result::Error(field.into()));
            self
        }
    }
    impl super::StreamCheckpointsRequest {
        /// Sets `start_sequence_number` with the provided value.
        pub fn with_start_sequence_number(mut self, field: u64) -> Self {
            self.start_sequence_number = Some(field);
            self
        }
        /// Sets `end_sequence_number` with the provided value.
        pub fn with_end_sequence_number(mut self, field: u64) -> Self {
            self.end_sequence_number = Some(field);
            self
        }
        /// Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.read_mask = Some(field.into());
            self
        }
        /// Sets `transactions_filter` with the provided value.
        pub fn with_transactions_filter<
            T: Into<super::super::filter::TransactionFilter>,
        >(mut self, field: T) -> Self {
            self.transactions_filter = Some(field.into());
            self
        }
        /// Sets `events_filter` with the provided value.
        pub fn with_events_filter<T: Into<super::super::filter::EventFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.events_filter = Some(field.into());
            self
        }
        /// Sets `filter_checkpoints` with the provided value.
        pub fn with_filter_checkpoints(mut self, field: bool) -> Self {
            self.filter_checkpoints = Some(field);
            self
        }
        /// Sets `progress_interval_ms` with the provided value.
        pub fn with_progress_interval_ms(mut self, field: u32) -> Self {
            self.progress_interval_ms = Some(field);
            self
        }
        /// Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.max_message_size_bytes = Some(field);
            self
        }
    }
    impl super::TransactionRequest {
        /// Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.digest = Some(field.into());
            self
        }
    }
    impl super::TransactionRequests {
        /// Sets `requests` with the provided value.
        pub fn with_requests(mut self, field: Vec<super::TransactionRequest>) -> Self {
            self.requests = field;
            self
        }
    }
    impl super::TransactionResult {
        /// Sets `executed_transaction` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_executed_transaction<
            T: Into<super::super::transaction::ExecutedTransaction>,
        >(mut self, field: T) -> Self {
            self.result = Some(
                super::transaction_result::Result::ExecutedTransaction(field.into()),
            );
            self
        }
        /// Sets `error` with the provided value.
        /// If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_error<
            T: Into<super::super::super::super::super::google::rpc::Status>,
        >(mut self, field: T) -> Self {
            self.result = Some(super::transaction_result::Result::Error(field.into()));
            self
        }
    }
}
