// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use futures::{Stream, StreamExt};
use iota_grpc_types::v0::events as grpc_events;
use iota_json_rpc_types::{BcsEvent, IotaEvent};
use iota_types::{
    base_types::{IotaAddress, ObjectID, TransactionDigest},
    event::EventID,
};
use move_core_types::{identifier::Identifier, language_storage::StructTag};
use tonic::transport::Channel;

/// Dedicated client for event-related gRPC operations.
///
/// This client handles all event service interactions including streaming
/// events with filtering capabilities.
#[derive(Clone)]
pub struct EventClient {
    client: grpc_events::event_service_client::EventServiceClient<Channel>,
}

impl EventClient {
    /// Create a new EventClient from a shared gRPC channel.
    pub(super) fn new(channel: Channel) -> Self {
        Self {
            client: grpc_events::event_service_client::EventServiceClient::new(channel),
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
        filter: grpc_events::EventFilter,
    ) -> Result<impl Stream<Item = Result<IotaEvent, tonic::Status>>, tonic::Status> {
        let request = grpc_events::EventStreamRequest {
            filter: Some(filter),
        };
        let stream = self.client.stream_events(request).await?.into_inner();

        Ok(stream.map(|result| {
            result.and_then(|event| {
                Self::deserialize_event(&event).map_err(|e| {
                    tonic::Status::internal(format!("Failed to deserialize event: {e}"))
                })
            })
        }))
    }

    /// Deserialize event data from BCS bytes.
    fn deserialize_event(event: &grpc_events::Event) -> anyhow::Result<IotaEvent> {
        let event_id = event
            .event_id
            .as_ref()
            .ok_or_else(|| anyhow!("Missing event ID"))?;

        let tx_digest = event_id
            .tx_digest
            .as_ref()
            .ok_or_else(|| anyhow!("Missing transaction digest"))?;

        let package_id = event
            .package_id
            .as_ref()
            .ok_or_else(|| anyhow!("Missing package ID"))?;

        let sender = event
            .sender
            .as_ref()
            .ok_or_else(|| anyhow!("Missing sender"))?;

        let bcs_data = event
            .event_data
            .as_ref()
            .ok_or_else(|| anyhow!("Missing event data"))?;

        // Parse the StructTag from string
        let type_tag: StructTag = event
            .type_name
            .parse()
            .map_err(|e| anyhow!("Failed to parse type tag: {e}"))?;

        // Parse the JSON
        let parsed_json: serde_json::Value = serde_json::from_str(&event.parsed_json)
            .map_err(|e| anyhow!("Failed to parse JSON: {e}"))?;

        Ok(IotaEvent {
            id: EventID {
                tx_digest: TransactionDigest::new(
                    tx_digest
                        .digest
                        .clone()
                        .try_into()
                        .map_err(|_| anyhow!("Invalid transaction digest length"))?,
                ),
                event_seq: event_id.event_seq,
            },
            package_id: ObjectID::from_bytes(&package_id.address)
                .map_err(|e| anyhow!("Invalid package ID: {e}"))?,
            transaction_module: Identifier::new(event.transaction_module.clone())
                .map_err(|e| anyhow!("Invalid transaction module: {e}"))?,
            sender: IotaAddress::from_bytes(&sender.address)
                .map_err(|e| anyhow!("Invalid sender address: {e}"))?,
            type_: type_tag,
            parsed_json,
            bcs: BcsEvent::new(bcs_data.data.clone()),
            timestamp_ms: event.timestamp_ms,
        })
    }
}
