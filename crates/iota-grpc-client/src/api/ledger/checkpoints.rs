// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for checkpoint queries.
//!
//! # Available Read Mask Fields
//!
//! All checkpoint query methods support the following `read_mask` fields to
//! control which data is included in the response:
//!
//! ## Checkpoint Fields
//! - `checkpoint` - includes all checkpoint fields
//!   - `checkpoint.sequence_number` - the sequence number of the checkpoint
//!   - `checkpoint.summary` - includes all checkpoint summary fields
//!     - `checkpoint.summary.digest` - the digest of the checkpoint summary
//!     - `checkpoint.summary.bcs` - the full BCS-encoded checkpoint summary
//!   - `checkpoint.contents` - includes all checkpoint contents fields
//!     - `checkpoint.contents.digest` - the digest of the checkpoint contents
//!     - `checkpoint.contents.bcs` - the full BCS-encoded checkpoint contents
//!   - `checkpoint.signature` - the validator aggregated signature for the
//!     checkpoint
//!
//! ## Transaction Fields
//! - `transactions` - includes all executed transaction fields
//!   - `transactions.transaction` - includes all transaction fields
//!     - `transactions.transaction.digest` - the transaction digest
//!     - `transactions.transaction.bcs` - the full BCS-encoded transaction
//!   - `transactions.signatures` - includes all signature fields
//!     - `transactions.signatures.bcs` - the full BCS-encoded signature
//!   - `transactions.effects` - includes all effects fields
//!     - `transactions.effects.digest` - the effects digest
//!     - `transactions.effects.bcs` - the full BCS-encoded effects
//!   - `transactions.events` - includes all event fields (all events of the
//!     transaction)
//!     - `transactions.events.digest` - the events digest
//!     - `transactions.events.events` - includes all event fields
//!       - `transactions.events.events.bcs` - the full BCS-encoded event
//!       - `transactions.events.events.package_id` - the ID of the package that
//!         emitted the event
//!       - `transactions.events.events.module` - the module that emitted the
//!         event
//!       - `transactions.events.events.sender` - the sender that triggered the
//!         event
//!       - `transactions.events.events.event_type` - the type of the event
//!       - `transactions.events.events.bcs_contents` - the full BCS-encoded
//!         contents of the event
//!       - `transactions.events.events.json_contents` - the JSON-encoded
//!         contents of the event
//!   - `transactions.checkpoint` - the checkpoint that included the transaction
//!   - `transactions.timestamp` - the timestamp of the checkpoint that included
//!     the transaction
//!   - `transactions.input_objects` - includes all input object fields
//!     - `transactions.input_objects.reference` - includes all reference fields
//!       - `transactions.input_objects.reference.object_id` - the ID of the
//!         input object
//!       - `transactions.input_objects.reference.version` - the version of the
//!         input object
//!       - `transactions.input_objects.reference.digest` - the digest of the
//!         input object contents
//!     - `transactions.input_objects.bcs` - the full BCS-encoded object
//!   - `transactions.output_objects` - includes all output object fields
//!     - `transactions.output_objects.reference` - includes all reference
//!       fields
//!       - `transactions.output_objects.reference.object_id` - the ID of the
//!         output object
//!       - `transactions.output_objects.reference.version` - the version of the
//!         output object
//!       - `transactions.output_objects.reference.digest` - the digest of the
//!         output object contents
//!     - `transactions.output_objects.bcs` - the full BCS-encoded object
//!
//! ## Event Fields
//! - `events` - includes all event fields (all events of all transactions in
//!   the checkpoint)
//!   - `events.bcs` - the full BCS-encoded event
//!   - `events.package_id` - the ID of the package that emitted the event
//!   - `events.module` - the module that emitted the event
//!   - `events.sender` - the sender that triggered the event
//!   - `events.event_type` - the type of the event
//!   - `events.bcs_contents` - the full BCS-encoded contents of the event
//!   - `events.json_contents` - the JSON-encoded contents of the event

use std::pin::Pin;

use futures::{Stream, StreamExt};
use iota_grpc_types::v1::{
    checkpoint, event, filter as grpc_filter,
    ledger_service::{
        GetCheckpointRequest, StreamCheckpointsRequest, checkpoint_data, get_checkpoint_request,
    },
    signatures::ValidatorAggregatedSignature as ProtoValidatorAggregatedSignature,
    transaction::ExecutedTransaction,
};
use iota_sdk_types::{CheckpointSequenceNumber, Digest};

use crate::{
    Client, Error,
    api::{
        CheckpointResponse, CheckpointStreamItem, GET_CHECKPOINT_READ_MASK, MetadataEnvelope,
        Result, TryFromProtoError, field_mask_with_default, saturating_usize_to_u32,
    },
};

impl Client {
    /// Get the latest checkpoint.
    ///
    /// This retrieves the checkpoint including summary, contents,
    /// transactions, and events based on the provided read mask.
    ///
    /// # Parameters
    ///
    /// * `read_mask` - Optional field mask specifying which fields to include.
    ///   If `None`, uses [`crate::api::GET_CHECKPOINT_READ_MASK`] as default.
    ///   See [module-level documentation](crate::api::ledger::checkpoints) for
    ///   all available fields.
    /// * `transactions_filter` - Optional filter to apply to transactions
    /// * `events_filter` - Optional filter to apply to events
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let checkpoint = client.get_checkpoint_latest(None, None, None).await?;
    /// println!("Received checkpoint {}", checkpoint.body().sequence_number,);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_checkpoint_latest(
        &self,
        read_mask: Option<&str>,
        transactions_filter: Option<grpc_filter::TransactionFilter>,
        events_filter: Option<grpc_filter::EventFilter>,
    ) -> Result<MetadataEnvelope<CheckpointResponse>> {
        self.get_checkpoint_internal(
            get_checkpoint_request::CheckpointId::Latest(true),
            read_mask,
            transactions_filter,
            events_filter,
        )
        .await
    }

    /// Get checkpoint by sequence number.
    ///
    /// This retrieves the checkpoint including summary, contents,
    /// transactions, and events based on the provided read mask.
    ///
    /// # Parameters
    ///
    /// * `sequence_number` - The checkpoint sequence number to fetch
    /// * `read_mask` - Optional field mask specifying which fields to include.
    ///   If `None`, uses [`crate::api::GET_CHECKPOINT_READ_MASK`] as default.
    ///   See [module-level documentation](crate::api::ledger::checkpoints) for
    ///   all available fields.
    /// * `transactions_filter` - Optional filter to apply to transactions
    /// * `events_filter` - Optional filter to apply to events
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let checkpoint = client
    ///     .get_checkpoint_by_sequence_number(100, None, None, None)
    ///     .await?;
    /// println!("Received checkpoint {}", checkpoint.body().sequence_number,);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
        read_mask: Option<&str>,
        transactions_filter: Option<grpc_filter::TransactionFilter>,
        events_filter: Option<grpc_filter::EventFilter>,
    ) -> Result<MetadataEnvelope<CheckpointResponse>> {
        self.get_checkpoint_internal(
            get_checkpoint_request::CheckpointId::SequenceNumber(sequence_number),
            read_mask,
            transactions_filter,
            events_filter,
        )
        .await
    }

    /// Get checkpoint by digest.
    ///
    /// This retrieves the checkpoint including summary, contents,
    /// transactions, and events based on the provided read mask.
    ///
    /// # Parameters
    ///
    /// * `digest` - The checkpoint digest to fetch
    /// * `read_mask` - Optional field mask specifying which fields to include.
    ///   If `None`, uses [`crate::api::GET_CHECKPOINT_READ_MASK`] as default.
    ///   See [module-level documentation](crate::api::ledger::checkpoints) for
    ///   all available fields.
    /// * `transactions_filter` - Optional filter to apply to transactions
    /// * `events_filter` - Optional filter to apply to events
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::Digest;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let digest: Digest = todo!();
    /// let checkpoint = client
    ///     .get_checkpoint_by_digest(digest, None, None, None)
    ///     .await?;
    /// println!("Received checkpoint {}", checkpoint.body().sequence_number,);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_checkpoint_by_digest(
        &self,
        digest: Digest,
        read_mask: Option<&str>,
        transactions_filter: Option<grpc_filter::TransactionFilter>,
        events_filter: Option<grpc_filter::EventFilter>,
    ) -> Result<MetadataEnvelope<CheckpointResponse>> {
        self.get_checkpoint_internal(
            get_checkpoint_request::CheckpointId::Digest(digest.into()),
            read_mask,
            transactions_filter,
            events_filter,
        )
        .await
    }

    /// Internal helper to fetch checkpoint by any ID type.
    async fn get_checkpoint_internal(
        &self,
        checkpoint_id: get_checkpoint_request::CheckpointId,
        read_mask: Option<&str>,
        transactions_filter: Option<grpc_filter::TransactionFilter>,
        events_filter: Option<grpc_filter::EventFilter>,
    ) -> Result<MetadataEnvelope<CheckpointResponse>> {
        let mut request = match checkpoint_id {
            get_checkpoint_request::CheckpointId::Latest(val) => {
                GetCheckpointRequest::default().with_latest(val)
            }
            get_checkpoint_request::CheckpointId::SequenceNumber(val) => {
                GetCheckpointRequest::default().with_sequence_number(val)
            }
            get_checkpoint_request::CheckpointId::Digest(val) => {
                GetCheckpointRequest::default().with_digest(val)
            }
            _ => {
                return Err(Error::Protocol("Invalid checkpoint ID type".into()));
            }
        }
        .with_read_mask(field_mask_with_default(read_mask, GET_CHECKPOINT_READ_MASK));

        if let Some(tf) = transactions_filter {
            request = request.with_transactions_filter(tf);
        }
        if let Some(ef) = events_filter {
            request = request.with_events_filter(ef);
        }
        if let Some(max_size) = self
            .max_decoding_message_size()
            .map(saturating_usize_to_u32)
        {
            request = request.with_max_message_size_bytes(max_size);
        }

        let mut client = self.ledger_service_client();
        let response = client.get_checkpoint(request).await?;
        let (stream, metadata) = MetadataEnvelope::from(response).into_parts();

        let reassembled = Self::reassemble_checkpoint_data_stream(stream);
        futures::pin_mut!(reassembled);

        // Skip any progress messages and find the first checkpoint
        let checkpoint = loop {
            match reassembled.next().await {
                Some(Ok(CheckpointStreamItem::Checkpoint(cp))) => break *cp,
                Some(Ok(CheckpointStreamItem::Progress { .. })) => continue,
                Some(Err(e)) => return Err(e),
                None => {
                    return Err(TryFromProtoError::missing("checkpoint data").into());
                }
            }
        };

        Ok(MetadataEnvelope::new(checkpoint, metadata))
    }

    /// Stream checkpoints across a range of checkpoints.
    ///
    /// Returns a stream of [`CheckpointResponse`] objects, each representing
    /// a complete checkpoint with its transactions and events. Every checkpoint
    /// in the range is yielded, even if the filters produce no matching
    /// transactions or events within it.
    ///
    /// To skip non-matching checkpoints entirely, use
    /// [`stream_checkpoints_filtered`](Self::stream_checkpoints_filtered).
    ///
    /// **Note:** The metadata in the returned [`MetadataEnvelope`] is captured
    /// from the initial gRPC response headers when the stream is opened. It is
    /// **not** updated as subsequent checkpoint data arrives.
    ///
    /// # Parameters
    ///
    /// * `start_sequence_number` - Optional starting checkpoint. If `None`,
    ///   starts from the latest checkpoint.
    /// * `end_sequence_number` - Optional ending checkpoint. If `None`, streams
    ///   indefinitely.
    /// * `read_mask` - Optional field mask specifying which fields to include.
    ///   If `None`, uses [`crate::api::GET_CHECKPOINT_READ_MASK`] as default.
    ///   See [module-level documentation](crate::api::ledger::checkpoints) for
    ///   all available fields.
    /// * `transactions_filter` - Optional filter to apply to transactions
    /// * `events_filter` - Optional filter to apply to events
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use futures::StreamExt;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let mut stream = client
    ///     .stream_checkpoints(Some(0), Some(10), None, None, None)
    ///     .await?;
    ///
    /// while let Some(checkpoint) = stream.body_mut().next().await {
    ///     let checkpoint = checkpoint?;
    ///     println!("Received checkpoint {}", checkpoint.sequence_number);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stream_checkpoints(
        &self,
        start_sequence_number: Option<CheckpointSequenceNumber>,
        end_sequence_number: Option<CheckpointSequenceNumber>,
        read_mask: Option<&str>,
        transactions_filter: Option<grpc_filter::TransactionFilter>,
        events_filter: Option<grpc_filter::EventFilter>,
    ) -> Result<MetadataEnvelope<Pin<Box<dyn Stream<Item = Result<CheckpointResponse>> + Send>>>>
    {
        let envelope = self
            .stream_checkpoints_raw(
                start_sequence_number,
                end_sequence_number,
                read_mask,
                transactions_filter,
                events_filter,
                false,
                None,
            )
            .await?;

        let (stream, metadata) = envelope.into_parts();

        // remove the wrapping CheckpointStreamItem layer since we know
        // filter_checkpoints is false and thus only Checkpoint items will be produced
        let filtered = stream.filter_map(|item| async {
            match item {
                Ok(CheckpointStreamItem::Checkpoint(cp)) => Some(Ok(*cp)),
                Ok(CheckpointStreamItem::Progress { .. }) => None,
                Err(e) => Some(Err(e)),
            }
        });

        Ok(MetadataEnvelope::new(Box::pin(filtered), metadata))
    }

    /// Stream checkpoints, skipping those with no matching data.
    ///
    /// Unlike [`stream_checkpoints`](Self::stream_checkpoints), this method
    /// sets `filter_checkpoints = true` on the server, which means checkpoints
    /// without any matching transactions or events are skipped entirely.
    ///
    /// The returned stream yields [`CheckpointStreamItem`], which is either a
    /// [`CheckpointStreamItem::Checkpoint`] or a
    /// [`CheckpointStreamItem::Progress`]. Progress messages are sent
    /// periodically during scanning to indicate liveness and the current scan
    /// position (default every 2 seconds, configurable via
    /// `progress_interval_ms`).
    ///
    /// For liveness detection, wrap `stream.next()` in
    /// `tokio::time::timeout()` — if neither a `Checkpoint` nor a `Progress`
    /// arrives within your chosen duration plus some buffer for connection
    /// latency, the connection is likely dead.
    ///
    /// At least one of `transactions_filter` or `events_filter` must be set.
    ///
    /// # Parameters
    ///
    /// * `start_sequence_number` - Optional starting checkpoint. If `None`,
    ///   starts from the latest checkpoint.
    /// * `end_sequence_number` - Optional ending checkpoint. If `None`, streams
    ///   indefinitely.
    /// * `read_mask` - Optional field mask specifying which fields to include.
    ///   If `None`, uses [`crate::api::GET_CHECKPOINT_READ_MASK`] as default.
    ///   See [module-level documentation](crate::api::ledger::checkpoints) for
    ///   all available fields.
    /// * `transactions_filter` - Optional filter to apply to transactions
    /// * `events_filter` - Optional filter to apply to events
    /// * `progress_interval_ms` - Optional progress message interval in
    ///   milliseconds. Defaults to 2000ms. Minimum 500ms.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::{Client, CheckpointStreamItem};
    /// # use iota_grpc_types::v1::filter as grpc_filter;
    /// # use futures::StreamExt;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// // At least one filter is required
    /// let tx_filter = grpc_filter::TransactionFilter::default();
    /// let mut stream = client
    ///     .stream_checkpoints_filtered(Some(0), None, None, Some(tx_filter), None, None)
    ///     .await?;
    ///
    /// while let Some(item) = stream.body_mut().next().await {
    ///     match item? {
    ///         CheckpointStreamItem::Checkpoint(cp) => {
    ///             println!("Matched checkpoint {}", cp.sequence_number);
    ///         }
    ///         CheckpointStreamItem::Progress {
    ///             latest_scanned_sequence_number,
    ///         } => {
    ///             println!("Scanned up to {latest_scanned_sequence_number}");
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stream_checkpoints_filtered(
        &self,
        start_sequence_number: Option<CheckpointSequenceNumber>,
        end_sequence_number: Option<CheckpointSequenceNumber>,
        read_mask: Option<&str>,
        transactions_filter: Option<grpc_filter::TransactionFilter>,
        events_filter: Option<grpc_filter::EventFilter>,
        progress_interval_ms: Option<u32>,
    ) -> Result<MetadataEnvelope<Pin<Box<dyn Stream<Item = Result<CheckpointStreamItem>> + Send>>>>
    {
        self.stream_checkpoints_raw(
            start_sequence_number,
            end_sequence_number,
            read_mask,
            transactions_filter,
            events_filter,
            true,
            progress_interval_ms,
        )
        .await
    }

    /// Internal helper that builds the stream request and returns the raw
    /// [`CheckpointStreamItem`] stream.
    async fn stream_checkpoints_raw(
        &self,
        start_sequence_number: Option<CheckpointSequenceNumber>,
        end_sequence_number: Option<CheckpointSequenceNumber>,
        read_mask: Option<&str>,
        transactions_filter: Option<grpc_filter::TransactionFilter>,
        events_filter: Option<grpc_filter::EventFilter>,
        filter_checkpoints: bool,
        progress_interval_ms: Option<u32>,
    ) -> Result<MetadataEnvelope<Pin<Box<dyn Stream<Item = Result<CheckpointStreamItem>> + Send>>>>
    {
        let mut request = StreamCheckpointsRequest::default()
            .with_read_mask(field_mask_with_default(read_mask, GET_CHECKPOINT_READ_MASK));

        if let Some(start) = start_sequence_number {
            request = request.with_start_sequence_number(start);
        }
        if let Some(end) = end_sequence_number {
            request = request.with_end_sequence_number(end);
        }
        if let Some(tf) = transactions_filter {
            request = request.with_transactions_filter(tf);
        }
        if let Some(ef) = events_filter {
            request = request.with_events_filter(ef);
        }
        if filter_checkpoints {
            request = request.with_filter_checkpoints(true);
        }
        if let Some(ms) = progress_interval_ms {
            request = request.with_progress_interval_ms(ms);
        }
        if let Some(max_size) = self
            .max_decoding_message_size()
            .map(saturating_usize_to_u32)
        {
            request = request.with_max_message_size_bytes(max_size);
        }

        let mut client = self.ledger_service_client();
        let response = client.stream_checkpoints(request).await?;
        let (stream, metadata) = MetadataEnvelope::from(response).into_parts();

        Ok(MetadataEnvelope::new(
            Box::pin(Self::reassemble_checkpoint_data_stream(stream)),
            metadata,
        ))
    }

    /// Reassemble a stream of checkpoint data chunks into complete checkpoints.
    ///
    /// The server sends checkpoint data in multiple messages:
    /// - `Checkpoint` - Contains the checkpoint summary and contents
    /// - `Transactions` - Contains executed transactions
    /// - `Events` - Contains events from transactions
    /// - `Progress` - Liveness indicator during filtered scanning
    /// - `EndMarker` - Signals the end of one checkpoint's data
    ///
    /// This function buffers the chunks and yields [`CheckpointStreamItem`]
    /// values: either complete [`CheckpointResponse`] objects when an
    /// `EndMarker` is received, or [`CheckpointStreamItem::Progress`] when
    /// a progress message arrives.
    fn reassemble_checkpoint_data_stream<S, E>(
        stream: S,
    ) -> impl Stream<Item = Result<CheckpointStreamItem>>
    where
        S: Stream<
            Item = std::result::Result<iota_grpc_types::v1::ledger_service::CheckpointData, E>,
        >,
        E: Into<Error>,
    {
        async_stream::try_stream! {
            futures::pin_mut!(stream);

            // State for accumulating checkpoint data
            let mut current_sequence_number: Option<CheckpointSequenceNumber> = None;
            let mut current_summary: Option<checkpoint::CheckpointSummary> = None;
            let mut current_signature: Option<ProtoValidatorAggregatedSignature> = None;
            let mut current_contents: Option<checkpoint::CheckpointContents> = None;
            let mut current_transactions: Vec<ExecutedTransaction> = Vec::new();
            let mut current_events: Vec<event::Event> = Vec::new();

            while let Some(data) = stream.next().await {
                let data = data.map_err(|e| e.into())?;

                match data.payload {
                    Some(checkpoint_data::Payload::Checkpoint(checkpoint)) => {
                        if checkpoint.sequence_number.is_none() {
                            Err(TryFromProtoError::missing("checkpoint.sequence_number"))?;
                        }

                        // Start of new checkpoint - throw error if previous checkpoint was incomplete
                        if current_sequence_number.is_some() {
                            Err(Error::Protocol("Received new chunked checkpoint header before completing previous checkpoint".into()))?;
                        }
                        current_sequence_number = checkpoint.sequence_number;

                        // Store proto summary (optional, no deserialization)
                        current_summary = checkpoint.summary;

                        // Store proto signature (optional, no deserialization)
                        current_signature = checkpoint.signature;

                        // Store proto contents (optional, no deserialization)
                        current_contents = checkpoint.contents;

                        // Reset accumulators for new checkpoint (in case Transactions or Events
                        // arrived between endmarker and Checkpoint)
                        current_transactions.clear();
                        current_events.clear();
                    }

                    Some(checkpoint_data::Payload::ExecutedTransactions(txs)) => {
                        if current_sequence_number.is_none() {
                            Err(Error::Protocol("Received new chunked checkpoint transactions before receiving checkpoint header".into()))?;
                        }

                        // Accumulate proto transactions (no deserialization)
                        current_transactions.extend(txs.executed_transactions.into_iter());
                    }

                    Some(checkpoint_data::Payload::Events(events)) => {
                        if current_sequence_number.is_none() {
                            Err(Error::Protocol("Received new chunked checkpoint events before receiving checkpoint header".into()))?;
                        }

                        // Accumulate proto events (no deserialization)
                        current_events.extend(events.events);
                    }

                    Some(checkpoint_data::Payload::EndMarker(marker)) => {
                        // End of current checkpoint - assemble the result and yield it
                         let sequence_number = current_sequence_number
                        .take()
                        .ok_or_else(|| -> Error { Error::Protocol("Received checkpoint end marker before receiving checkpoint header".into()) })?;

                        let marker_sequence_number = marker.sequence_number
                        .ok_or_else(|| -> Error { TryFromProtoError::missing("end_marker.sequence_number").into() })?;

                        if marker_sequence_number != sequence_number {
                            Err(Error::Protocol(format!(
                                "EndMarker sequence_number {marker_sequence_number} does not match current checkpoint sequence_number {sequence_number:?}",
                            )))?;
                        }

                        let response = CheckpointResponse {
                            sequence_number,
                            summary: current_summary.take(),
                            signature: current_signature.take(),
                            contents: current_contents.take(),
                            executed_transactions: std::mem::take(&mut current_transactions),
                            events: std::mem::take(&mut current_events),
                        };

                        yield CheckpointStreamItem::Checkpoint(Box::new(response));
                    }

                    Some(checkpoint_data::Payload::Progress(progress)) => {
                        yield CheckpointStreamItem::Progress {
                            latest_scanned_sequence_number: progress.latest_scanned_sequence_number,
                        };
                    }

                    None => {
                        // Empty payload - skip
                        continue;
                    }

                    Some(_) => {
                        // Unknown payload type
                        Err(Error::Protocol("Received unknown checkpoint data payload type".into()))?;
                    }
                }
            }

            // Check if stream ended with incomplete checkpoint data
            if let Some(sequence_number) = current_sequence_number {
                Err(Error::Protocol(format!(
                    "Stream ended with incomplete checkpoint data for sequence number {sequence_number}"
                )))?;
            }
        }
    }
}
