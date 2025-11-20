// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod _accessor_impls {
    #![allow(clippy::useless_conversion)]
    impl super::CheckpointData {
        pub const fn const_default() -> Self {
            Self { payload: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::CheckpointData = super::CheckpointData::const_default();
            &DEFAULT
        }
        ///Returns the value of `checkpoint`, or the default value if `checkpoint` is unset.
        pub fn checkpoint(&self) -> &super::super::checkpoint::Checkpoint {
            if let Some(super::checkpoint_data::Payload::Checkpoint(field)) = &self
                .payload
            {
                field as _
            } else {
                super::super::checkpoint::Checkpoint::default_instance() as _
            }
        }
        ///If `checkpoint` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn checkpoint_opt(&self) -> Option<&super::super::checkpoint::Checkpoint> {
            if let Some(super::checkpoint_data::Payload::Checkpoint(field)) = &self
                .payload
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `checkpoint` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn checkpoint_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::checkpoint::Checkpoint> {
            if let Some(super::checkpoint_data::Payload::Checkpoint(field)) = &mut self
                .payload
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `checkpoint`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn checkpoint_mut(&mut self) -> &mut super::super::checkpoint::Checkpoint {
            if self.checkpoint_opt_mut().is_none() {
                self.payload = Some(
                    super::checkpoint_data::Payload::Checkpoint(
                        super::super::checkpoint::Checkpoint::default(),
                    ),
                );
            }
            self.checkpoint_opt_mut().unwrap()
        }
        ///Sets `checkpoint` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_checkpoint<T: Into<super::super::checkpoint::Checkpoint>>(
            &mut self,
            field: T,
        ) {
            self.payload = Some(
                super::checkpoint_data::Payload::Checkpoint(field.into().into()),
            );
        }
        ///Sets `checkpoint` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_checkpoint<T: Into<super::super::checkpoint::Checkpoint>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_checkpoint(field.into());
            self
        }
        ///Returns the value of `transactions`, or the default value if `transactions` is unset.
        pub fn transactions(&self) -> &super::super::transaction::ExecutedTransaction {
            if let Some(super::checkpoint_data::Payload::Transactions(field)) = &self
                .payload
            {
                field as _
            } else {
                super::super::transaction::ExecutedTransaction::default_instance() as _
            }
        }
        ///If `transactions` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transactions_opt(
            &self,
        ) -> Option<&super::super::transaction::ExecutedTransaction> {
            if let Some(super::checkpoint_data::Payload::Transactions(field)) = &self
                .payload
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `transactions` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transactions_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::transaction::ExecutedTransaction> {
            if let Some(super::checkpoint_data::Payload::Transactions(field)) = &mut self
                .payload
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `transactions`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn transactions_mut(
            &mut self,
        ) -> &mut super::super::transaction::ExecutedTransaction {
            if self.transactions_opt_mut().is_none() {
                self.payload = Some(
                    super::checkpoint_data::Payload::Transactions(
                        super::super::transaction::ExecutedTransaction::default(),
                    ),
                );
            }
            self.transactions_opt_mut().unwrap()
        }
        ///Sets `transactions` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_transactions<T: Into<super::super::transaction::ExecutedTransaction>>(
            &mut self,
            field: T,
        ) {
            self.payload = Some(
                super::checkpoint_data::Payload::Transactions(field.into().into()),
            );
        }
        ///Sets `transactions` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_transactions<
            T: Into<super::super::transaction::ExecutedTransaction>,
        >(mut self, field: T) -> Self {
            self.set_transactions(field.into());
            self
        }
        ///Returns the value of `events`, or the default value if `events` is unset.
        pub fn events(&self) -> &super::super::event::Events {
            if let Some(super::checkpoint_data::Payload::Events(field)) = &self.payload {
                field as _
            } else {
                super::super::event::Events::default_instance() as _
            }
        }
        ///If `events` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn events_opt(&self) -> Option<&super::super::event::Events> {
            if let Some(super::checkpoint_data::Payload::Events(field)) = &self.payload {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `events` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn events_opt_mut(&mut self) -> Option<&mut super::super::event::Events> {
            if let Some(super::checkpoint_data::Payload::Events(field)) = &mut self
                .payload
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `events`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn events_mut(&mut self) -> &mut super::super::event::Events {
            if self.events_opt_mut().is_none() {
                self.payload = Some(
                    super::checkpoint_data::Payload::Events(
                        super::super::event::Events::default(),
                    ),
                );
            }
            self.events_opt_mut().unwrap()
        }
        ///Sets `events` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_events<T: Into<super::super::event::Events>>(&mut self, field: T) {
            self.payload = Some(
                super::checkpoint_data::Payload::Events(field.into().into()),
            );
        }
        ///Sets `events` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_events<T: Into<super::super::event::Events>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_events(field.into());
            self
        }
        ///Returns the value of `end_marker`, or the default value if `end_marker` is unset.
        pub fn end_marker(&self) -> &super::checkpoint_data::EndMarker {
            if let Some(super::checkpoint_data::Payload::EndMarker(field)) = &self
                .payload
            {
                field as _
            } else {
                super::checkpoint_data::EndMarker::default_instance() as _
            }
        }
        ///If `end_marker` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn end_marker_opt(&self) -> Option<&super::checkpoint_data::EndMarker> {
            if let Some(super::checkpoint_data::Payload::EndMarker(field)) = &self
                .payload
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `end_marker` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn end_marker_opt_mut(
            &mut self,
        ) -> Option<&mut super::checkpoint_data::EndMarker> {
            if let Some(super::checkpoint_data::Payload::EndMarker(field)) = &mut self
                .payload
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `end_marker`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn end_marker_mut(&mut self) -> &mut super::checkpoint_data::EndMarker {
            if self.end_marker_opt_mut().is_none() {
                self.payload = Some(
                    super::checkpoint_data::Payload::EndMarker(
                        super::checkpoint_data::EndMarker::default(),
                    ),
                );
            }
            self.end_marker_opt_mut().unwrap()
        }
        ///Sets `end_marker` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_end_marker<T: Into<super::checkpoint_data::EndMarker>>(
            &mut self,
            field: T,
        ) {
            self.payload = Some(
                super::checkpoint_data::Payload::EndMarker(field.into().into()),
            );
        }
        ///Sets `end_marker` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_end_marker<T: Into<super::checkpoint_data::EndMarker>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_end_marker(field.into());
            self
        }
    }
    impl super::checkpoint_data::EndMarker {
        pub const fn const_default() -> Self {
            Self { sequence_number: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::checkpoint_data::EndMarker = super::checkpoint_data::EndMarker::const_default();
            &DEFAULT
        }
        ///If `sequence_number` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn sequence_number_opt_mut(&mut self) -> Option<&mut u64> {
            self.sequence_number.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `sequence_number`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn sequence_number_mut(&mut self) -> &mut u64 {
            self.sequence_number.get_or_insert_default()
        }
        ///If `sequence_number` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn sequence_number_opt(&self) -> Option<u64> {
            self.sequence_number.as_ref().map(|field| *field)
        }
        ///Sets `sequence_number` with the provided value.
        pub fn set_sequence_number(&mut self, field: u64) {
            self.sequence_number = Some(field);
        }
        ///Sets `sequence_number` with the provided value.
        pub fn with_sequence_number(mut self, field: u64) -> Self {
            self.set_sequence_number(field);
            self
        }
    }
    impl super::CheckpointDataStreamRequest {
        pub const fn const_default() -> Self {
            Self {
                start_sequence_number: None,
                end_sequence_number: None,
                checkpoint_read_mask: None,
                transactions_filter: None,
                transaction_read_mask: None,
                events_filter: None,
                event_read_mask: None,
                max_message_size_bytes: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::CheckpointDataStreamRequest = super::CheckpointDataStreamRequest::const_default();
            &DEFAULT
        }
        ///If `start_sequence_number` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn start_sequence_number_opt_mut(&mut self) -> Option<&mut u64> {
            self.start_sequence_number.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `start_sequence_number`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn start_sequence_number_mut(&mut self) -> &mut u64 {
            self.start_sequence_number.get_or_insert_default()
        }
        ///If `start_sequence_number` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn start_sequence_number_opt(&self) -> Option<u64> {
            self.start_sequence_number.as_ref().map(|field| *field)
        }
        ///Sets `start_sequence_number` with the provided value.
        pub fn set_start_sequence_number(&mut self, field: u64) {
            self.start_sequence_number = Some(field);
        }
        ///Sets `start_sequence_number` with the provided value.
        pub fn with_start_sequence_number(mut self, field: u64) -> Self {
            self.set_start_sequence_number(field);
            self
        }
        ///If `end_sequence_number` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn end_sequence_number_opt_mut(&mut self) -> Option<&mut u64> {
            self.end_sequence_number.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `end_sequence_number`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn end_sequence_number_mut(&mut self) -> &mut u64 {
            self.end_sequence_number.get_or_insert_default()
        }
        ///If `end_sequence_number` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn end_sequence_number_opt(&self) -> Option<u64> {
            self.end_sequence_number.as_ref().map(|field| *field)
        }
        ///Sets `end_sequence_number` with the provided value.
        pub fn set_end_sequence_number(&mut self, field: u64) {
            self.end_sequence_number = Some(field);
        }
        ///Sets `end_sequence_number` with the provided value.
        pub fn with_end_sequence_number(mut self, field: u64) -> Self {
            self.set_end_sequence_number(field);
            self
        }
        ///If `checkpoint_read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn checkpoint_read_mask_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost_types::FieldMask> {
            self.checkpoint_read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `checkpoint_read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn checkpoint_read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.checkpoint_read_mask.get_or_insert_default()
        }
        ///If `checkpoint_read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn checkpoint_read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.checkpoint_read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `checkpoint_read_mask` with the provided value.
        pub fn set_checkpoint_read_mask<T: Into<::prost_types::FieldMask>>(
            &mut self,
            field: T,
        ) {
            self.checkpoint_read_mask = Some(field.into().into());
        }
        ///Sets `checkpoint_read_mask` with the provided value.
        pub fn with_checkpoint_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_checkpoint_read_mask(field.into());
            self
        }
        ///Returns the value of `transactions_filter`, or the default value if `transactions_filter` is unset.
        pub fn transactions_filter(&self) -> &super::super::filter::TransactionFilter {
            self.transactions_filter
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::filter::TransactionFilter::default_instance() as _
                })
        }
        ///If `transactions_filter` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transactions_filter_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::filter::TransactionFilter> {
            self.transactions_filter.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `transactions_filter`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transactions_filter_mut(
            &mut self,
        ) -> &mut super::super::filter::TransactionFilter {
            self.transactions_filter.get_or_insert_default()
        }
        ///If `transactions_filter` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transactions_filter_opt(
            &self,
        ) -> Option<&super::super::filter::TransactionFilter> {
            self.transactions_filter.as_ref().map(|field| field as _)
        }
        ///Sets `transactions_filter` with the provided value.
        pub fn set_transactions_filter<T: Into<super::super::filter::TransactionFilter>>(
            &mut self,
            field: T,
        ) {
            self.transactions_filter = Some(field.into().into());
        }
        ///Sets `transactions_filter` with the provided value.
        pub fn with_transactions_filter<
            T: Into<super::super::filter::TransactionFilter>,
        >(mut self, field: T) -> Self {
            self.set_transactions_filter(field.into());
            self
        }
        ///If `transaction_read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transaction_read_mask_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost_types::FieldMask> {
            self.transaction_read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `transaction_read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transaction_read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.transaction_read_mask.get_or_insert_default()
        }
        ///If `transaction_read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transaction_read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.transaction_read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `transaction_read_mask` with the provided value.
        pub fn set_transaction_read_mask<T: Into<::prost_types::FieldMask>>(
            &mut self,
            field: T,
        ) {
            self.transaction_read_mask = Some(field.into().into());
        }
        ///Sets `transaction_read_mask` with the provided value.
        pub fn with_transaction_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_transaction_read_mask(field.into());
            self
        }
        ///Returns the value of `events_filter`, or the default value if `events_filter` is unset.
        pub fn events_filter(&self) -> &super::super::filter::EventFilter {
            self.events_filter
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::filter::EventFilter::default_instance() as _
                })
        }
        ///If `events_filter` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn events_filter_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::filter::EventFilter> {
            self.events_filter.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `events_filter`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn events_filter_mut(&mut self) -> &mut super::super::filter::EventFilter {
            self.events_filter.get_or_insert_default()
        }
        ///If `events_filter` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn events_filter_opt(&self) -> Option<&super::super::filter::EventFilter> {
            self.events_filter.as_ref().map(|field| field as _)
        }
        ///Sets `events_filter` with the provided value.
        pub fn set_events_filter<T: Into<super::super::filter::EventFilter>>(
            &mut self,
            field: T,
        ) {
            self.events_filter = Some(field.into().into());
        }
        ///Sets `events_filter` with the provided value.
        pub fn with_events_filter<T: Into<super::super::filter::EventFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_events_filter(field.into());
            self
        }
        ///If `event_read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn event_read_mask_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost_types::FieldMask> {
            self.event_read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `event_read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn event_read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.event_read_mask.get_or_insert_default()
        }
        ///If `event_read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn event_read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.event_read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `event_read_mask` with the provided value.
        pub fn set_event_read_mask<T: Into<::prost_types::FieldMask>>(
            &mut self,
            field: T,
        ) {
            self.event_read_mask = Some(field.into().into());
        }
        ///Sets `event_read_mask` with the provided value.
        pub fn with_event_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_event_read_mask(field.into());
            self
        }
        ///If `max_message_size_bytes` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn max_message_size_bytes_opt_mut(&mut self) -> Option<&mut u32> {
            self.max_message_size_bytes.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `max_message_size_bytes`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn max_message_size_bytes_mut(&mut self) -> &mut u32 {
            self.max_message_size_bytes.get_or_insert_default()
        }
        ///If `max_message_size_bytes` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn max_message_size_bytes_opt(&self) -> Option<u32> {
            self.max_message_size_bytes.as_ref().map(|field| *field)
        }
        ///Sets `max_message_size_bytes` with the provided value.
        pub fn set_max_message_size_bytes(&mut self, field: u32) {
            self.max_message_size_bytes = Some(field);
        }
        ///Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.set_max_message_size_bytes(field);
            self
        }
    }
    impl super::GetCheckpointDataRequest {
        pub const fn const_default() -> Self {
            Self {
                checkpoint_read_mask: None,
                transactions_filter: None,
                transaction_read_mask: None,
                events_filter: None,
                event_read_mask: None,
                max_message_size_bytes: None,
                checkpoint_id: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::GetCheckpointDataRequest = super::GetCheckpointDataRequest::const_default();
            &DEFAULT
        }
        ///If `checkpoint_read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn checkpoint_read_mask_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost_types::FieldMask> {
            self.checkpoint_read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `checkpoint_read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn checkpoint_read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.checkpoint_read_mask.get_or_insert_default()
        }
        ///If `checkpoint_read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn checkpoint_read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.checkpoint_read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `checkpoint_read_mask` with the provided value.
        pub fn set_checkpoint_read_mask<T: Into<::prost_types::FieldMask>>(
            &mut self,
            field: T,
        ) {
            self.checkpoint_read_mask = Some(field.into().into());
        }
        ///Sets `checkpoint_read_mask` with the provided value.
        pub fn with_checkpoint_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_checkpoint_read_mask(field.into());
            self
        }
        ///Returns the value of `transactions_filter`, or the default value if `transactions_filter` is unset.
        pub fn transactions_filter(&self) -> &super::super::filter::TransactionFilter {
            self.transactions_filter
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::filter::TransactionFilter::default_instance() as _
                })
        }
        ///If `transactions_filter` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transactions_filter_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::filter::TransactionFilter> {
            self.transactions_filter.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `transactions_filter`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transactions_filter_mut(
            &mut self,
        ) -> &mut super::super::filter::TransactionFilter {
            self.transactions_filter.get_or_insert_default()
        }
        ///If `transactions_filter` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transactions_filter_opt(
            &self,
        ) -> Option<&super::super::filter::TransactionFilter> {
            self.transactions_filter.as_ref().map(|field| field as _)
        }
        ///Sets `transactions_filter` with the provided value.
        pub fn set_transactions_filter<T: Into<super::super::filter::TransactionFilter>>(
            &mut self,
            field: T,
        ) {
            self.transactions_filter = Some(field.into().into());
        }
        ///Sets `transactions_filter` with the provided value.
        pub fn with_transactions_filter<
            T: Into<super::super::filter::TransactionFilter>,
        >(mut self, field: T) -> Self {
            self.set_transactions_filter(field.into());
            self
        }
        ///If `transaction_read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transaction_read_mask_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost_types::FieldMask> {
            self.transaction_read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `transaction_read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transaction_read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.transaction_read_mask.get_or_insert_default()
        }
        ///If `transaction_read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transaction_read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.transaction_read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `transaction_read_mask` with the provided value.
        pub fn set_transaction_read_mask<T: Into<::prost_types::FieldMask>>(
            &mut self,
            field: T,
        ) {
            self.transaction_read_mask = Some(field.into().into());
        }
        ///Sets `transaction_read_mask` with the provided value.
        pub fn with_transaction_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_transaction_read_mask(field.into());
            self
        }
        ///Returns the value of `events_filter`, or the default value if `events_filter` is unset.
        pub fn events_filter(&self) -> &super::super::filter::EventFilter {
            self.events_filter
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::filter::EventFilter::default_instance() as _
                })
        }
        ///If `events_filter` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn events_filter_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::filter::EventFilter> {
            self.events_filter.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `events_filter`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn events_filter_mut(&mut self) -> &mut super::super::filter::EventFilter {
            self.events_filter.get_or_insert_default()
        }
        ///If `events_filter` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn events_filter_opt(&self) -> Option<&super::super::filter::EventFilter> {
            self.events_filter.as_ref().map(|field| field as _)
        }
        ///Sets `events_filter` with the provided value.
        pub fn set_events_filter<T: Into<super::super::filter::EventFilter>>(
            &mut self,
            field: T,
        ) {
            self.events_filter = Some(field.into().into());
        }
        ///Sets `events_filter` with the provided value.
        pub fn with_events_filter<T: Into<super::super::filter::EventFilter>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_events_filter(field.into());
            self
        }
        ///If `event_read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn event_read_mask_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost_types::FieldMask> {
            self.event_read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `event_read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn event_read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.event_read_mask.get_or_insert_default()
        }
        ///If `event_read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn event_read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.event_read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `event_read_mask` with the provided value.
        pub fn set_event_read_mask<T: Into<::prost_types::FieldMask>>(
            &mut self,
            field: T,
        ) {
            self.event_read_mask = Some(field.into().into());
        }
        ///Sets `event_read_mask` with the provided value.
        pub fn with_event_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_event_read_mask(field.into());
            self
        }
        ///If `max_message_size_bytes` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn max_message_size_bytes_opt_mut(&mut self) -> Option<&mut u32> {
            self.max_message_size_bytes.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `max_message_size_bytes`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn max_message_size_bytes_mut(&mut self) -> &mut u32 {
            self.max_message_size_bytes.get_or_insert_default()
        }
        ///If `max_message_size_bytes` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn max_message_size_bytes_opt(&self) -> Option<u32> {
            self.max_message_size_bytes.as_ref().map(|field| *field)
        }
        ///Sets `max_message_size_bytes` with the provided value.
        pub fn set_max_message_size_bytes(&mut self, field: u32) {
            self.max_message_size_bytes = Some(field);
        }
        ///Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.set_max_message_size_bytes(field);
            self
        }
        ///Returns the value of `latest`, or the default value if `latest` is unset.
        pub fn latest(&self) -> bool {
            if let Some(
                super::get_checkpoint_data_request::CheckpointId::Latest(field),
            ) = &self.checkpoint_id
            {
                *field
            } else {
                false
            }
        }
        ///If `latest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn latest_opt(&self) -> Option<bool> {
            if let Some(
                super::get_checkpoint_data_request::CheckpointId::Latest(field),
            ) = &self.checkpoint_id
            {
                Some(*field)
            } else {
                None
            }
        }
        ///If `latest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn latest_opt_mut(&mut self) -> Option<&mut bool> {
            if let Some(
                super::get_checkpoint_data_request::CheckpointId::Latest(field),
            ) = &mut self.checkpoint_id
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `latest`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn latest_mut(&mut self) -> &mut bool {
            if self.latest_opt_mut().is_none() {
                self.checkpoint_id = Some(
                    super::get_checkpoint_data_request::CheckpointId::Latest(
                        bool::default(),
                    ),
                );
            }
            self.latest_opt_mut().unwrap()
        }
        ///Sets `latest` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_latest(&mut self, field: bool) {
            self.checkpoint_id = Some(
                super::get_checkpoint_data_request::CheckpointId::Latest(field),
            );
        }
        ///Sets `latest` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_latest(mut self, field: bool) -> Self {
            self.set_latest(field);
            self
        }
        ///Returns the value of `sequence_number`, or the default value if `sequence_number` is unset.
        pub fn sequence_number(&self) -> u64 {
            if let Some(
                super::get_checkpoint_data_request::CheckpointId::SequenceNumber(field),
            ) = &self.checkpoint_id
            {
                *field
            } else {
                0u64
            }
        }
        ///If `sequence_number` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn sequence_number_opt(&self) -> Option<u64> {
            if let Some(
                super::get_checkpoint_data_request::CheckpointId::SequenceNumber(field),
            ) = &self.checkpoint_id
            {
                Some(*field)
            } else {
                None
            }
        }
        ///If `sequence_number` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn sequence_number_opt_mut(&mut self) -> Option<&mut u64> {
            if let Some(
                super::get_checkpoint_data_request::CheckpointId::SequenceNumber(field),
            ) = &mut self.checkpoint_id
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `sequence_number`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn sequence_number_mut(&mut self) -> &mut u64 {
            if self.sequence_number_opt_mut().is_none() {
                self.checkpoint_id = Some(
                    super::get_checkpoint_data_request::CheckpointId::SequenceNumber(
                        u64::default(),
                    ),
                );
            }
            self.sequence_number_opt_mut().unwrap()
        }
        ///Sets `sequence_number` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_sequence_number(&mut self, field: u64) {
            self.checkpoint_id = Some(
                super::get_checkpoint_data_request::CheckpointId::SequenceNumber(field),
            );
        }
        ///Sets `sequence_number` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_sequence_number(mut self, field: u64) -> Self {
            self.set_sequence_number(field);
            self
        }
        ///Returns the value of `digest`, or the default value if `digest` is unset.
        pub fn digest(&self) -> &super::super::types::Digest {
            if let Some(
                super::get_checkpoint_data_request::CheckpointId::Digest(field),
            ) = &self.checkpoint_id
            {
                field as _
            } else {
                super::super::types::Digest::default_instance() as _
            }
        }
        ///If `digest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn digest_opt(&self) -> Option<&super::super::types::Digest> {
            if let Some(
                super::get_checkpoint_data_request::CheckpointId::Digest(field),
            ) = &self.checkpoint_id
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `digest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn digest_opt_mut(&mut self) -> Option<&mut super::super::types::Digest> {
            if let Some(
                super::get_checkpoint_data_request::CheckpointId::Digest(field),
            ) = &mut self.checkpoint_id
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `digest`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn digest_mut(&mut self) -> &mut super::super::types::Digest {
            if self.digest_opt_mut().is_none() {
                self.checkpoint_id = Some(
                    super::get_checkpoint_data_request::CheckpointId::Digest(
                        super::super::types::Digest::default(),
                    ),
                );
            }
            self.digest_opt_mut().unwrap()
        }
        ///Sets `digest` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_digest<T: Into<super::super::types::Digest>>(&mut self, field: T) {
            self.checkpoint_id = Some(
                super::get_checkpoint_data_request::CheckpointId::Digest(
                    field.into().into(),
                ),
            );
        }
        ///Sets `digest` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_digest(field.into());
            self
        }
    }
    impl super::GetEpochRequest {
        pub const fn const_default() -> Self {
            Self {
                epoch: None,
                read_mask: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::GetEpochRequest = super::GetEpochRequest::const_default();
            &DEFAULT
        }
        ///If `epoch` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn epoch_opt_mut(&mut self) -> Option<&mut u64> {
            self.epoch.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `epoch`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn epoch_mut(&mut self) -> &mut u64 {
            self.epoch.get_or_insert_default()
        }
        ///If `epoch` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn epoch_opt(&self) -> Option<u64> {
            self.epoch.as_ref().map(|field| *field)
        }
        ///Sets `epoch` with the provided value.
        pub fn set_epoch(&mut self, field: u64) {
            self.epoch = Some(field);
        }
        ///Sets `epoch` with the provided value.
        pub fn with_epoch(mut self, field: u64) -> Self {
            self.set_epoch(field);
            self
        }
        ///If `read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn read_mask_opt_mut(&mut self) -> Option<&mut ::prost_types::FieldMask> {
            self.read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.read_mask.get_or_insert_default()
        }
        ///If `read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `read_mask` with the provided value.
        pub fn set_read_mask<T: Into<::prost_types::FieldMask>>(&mut self, field: T) {
            self.read_mask = Some(field.into().into());
        }
        ///Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_read_mask(field.into());
            self
        }
    }
    impl super::GetEpochResponse {
        pub const fn const_default() -> Self {
            Self { epoch: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::GetEpochResponse = super::GetEpochResponse::const_default();
            &DEFAULT
        }
        ///Returns the value of `epoch`, or the default value if `epoch` is unset.
        pub fn epoch(&self) -> &super::super::epoch::Epoch {
            self.epoch
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::epoch::Epoch::default_instance() as _)
        }
        ///If `epoch` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn epoch_opt_mut(&mut self) -> Option<&mut super::super::epoch::Epoch> {
            self.epoch.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `epoch`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn epoch_mut(&mut self) -> &mut super::super::epoch::Epoch {
            self.epoch.get_or_insert_default()
        }
        ///If `epoch` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn epoch_opt(&self) -> Option<&super::super::epoch::Epoch> {
            self.epoch.as_ref().map(|field| field as _)
        }
        ///Sets `epoch` with the provided value.
        pub fn set_epoch<T: Into<super::super::epoch::Epoch>>(&mut self, field: T) {
            self.epoch = Some(field.into().into());
        }
        ///Sets `epoch` with the provided value.
        pub fn with_epoch<T: Into<super::super::epoch::Epoch>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_epoch(field.into());
            self
        }
    }
    impl super::GetObjectsRequest {
        pub const fn const_default() -> Self {
            Self {
                requests: None,
                read_mask: None,
                max_message_size_bytes: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::GetObjectsRequest = super::GetObjectsRequest::const_default();
            &DEFAULT
        }
        ///Returns the value of `requests`, or the default value if `requests` is unset.
        pub fn requests(&self) -> &super::ObjectRequests {
            self.requests
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::ObjectRequests::default_instance() as _)
        }
        ///If `requests` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn requests_opt_mut(&mut self) -> Option<&mut super::ObjectRequests> {
            self.requests.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `requests`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn requests_mut(&mut self) -> &mut super::ObjectRequests {
            self.requests.get_or_insert_default()
        }
        ///If `requests` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn requests_opt(&self) -> Option<&super::ObjectRequests> {
            self.requests.as_ref().map(|field| field as _)
        }
        ///Sets `requests` with the provided value.
        pub fn set_requests<T: Into<super::ObjectRequests>>(&mut self, field: T) {
            self.requests = Some(field.into().into());
        }
        ///Sets `requests` with the provided value.
        pub fn with_requests<T: Into<super::ObjectRequests>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_requests(field.into());
            self
        }
        ///If `read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn read_mask_opt_mut(&mut self) -> Option<&mut ::prost_types::FieldMask> {
            self.read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.read_mask.get_or_insert_default()
        }
        ///If `read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `read_mask` with the provided value.
        pub fn set_read_mask<T: Into<::prost_types::FieldMask>>(&mut self, field: T) {
            self.read_mask = Some(field.into().into());
        }
        ///Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_read_mask(field.into());
            self
        }
        ///If `max_message_size_bytes` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn max_message_size_bytes_opt_mut(&mut self) -> Option<&mut u32> {
            self.max_message_size_bytes.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `max_message_size_bytes`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn max_message_size_bytes_mut(&mut self) -> &mut u32 {
            self.max_message_size_bytes.get_or_insert_default()
        }
        ///If `max_message_size_bytes` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn max_message_size_bytes_opt(&self) -> Option<u32> {
            self.max_message_size_bytes.as_ref().map(|field| *field)
        }
        ///Sets `max_message_size_bytes` with the provided value.
        pub fn set_max_message_size_bytes(&mut self, field: u32) {
            self.max_message_size_bytes = Some(field);
        }
        ///Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.set_max_message_size_bytes(field);
            self
        }
    }
    impl super::GetObjectsResponse {
        pub const fn const_default() -> Self {
            Self {
                objects: Vec::new(),
                has_next: false,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::GetObjectsResponse = super::GetObjectsResponse::const_default();
            &DEFAULT
        }
        ///Returns the value of `objects`, or the default value if `objects` is unset.
        pub fn objects(&self) -> &[super::ObjectResult] {
            &self.objects
        }
        ///Returns a mutable reference to `objects`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn objects_mut(&mut self) -> &mut Vec<super::ObjectResult> {
            &mut self.objects
        }
        ///Sets `objects` with the provided value.
        pub fn set_objects(&mut self, field: Vec<super::ObjectResult>) {
            self.objects = field;
        }
        ///Sets `objects` with the provided value.
        pub fn with_objects(mut self, field: Vec<super::ObjectResult>) -> Self {
            self.set_objects(field);
            self
        }
        ///Returns a mutable reference to `has_next`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn has_next_mut(&mut self) -> &mut bool {
            &mut self.has_next
        }
        ///Sets `has_next` with the provided value.
        pub fn set_has_next(&mut self, field: bool) {
            self.has_next = field;
        }
        ///Sets `has_next` with the provided value.
        pub fn with_has_next(mut self, field: bool) -> Self {
            self.set_has_next(field);
            self
        }
    }
    impl super::GetServiceInfoRequest {
        pub const fn const_default() -> Self {
            Self { read_mask: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::GetServiceInfoRequest = super::GetServiceInfoRequest::const_default();
            &DEFAULT
        }
        ///If `read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn read_mask_opt_mut(&mut self) -> Option<&mut ::prost_types::FieldMask> {
            self.read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.read_mask.get_or_insert_default()
        }
        ///If `read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `read_mask` with the provided value.
        pub fn set_read_mask<T: Into<::prost_types::FieldMask>>(&mut self, field: T) {
            self.read_mask = Some(field.into().into());
        }
        ///Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_read_mask(field.into());
            self
        }
    }
    impl super::GetServiceInfoResponse {
        pub const fn const_default() -> Self {
            Self {
                chain_id: None,
                chain: None,
                epoch: None,
                executed_checkpoint_height: None,
                executed_checkpoint_timestamp: None,
                lowest_available_checkpoint: None,
                lowest_available_checkpoint_objects: None,
                server: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::GetServiceInfoResponse = super::GetServiceInfoResponse::const_default();
            &DEFAULT
        }
        ///If `chain_id` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn chain_id_opt_mut(&mut self) -> Option<&mut String> {
            self.chain_id.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `chain_id`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn chain_id_mut(&mut self) -> &mut String {
            self.chain_id.get_or_insert_default()
        }
        ///If `chain_id` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn chain_id_opt(&self) -> Option<&str> {
            self.chain_id.as_ref().map(|field| field as _)
        }
        ///Sets `chain_id` with the provided value.
        pub fn set_chain_id<T: Into<String>>(&mut self, field: T) {
            self.chain_id = Some(field.into().into());
        }
        ///Sets `chain_id` with the provided value.
        pub fn with_chain_id<T: Into<String>>(mut self, field: T) -> Self {
            self.set_chain_id(field.into());
            self
        }
        ///If `chain` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn chain_opt_mut(&mut self) -> Option<&mut String> {
            self.chain.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `chain`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn chain_mut(&mut self) -> &mut String {
            self.chain.get_or_insert_default()
        }
        ///If `chain` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn chain_opt(&self) -> Option<&str> {
            self.chain.as_ref().map(|field| field as _)
        }
        ///Sets `chain` with the provided value.
        pub fn set_chain<T: Into<String>>(&mut self, field: T) {
            self.chain = Some(field.into().into());
        }
        ///Sets `chain` with the provided value.
        pub fn with_chain<T: Into<String>>(mut self, field: T) -> Self {
            self.set_chain(field.into());
            self
        }
        ///If `epoch` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn epoch_opt_mut(&mut self) -> Option<&mut u64> {
            self.epoch.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `epoch`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn epoch_mut(&mut self) -> &mut u64 {
            self.epoch.get_or_insert_default()
        }
        ///If `epoch` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn epoch_opt(&self) -> Option<u64> {
            self.epoch.as_ref().map(|field| *field)
        }
        ///Sets `epoch` with the provided value.
        pub fn set_epoch(&mut self, field: u64) {
            self.epoch = Some(field);
        }
        ///Sets `epoch` with the provided value.
        pub fn with_epoch(mut self, field: u64) -> Self {
            self.set_epoch(field);
            self
        }
        ///If `executed_checkpoint_height` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn executed_checkpoint_height_opt_mut(&mut self) -> Option<&mut u64> {
            self.executed_checkpoint_height.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `executed_checkpoint_height`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn executed_checkpoint_height_mut(&mut self) -> &mut u64 {
            self.executed_checkpoint_height.get_or_insert_default()
        }
        ///If `executed_checkpoint_height` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn executed_checkpoint_height_opt(&self) -> Option<u64> {
            self.executed_checkpoint_height.as_ref().map(|field| *field)
        }
        ///Sets `executed_checkpoint_height` with the provided value.
        pub fn set_executed_checkpoint_height(&mut self, field: u64) {
            self.executed_checkpoint_height = Some(field);
        }
        ///Sets `executed_checkpoint_height` with the provided value.
        pub fn with_executed_checkpoint_height(mut self, field: u64) -> Self {
            self.set_executed_checkpoint_height(field);
            self
        }
        ///If `executed_checkpoint_timestamp` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn executed_checkpoint_timestamp_opt_mut(
            &mut self,
        ) -> Option<&mut ::prost_types::Timestamp> {
            self.executed_checkpoint_timestamp.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `executed_checkpoint_timestamp`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn executed_checkpoint_timestamp_mut(
            &mut self,
        ) -> &mut ::prost_types::Timestamp {
            self.executed_checkpoint_timestamp.get_or_insert_default()
        }
        ///If `executed_checkpoint_timestamp` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn executed_checkpoint_timestamp_opt(
            &self,
        ) -> Option<&::prost_types::Timestamp> {
            self.executed_checkpoint_timestamp.as_ref().map(|field| field as _)
        }
        ///Sets `executed_checkpoint_timestamp` with the provided value.
        pub fn set_executed_checkpoint_timestamp<T: Into<::prost_types::Timestamp>>(
            &mut self,
            field: T,
        ) {
            self.executed_checkpoint_timestamp = Some(field.into().into());
        }
        ///Sets `executed_checkpoint_timestamp` with the provided value.
        pub fn with_executed_checkpoint_timestamp<T: Into<::prost_types::Timestamp>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_executed_checkpoint_timestamp(field.into());
            self
        }
        ///If `lowest_available_checkpoint` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn lowest_available_checkpoint_opt_mut(&mut self) -> Option<&mut u64> {
            self.lowest_available_checkpoint.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `lowest_available_checkpoint`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn lowest_available_checkpoint_mut(&mut self) -> &mut u64 {
            self.lowest_available_checkpoint.get_or_insert_default()
        }
        ///If `lowest_available_checkpoint` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn lowest_available_checkpoint_opt(&self) -> Option<u64> {
            self.lowest_available_checkpoint.as_ref().map(|field| *field)
        }
        ///Sets `lowest_available_checkpoint` with the provided value.
        pub fn set_lowest_available_checkpoint(&mut self, field: u64) {
            self.lowest_available_checkpoint = Some(field);
        }
        ///Sets `lowest_available_checkpoint` with the provided value.
        pub fn with_lowest_available_checkpoint(mut self, field: u64) -> Self {
            self.set_lowest_available_checkpoint(field);
            self
        }
        ///If `lowest_available_checkpoint_objects` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn lowest_available_checkpoint_objects_opt_mut(
            &mut self,
        ) -> Option<&mut u64> {
            self.lowest_available_checkpoint_objects.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `lowest_available_checkpoint_objects`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn lowest_available_checkpoint_objects_mut(&mut self) -> &mut u64 {
            self.lowest_available_checkpoint_objects.get_or_insert_default()
        }
        ///If `lowest_available_checkpoint_objects` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn lowest_available_checkpoint_objects_opt(&self) -> Option<u64> {
            self.lowest_available_checkpoint_objects.as_ref().map(|field| *field)
        }
        ///Sets `lowest_available_checkpoint_objects` with the provided value.
        pub fn set_lowest_available_checkpoint_objects(&mut self, field: u64) {
            self.lowest_available_checkpoint_objects = Some(field);
        }
        ///Sets `lowest_available_checkpoint_objects` with the provided value.
        pub fn with_lowest_available_checkpoint_objects(mut self, field: u64) -> Self {
            self.set_lowest_available_checkpoint_objects(field);
            self
        }
        ///If `server` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn server_opt_mut(&mut self) -> Option<&mut String> {
            self.server.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `server`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn server_mut(&mut self) -> &mut String {
            self.server.get_or_insert_default()
        }
        ///If `server` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn server_opt(&self) -> Option<&str> {
            self.server.as_ref().map(|field| field as _)
        }
        ///Sets `server` with the provided value.
        pub fn set_server<T: Into<String>>(&mut self, field: T) {
            self.server = Some(field.into().into());
        }
        ///Sets `server` with the provided value.
        pub fn with_server<T: Into<String>>(mut self, field: T) -> Self {
            self.set_server(field.into());
            self
        }
    }
    impl super::GetTransactionsRequest {
        pub const fn const_default() -> Self {
            Self {
                requests: None,
                read_mask: None,
                max_message_size_bytes: None,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::GetTransactionsRequest = super::GetTransactionsRequest::const_default();
            &DEFAULT
        }
        ///Returns the value of `requests`, or the default value if `requests` is unset.
        pub fn requests(&self) -> &super::TransactionRequests {
            self.requests
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::TransactionRequests::default_instance() as _)
        }
        ///If `requests` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn requests_opt_mut(&mut self) -> Option<&mut super::TransactionRequests> {
            self.requests.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `requests`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn requests_mut(&mut self) -> &mut super::TransactionRequests {
            self.requests.get_or_insert_default()
        }
        ///If `requests` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn requests_opt(&self) -> Option<&super::TransactionRequests> {
            self.requests.as_ref().map(|field| field as _)
        }
        ///Sets `requests` with the provided value.
        pub fn set_requests<T: Into<super::TransactionRequests>>(&mut self, field: T) {
            self.requests = Some(field.into().into());
        }
        ///Sets `requests` with the provided value.
        pub fn with_requests<T: Into<super::TransactionRequests>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_requests(field.into());
            self
        }
        ///If `read_mask` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn read_mask_opt_mut(&mut self) -> Option<&mut ::prost_types::FieldMask> {
            self.read_mask.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `read_mask`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn read_mask_mut(&mut self) -> &mut ::prost_types::FieldMask {
            self.read_mask.get_or_insert_default()
        }
        ///If `read_mask` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn read_mask_opt(&self) -> Option<&::prost_types::FieldMask> {
            self.read_mask.as_ref().map(|field| field as _)
        }
        ///Sets `read_mask` with the provided value.
        pub fn set_read_mask<T: Into<::prost_types::FieldMask>>(&mut self, field: T) {
            self.read_mask = Some(field.into().into());
        }
        ///Sets `read_mask` with the provided value.
        pub fn with_read_mask<T: Into<::prost_types::FieldMask>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_read_mask(field.into());
            self
        }
        ///If `max_message_size_bytes` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn max_message_size_bytes_opt_mut(&mut self) -> Option<&mut u32> {
            self.max_message_size_bytes.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `max_message_size_bytes`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn max_message_size_bytes_mut(&mut self) -> &mut u32 {
            self.max_message_size_bytes.get_or_insert_default()
        }
        ///If `max_message_size_bytes` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn max_message_size_bytes_opt(&self) -> Option<u32> {
            self.max_message_size_bytes.as_ref().map(|field| *field)
        }
        ///Sets `max_message_size_bytes` with the provided value.
        pub fn set_max_message_size_bytes(&mut self, field: u32) {
            self.max_message_size_bytes = Some(field);
        }
        ///Sets `max_message_size_bytes` with the provided value.
        pub fn with_max_message_size_bytes(mut self, field: u32) -> Self {
            self.set_max_message_size_bytes(field);
            self
        }
    }
    impl super::GetTransactionsResponse {
        pub const fn const_default() -> Self {
            Self {
                transactions: Vec::new(),
                has_next: false,
            }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::GetTransactionsResponse = super::GetTransactionsResponse::const_default();
            &DEFAULT
        }
        ///Returns the value of `transactions`, or the default value if `transactions` is unset.
        pub fn transactions(&self) -> &[super::TransactionResult] {
            &self.transactions
        }
        ///Returns a mutable reference to `transactions`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn transactions_mut(&mut self) -> &mut Vec<super::TransactionResult> {
            &mut self.transactions
        }
        ///Sets `transactions` with the provided value.
        pub fn set_transactions(&mut self, field: Vec<super::TransactionResult>) {
            self.transactions = field;
        }
        ///Sets `transactions` with the provided value.
        pub fn with_transactions(
            mut self,
            field: Vec<super::TransactionResult>,
        ) -> Self {
            self.set_transactions(field);
            self
        }
        ///Returns a mutable reference to `has_next`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn has_next_mut(&mut self) -> &mut bool {
            &mut self.has_next
        }
        ///Sets `has_next` with the provided value.
        pub fn set_has_next(&mut self, field: bool) {
            self.has_next = field;
        }
        ///Sets `has_next` with the provided value.
        pub fn with_has_next(mut self, field: bool) -> Self {
            self.set_has_next(field);
            self
        }
    }
    impl super::ObjectRequest {
        pub const fn const_default() -> Self {
            Self { object_ref: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ObjectRequest = super::ObjectRequest::const_default();
            &DEFAULT
        }
        ///Returns the value of `object_ref`, or the default value if `object_ref` is unset.
        pub fn object_ref(&self) -> &super::super::types::ObjectReference {
            self.object_ref
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| {
                    super::super::types::ObjectReference::default_instance() as _
                })
        }
        ///If `object_ref` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn object_ref_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::types::ObjectReference> {
            self.object_ref.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `object_ref`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn object_ref_mut(&mut self) -> &mut super::super::types::ObjectReference {
            self.object_ref.get_or_insert_default()
        }
        ///If `object_ref` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn object_ref_opt(&self) -> Option<&super::super::types::ObjectReference> {
            self.object_ref.as_ref().map(|field| field as _)
        }
        ///Sets `object_ref` with the provided value.
        pub fn set_object_ref<T: Into<super::super::types::ObjectReference>>(
            &mut self,
            field: T,
        ) {
            self.object_ref = Some(field.into().into());
        }
        ///Sets `object_ref` with the provided value.
        pub fn with_object_ref<T: Into<super::super::types::ObjectReference>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_object_ref(field.into());
            self
        }
    }
    impl super::ObjectRequests {
        pub const fn const_default() -> Self {
            Self { requests: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ObjectRequests = super::ObjectRequests::const_default();
            &DEFAULT
        }
        ///Returns the value of `requests`, or the default value if `requests` is unset.
        pub fn requests(&self) -> &[super::ObjectRequest] {
            &self.requests
        }
        ///Returns a mutable reference to `requests`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn requests_mut(&mut self) -> &mut Vec<super::ObjectRequest> {
            &mut self.requests
        }
        ///Sets `requests` with the provided value.
        pub fn set_requests(&mut self, field: Vec<super::ObjectRequest>) {
            self.requests = field;
        }
        ///Sets `requests` with the provided value.
        pub fn with_requests(mut self, field: Vec<super::ObjectRequest>) -> Self {
            self.set_requests(field);
            self
        }
    }
    impl super::ObjectResult {
        pub const fn const_default() -> Self {
            Self { result: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::ObjectResult = super::ObjectResult::const_default();
            &DEFAULT
        }
        ///Returns the value of `object`, or the default value if `object` is unset.
        pub fn object(&self) -> &super::super::object::Object {
            if let Some(super::object_result::Result::Object(field)) = &self.result {
                field as _
            } else {
                super::super::object::Object::default_instance() as _
            }
        }
        ///If `object` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn object_opt(&self) -> Option<&super::super::object::Object> {
            if let Some(super::object_result::Result::Object(field)) = &self.result {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `object` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn object_opt_mut(&mut self) -> Option<&mut super::super::object::Object> {
            if let Some(super::object_result::Result::Object(field)) = &mut self.result {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `object`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn object_mut(&mut self) -> &mut super::super::object::Object {
            if self.object_opt_mut().is_none() {
                self.result = Some(
                    super::object_result::Result::Object(
                        super::super::object::Object::default(),
                    ),
                );
            }
            self.object_opt_mut().unwrap()
        }
        ///Sets `object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_object<T: Into<super::super::object::Object>>(&mut self, field: T) {
            self.result = Some(
                super::object_result::Result::Object(field.into().into()),
            );
        }
        ///Sets `object` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_object<T: Into<super::super::object::Object>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_object(field.into());
            self
        }
        ///Returns the value of `error`, or the default value if `error` is unset.
        pub fn error(&self) -> &super::super::super::super::super::google::rpc::Status {
            if let Some(super::object_result::Result::Error(field)) = &self.result {
                field as _
            } else {
                super::super::super::super::super::google::rpc::Status::default_instance()
                    as _
            }
        }
        ///If `error` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn error_opt(
            &self,
        ) -> Option<&super::super::super::super::super::google::rpc::Status> {
            if let Some(super::object_result::Result::Error(field)) = &self.result {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `error` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn error_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::super::super::super::google::rpc::Status> {
            if let Some(super::object_result::Result::Error(field)) = &mut self.result {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `error`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn error_mut(
            &mut self,
        ) -> &mut super::super::super::super::super::google::rpc::Status {
            if self.error_opt_mut().is_none() {
                self.result = Some(
                    super::object_result::Result::Error(
                        super::super::super::super::super::google::rpc::Status::default(),
                    ),
                );
            }
            self.error_opt_mut().unwrap()
        }
        ///Sets `error` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_error<
            T: Into<super::super::super::super::super::google::rpc::Status>,
        >(&mut self, field: T) {
            self.result = Some(super::object_result::Result::Error(field.into().into()));
        }
        ///Sets `error` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_error<
            T: Into<super::super::super::super::super::google::rpc::Status>,
        >(mut self, field: T) -> Self {
            self.set_error(field.into());
            self
        }
    }
    impl super::TransactionRequest {
        pub const fn const_default() -> Self {
            Self { digest: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TransactionRequest = super::TransactionRequest::const_default();
            &DEFAULT
        }
        ///Returns the value of `digest`, or the default value if `digest` is unset.
        pub fn digest(&self) -> &super::super::types::Digest {
            self.digest
                .as_ref()
                .map(|field| field as _)
                .unwrap_or_else(|| super::super::types::Digest::default_instance() as _)
        }
        ///If `digest` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn digest_opt_mut(&mut self) -> Option<&mut super::super::types::Digest> {
            self.digest.as_mut().map(|field| field as _)
        }
        ///Returns a mutable reference to `digest`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn digest_mut(&mut self) -> &mut super::super::types::Digest {
            self.digest.get_or_insert_default()
        }
        ///If `digest` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn digest_opt(&self) -> Option<&super::super::types::Digest> {
            self.digest.as_ref().map(|field| field as _)
        }
        ///Sets `digest` with the provided value.
        pub fn set_digest<T: Into<super::super::types::Digest>>(&mut self, field: T) {
            self.digest = Some(field.into().into());
        }
        ///Sets `digest` with the provided value.
        pub fn with_digest<T: Into<super::super::types::Digest>>(
            mut self,
            field: T,
        ) -> Self {
            self.set_digest(field.into());
            self
        }
    }
    impl super::TransactionRequests {
        pub const fn const_default() -> Self {
            Self { requests: Vec::new() }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TransactionRequests = super::TransactionRequests::const_default();
            &DEFAULT
        }
        ///Returns the value of `requests`, or the default value if `requests` is unset.
        pub fn requests(&self) -> &[super::TransactionRequest] {
            &self.requests
        }
        ///Returns a mutable reference to `requests`.
        ///If the field is unset, it is first initialized with the default value.
        pub fn requests_mut(&mut self) -> &mut Vec<super::TransactionRequest> {
            &mut self.requests
        }
        ///Sets `requests` with the provided value.
        pub fn set_requests(&mut self, field: Vec<super::TransactionRequest>) {
            self.requests = field;
        }
        ///Sets `requests` with the provided value.
        pub fn with_requests(mut self, field: Vec<super::TransactionRequest>) -> Self {
            self.set_requests(field);
            self
        }
    }
    impl super::TransactionResult {
        pub const fn const_default() -> Self {
            Self { result: None }
        }
        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: super::TransactionResult = super::TransactionResult::const_default();
            &DEFAULT
        }
        ///Returns the value of `transaction`, or the default value if `transaction` is unset.
        pub fn transaction(&self) -> &super::super::transaction::ExecutedTransaction {
            if let Some(super::transaction_result::Result::Transaction(field)) = &self
                .result
            {
                field as _
            } else {
                super::super::transaction::ExecutedTransaction::default_instance() as _
            }
        }
        ///If `transaction` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn transaction_opt(
            &self,
        ) -> Option<&super::super::transaction::ExecutedTransaction> {
            if let Some(super::transaction_result::Result::Transaction(field)) = &self
                .result
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `transaction` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn transaction_opt_mut(
            &mut self,
        ) -> Option<
            &mut ::prost::alloc::boxed::Box<
                super::super::transaction::ExecutedTransaction,
            >,
        > {
            if let Some(super::transaction_result::Result::Transaction(field)) = &mut self
                .result
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `transaction`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn transaction_mut(
            &mut self,
        ) -> &mut ::prost::alloc::boxed::Box<
            super::super::transaction::ExecutedTransaction,
        > {
            if self.transaction_opt_mut().is_none() {
                self.result = Some(
                    super::transaction_result::Result::Transaction(
                        ::prost::alloc::boxed::Box::default(),
                    ),
                );
            }
            self.transaction_opt_mut().unwrap()
        }
        ///Sets `transaction` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_transaction<
            T: Into<
                    ::prost::alloc::boxed::Box<
                        super::super::transaction::ExecutedTransaction,
                    >,
                >,
        >(&mut self, field: T) {
            self.result = Some(
                super::transaction_result::Result::Transaction(field.into().into()),
            );
        }
        ///Sets `transaction` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_transaction<
            T: Into<
                    ::prost::alloc::boxed::Box<
                        super::super::transaction::ExecutedTransaction,
                    >,
                >,
        >(mut self, field: T) -> Self {
            self.set_transaction(field.into());
            self
        }
        ///Returns the value of `error`, or the default value if `error` is unset.
        pub fn error(&self) -> &super::super::super::super::super::google::rpc::Status {
            if let Some(super::transaction_result::Result::Error(field)) = &self.result {
                field as _
            } else {
                super::super::super::super::super::google::rpc::Status::default_instance()
                    as _
            }
        }
        ///If `error` is set, returns [`Some`] with the value; otherwise returns [`None`].
        pub fn error_opt(
            &self,
        ) -> Option<&super::super::super::super::super::google::rpc::Status> {
            if let Some(super::transaction_result::Result::Error(field)) = &self.result {
                Some(field as _)
            } else {
                None
            }
        }
        ///If `error` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`].
        pub fn error_opt_mut(
            &mut self,
        ) -> Option<&mut super::super::super::super::super::google::rpc::Status> {
            if let Some(super::transaction_result::Result::Error(field)) = &mut self
                .result
            {
                Some(field as _)
            } else {
                None
            }
        }
        ///Returns a mutable reference to `error`.
        ///If the field is unset, it is first initialized with the default value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn error_mut(
            &mut self,
        ) -> &mut super::super::super::super::super::google::rpc::Status {
            if self.error_opt_mut().is_none() {
                self.result = Some(
                    super::transaction_result::Result::Error(
                        super::super::super::super::super::google::rpc::Status::default(),
                    ),
                );
            }
            self.error_opt_mut().unwrap()
        }
        ///Sets `error` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn set_error<
            T: Into<super::super::super::super::super::google::rpc::Status>,
        >(&mut self, field: T) {
            self.result = Some(
                super::transaction_result::Result::Error(field.into().into()),
            );
        }
        ///Sets `error` with the provided value.
        ///If any other oneof field in the same oneof is set, it will be cleared.
        pub fn with_error<
            T: Into<super::super::super::super::super::google::rpc::Status>,
        >(mut self, field: T) -> Self {
            self.set_error(field.into());
            self
        }
    }
}
