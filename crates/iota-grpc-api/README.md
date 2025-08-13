# IOTA gRPC API

> **⚠️ EXPERIMENTAL - INTERNAL USE ONLY**
>
> This gRPC API is highly experimental and intended for internal use only. The API surface, data formats, and behavior are subject to significant changes without notice. **Do not use this in production or build external integrations against it** as breaking changes are expected and likely.

This crate introduces a gRPC API for IOTA. The primary goal of this API is to provide a more efficient and lower-latency method for data access, intended to replace existing REST-API polling or filesystem-based synchronization. This reduces the delay between data creation and their subsequent processing by external services.

## Features

The `NodeService` provides the following RPC endpoints:

- `StreamCheckpoints`: Stream checkpoint data based on a flexible range.
- `GetEpochFirstCheckpointSequenceNumber`: Query the first checkpoint sequence number for a given epoch (useful for robust reset and epoch boundary handling).

## Usage

The `iota-grpc-api` crate defines the gRPC service and its messages. The `iota-node` crate integrates and starts this gRPC server if `enable-grpc-api` is set to `true` and `grpc-api-config` is configured.

A shared gRPC client (`GrpcNodeClient`) is provided by this crate and should be used by downstream consumers to connect and stream checkpoints. This ensures all consumers use the same, up-to-date protocol and data model.

**Configuration Example:**

```toml
# In your node config file (e.g., fullnode.yaml)
enable-grpc-api: true
grpc-api-config:
  address: "0.0.0.0:50051"
  checkpoint-broadcast-buffer-size: 100
```

**Client Example:**

```rust
use iota_grpc_api::client::GrpcNodeClient;

let mut client = GrpcNodeClient::connect("http://localhost:50051").await?;
let mut stream = client.stream_checkpoints(Some(0), Some(10), Some(false)).await?;
while let Some(Ok(checkpoint)) = stream.next().await {
    // Deserialize and process checkpoint.data (BCS-encoded CertifiedCheckpointSummary)
}
let mut stream = client.stream_checkpoints(None, Some(4), Some(true)).await?;
if let Some(Ok(checkpoint)) = stream.next().await {
    // Deserialize as CheckpointData
}
let mut stream = client.stream_checkpoints(Some(5), None, Some(true)).await?;
while let Some(Ok(checkpoint)) = stream.next().await {
    // checkpoint.data is BCS-encoded CheckpointData
}
```
