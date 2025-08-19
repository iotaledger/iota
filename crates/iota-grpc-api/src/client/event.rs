// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::{Stream, StreamExt};
use iota_json_rpc_types::IotaEvent;
use tonic::transport::Channel;

use crate::events::{Event, EventStreamRequest, event_service_client::EventServiceClient};

/// Dedicated client for event-related gRPC operations.
///
/// This client handles all event service interactions including streaming
/// events with filtering capabilities.
#[derive(Clone)]
pub struct EventClient {
    client: EventServiceClient<Channel>,
}

impl EventClient {
    /// Create a new EventClient from a shared gRPC channel.
    pub(super) fn new(channel: Channel) -> Self {
        Self {
            client: EventServiceClient::new(channel),
        }
    }

    /// Stream events with automatic BCS deserialization and filtering.
    ///
    /// # Arguments
    /// * `filter` - Event filter to apply to the stream
    ///
    /// # Returns
    /// A stream of IOTA events that match the specified filter
    pub async fn stream_events(
        &mut self,
        filter: crate::events::EventFilter,
    ) -> Result<impl Stream<Item = Result<IotaEvent, tonic::Status>>, tonic::Status> {
        let request = EventStreamRequest {
            filter: Some(filter),
        };
        let stream = self.client.stream_events(request).await?.into_inner();

        Ok(stream.map(|result| {
            result.and_then(|event| {
                Self::deserialize_event(&event).map_err(|e| {
                    tonic::Status::internal(format!("Failed to deserialize event: {}", e))
                })
            })
        }))
    }

    /// Deserialize event data from BCS bytes.
    fn deserialize_event(event: &Event) -> anyhow::Result<IotaEvent> {
        crate::bcs_event::try_from_bcs_bytes(&event.event_data)
            .map_err(|e| anyhow::anyhow!("BCS deserialization failed: {}", e))
    }
}
