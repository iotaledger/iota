# IOTA gRPC API

> **⚠️ EXPERIMENTAL - INTERNAL USE ONLY**
>
> This gRPC API is highly experimental and intended for internal use only. The API surface, data formats, and behavior are subject to significant changes without notice. **Do not use this in production or build external integrations against it** as breaking changes are expected and likely.

This crate introduces a gRPC API for IOTA. The primary goal of this API is to provide a more efficient and lower-latency method for data access, intended to replace existing REST-API polling or filesystem-based synchronization. This reduces the delay between data creation and their subsequent processing by external services.

## Features

The gRPC API provides the following services:

### Checkpoint Service

- `StreamCheckpoints`: Stream checkpoint data based on a flexible range.
- `GetEpochFirstCheckpointSequenceNumber`: Query the first checkpoint sequence number for a given epoch (useful for robust reset and epoch boundary handling).

### Event Service

- `StreamEvents`: Stream events with flexible filtering capabilities

Event filters allow precise control over which events are streamed to clients, including filtering by event type, package, sender, transaction, field values, time ranges, and boolean combinations. For the complete list and definitions of available filters, see [`proto/event.proto`](proto/event.proto).

## Usage

The `iota-grpc-server` crate defines the gRPC service and its messages. The `iota-node` crate integrates and starts this gRPC server if `enable-grpc-api` is set to `true` and `grpc-api-config` is configured.

Shared gRPC clients are provided by this crate:

- `CheckpointClient`: For streaming checkpoints and querying epoch information
- `EventClient`: For streaming events with filtering capabilities
- `NodeClient`: Factory for creating and managing service clients

These clients should be used by downstream consumers to ensure all consumers use the same, up-to-date protocol and data model.

**Configuration Example:**

```toml
# In your node config file (e.g., fullnode.yaml)
enable-grpc-api: true
grpc-api-config:
  address: "0.0.0.0:50051"
  checkpoint-broadcast-buffer-size: 100
  event-broadcast-buffer-size: 1000
```

**Client Examples:**

```rust
use iota_grpc_client::NodeClient;

// Connect to gRPC node
let node_client = NodeClient::connect("http://localhost:50051").await?;

// Checkpoint streaming example
let mut checkpoint_client = node_client.checkpoint_client().expect("Checkpoint client available");
let mut checkpoint_stream = checkpoint_client.stream_checkpoints(Some(0), Some(10), false).await?;
while let Some(Ok(checkpoint_content)) = checkpoint_stream.next().await {
    // Process checkpoint data or summary
}

// Event streaming example
use iota_grpc_types::v0::events::{EventFilter, AllFilter, event_filter::Filter};

let mut event_client = node_client.event_client().expect("Event client available");
let all_events_filter = EventFilter {
    filter: Some(Filter::All(AllFilter {})),
};
let mut event_stream = event_client.stream_events(all_events_filter).await?;
while let Some(Ok(event)) = event_stream.next().await {
    // Process IotaEvent
}
```
