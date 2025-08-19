// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_json_rpc_types::{EventFilter, Filter, IotaEvent};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
};
use move_core_types::{identifier::Identifier, language_storage::StructTag};
use serde_json;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};
use tracing::debug;

use crate::{
    bcs_event::try_to_bcs_bytes,
    events::{Event, EventId, EventStreamRequest, event_service_server::EventService},
    types::GrpcEventBroadcaster,
};

pub struct EventGrpcService {
    pub event_broadcaster: GrpcEventBroadcaster,
    pub cancellation_token: CancellationToken,
}

impl EventGrpcService {
    pub fn new(
        event_broadcaster: GrpcEventBroadcaster,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            event_broadcaster,
            cancellation_token,
        }
    }
}

#[tonic::async_trait]
impl EventService for EventGrpcService {
    type StreamEventsStream =
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<Event, Status>> + Send>>;

    async fn stream_events(
        &self,
        request: Request<EventStreamRequest>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        let proto_filter = request
            .into_inner()
            .filter
            .ok_or_else(|| Status::invalid_argument("Filter is required"))?;

        let event_filter = create_event_filter(&proto_filter)?;
        debug!("New gRPC client subscribed with filter: {event_filter:?}");

        let mut event_rx = self.event_broadcaster.subscribe();
        let cancellation_token = self.cancellation_token.clone();

        let stream = async_stream::try_stream! {
            loop {
                let event_result = tokio::select! {
                    recv_result = event_rx.recv() => Some(recv_result),
                    _ = cancellation_token.cancelled() => {
                        debug!("Event stream cancelled");
                        None
                    }
                };

                match event_result {
                    Some(Ok(event_arc)) => {
                        let event = &*event_arc;

                        // Use existing filter matching logic
                        if event_filter.matches(event) {
                            debug!(
                                "Event matched filter: TX: {}, Type: {}, Sender: {}",
                                event.id.tx_digest,
                                event.type_.name.as_ident_str(),
                                event.sender
                            );

                            // Convert to protobuf Event
                            let proto_event = Event::try_from(event)
                                .map_err(|e| Status::internal(format!("Failed to convert event: {e}")))?;

                            yield proto_event;
                        }
                    }
                    Some(Err(_)) => {
                        debug!("Event broadcast channel closed");
                        break;
                    }
                    None => {
                        // Cancellation occurred
                        break;
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }
}

/// Convert protobuf EventFilter to iota_json_rpc_types::EventFilter
pub fn create_event_filter(
    proto_filter: &crate::events::EventFilter,
) -> Result<EventFilter, Status> {
    use crate::events::event_filter::Filter;

    match &proto_filter.filter {
        Some(Filter::MoveEventType(f)) => {
            let struct_tag = StructTag {
                address: *parse_object_id(&f.address)?,
                module: parse_identifier(&f.module, "module name")?,
                name: parse_identifier(&f.name, "event name")?,
                type_params: vec![],
            };
            Ok(EventFilter::MoveEventType(struct_tag))
        }
        Some(Filter::MoveEventField(f)) => Ok(EventFilter::MoveEventField {
            path: f.path.clone(),
            value: serde_json::Value::String(f.value.clone()),
        }),
        Some(Filter::Package(f)) => Ok(EventFilter::Package(parse_object_id(&f.package_id)?)),
        Some(Filter::MoveEventModule(f)) => Ok(EventFilter::MoveEventModule {
            package: parse_object_id(&f.package_id)?,
            module: parse_identifier(&f.module, "module name")?,
        }),
        Some(Filter::And(f)) => {
            let filters = parse_filter_list(&f.filters)?;
            build_and_filter(filters)
        }
        Some(Filter::Or(f)) => {
            let filters = parse_filter_list(&f.filters)?;
            build_or_filter(filters)
        }
        Some(Filter::All(_)) => Ok(EventFilter::All(vec![])),
        Some(Filter::Sender(f)) => Ok(EventFilter::Sender(parse_address(&f.sender)?)),
        Some(Filter::Transaction(f)) => {
            Ok(EventFilter::Transaction(parse_tx_digest(&f.tx_digest)?))
        }
        Some(Filter::MoveModule(f)) => Ok(EventFilter::MoveModule {
            package: parse_object_id(&f.package_id)?,
            module: parse_identifier(&f.module, "module name")?,
        }),
        Some(Filter::TimeRange(f)) => Ok(EventFilter::TimeRange {
            start_time: f.start_time,
            end_time: f.end_time,
        }),
        None => Ok(EventFilter::All(vec![])),
    }
}

// Helper functions to reduce repetition and improve error messages
fn parse_object_id(hex_str: &str) -> Result<ObjectID, Status> {
    ObjectID::from_hex_literal(hex_str)
        .map_err(|e| Status::invalid_argument(format!("Invalid object ID '{hex_str}': {e}")))
}

fn parse_identifier(id_str: &str, field_name: &str) -> Result<Identifier, Status> {
    Identifier::from_str(id_str)
        .map_err(|e| Status::invalid_argument(format!("Invalid {field_name} '{id_str}': {e}")))
}

fn parse_address(addr_str: &str) -> Result<IotaAddress, Status> {
    IotaAddress::from_str(addr_str)
        .map_err(|e| Status::invalid_argument(format!("Invalid address '{addr_str}': {e}")))
}

fn parse_tx_digest(digest_str: &str) -> Result<TransactionDigest, Status> {
    TransactionDigest::from_str(digest_str).map_err(|e| {
        Status::invalid_argument(format!("Invalid transaction digest '{digest_str}': {e}"))
    })
}

fn parse_filter_list(filters: &[crate::events::EventFilter]) -> Result<Vec<EventFilter>, Status> {
    filters.iter().map(create_event_filter).collect()
}

/// Generic function to build chained filters to ease of implementing other
/// logics in the future
fn build_chained_filter<F>(
    mut filters: Vec<EventFilter>,
    filter_type: &str,
    constructor: F,
) -> Result<EventFilter, Status>
where
    F: Fn(Box<EventFilter>, Box<EventFilter>) -> EventFilter,
{
    if filters.is_empty() {
        return Err(Status::invalid_argument(format!(
            "{filter_type} filter cannot be empty",
        )));
    }

    let mut result = filters.pop().unwrap();
    while let Some(filter) = filters.pop() {
        result = constructor(Box::new(filter), Box::new(result));
    }
    Ok(result)
}

/// Build AND filter by chaining EventFilter::And
fn build_and_filter(filters: Vec<EventFilter>) -> Result<EventFilter, Status> {
    build_chained_filter(filters, "AND", EventFilter::And)
}

/// Build OR filter by chaining EventFilter::Or
fn build_or_filter(filters: Vec<EventFilter>) -> Result<EventFilter, Status> {
    build_chained_filter(filters, "OR", EventFilter::Or)
}

// Convert IotaEvent to protobuf Event
impl TryFrom<&IotaEvent> for Event {
    type Error = anyhow::Error;

    fn try_from(event: &IotaEvent) -> Result<Self, Self::Error> {
        Ok(Event {
            event_data: try_to_bcs_bytes(event)?,
            event_id: Some(EventId {
                tx_seq: event.id.event_seq,
                event_seq: event.id.event_seq,
                tx_digest: event.id.tx_digest.to_string(),
            }),
            timestamp_ms: event.timestamp_ms,
        })
    }
}
