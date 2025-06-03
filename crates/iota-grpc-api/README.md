# IOTA Checkpoint gRPC API (Proof of Concept)

This crate introduces a proof-of-concept (PoC) gRPC API for streaming IOTA checkpoints. The primary goal of this API is to provide a more efficient and lower-latency method for fetching checkpoints, specifically intended to replace the existing REST-API polling or filesystem-based synchronization used by the indexer and data ingestion services. This reduces the delay between checkpoint creation and their subsequent indexing or processing.

The gRPC API supports subscriptions, similar to the `INX` (IOTA Node Extension) component in Hornet, allowing clients to receive new checkpoints as they are confirmed ([reference](https://github.com/iotaledger/hornet/blob/3ab964191f30ec70f4d54dc121ea01bc48497bc1/components/inx/server_milestones.go#L169)).

## Features

The `CheckpointService` provides the following RPC endpoints:

- `StreamCheckpoints`: Stream checkpoint data based on a flexible range.
- `GetEpochFirstCheckpointSequenceNumber`: Query the first checkpoint sequence number for a given epoch (useful for robust reset and epoch boundary handling).

### Proto

```protobuf
service CheckpointService {
  rpc StreamCheckpoints (StreamRequest) returns (stream Checkpoint);
  rpc GetEpochFirstCheckpointSequenceNumber (EpochRequest) returns (CheckpointSequenceNumberResponse);
}

message StreamRequest {
  optional uint64 start_index = 1;
  optional uint64 end_index = 2;
  optional bool full = 3;
}

message EpochRequest {
  uint64 epoch = 1;
}

message CheckpointSequenceNumberResponse {
  uint64 sequence_number = 1;
}

message Checkpoint {
  uint64 index = 1;
  bytes data = 2;
}
```

### Streaming Range Logic

For all cases, the `full` flag determines the data type:
- If `full=false` (default): streams `CertifiedCheckpointSummary` (BCS-encoded in bytes field)
- If `full=true`: streams `CheckpointData` (BCS-encoded in bytes field)

The four supported range patterns:

- **Both `start_index` and `end_index` omitted:**
  - Streams the latest checkpoint and keeps streaming new ones as they arrive.
- **Only `start_index` provided:**
  - Streams from `start_index` and keeps streaming new ones as they arrive.
- **Only `end_index` provided:**
  - Streams only the checkpoint at `end_index`.
- **Both `start_index` and `end_index` provided:**
  - Streams from `start_index` to `end_index` (inclusive).

The service does not attempt to compute a "latest" checkpoint index, making it robust to on-the-fly checkpoint generation.

## REST vs. gRPC Checkpoint Streaming: Comparison

| Aspect               | REST API Path                                    | gRPC API Path                                    | Alignment Status          |
| -------------------- | ------------------------------------------------ | ------------------------------------------------ | ------------------------- |
| **Purpose**          | Fetch full checkpoint data via HTTP              | Stream full checkpoint data via gRPC             | Aligned (for checkpoints) |
| **Data Model**       | `CheckpointData` (BCS-encoded)                   | `CheckpointData` (BCS-encoded in bytes field)    | Aligned                   |
| **Worker Interface** | Implements `Worker` trait (`process_checkpoint`) | Implements `Worker` trait (`process_checkpoint`) | Aligned                   |
| **Client Location**  | Inline HTTP client in worker                     | Shared gRPC client in `iota-grpc-api`            | Aligned (modular)         |
| **Test Coverage**    | Integration tests with REST node                 | Integration tests with gRPC node                 | Aligned                   |
| **Scope**            | Can fetch any checkpoint, full or summary        | **Only streams checkpoints**                     | Aligned (by requirement)  |
| **Extensibility**    | Can add more REST endpoints if needed            | Only checkpoint streaming is implemented         | Aligned (by requirement)  |

## Visual Comparison

```mermaid
flowchart LR
    subgraph REST_API
        A1["Indexer"] -- "HTTP GET /api/v1/checkpoints/{checkpoint}/full" --> B1["Node REST API"]
        B1 -- "fetches from" --> C1["Node State"]
        B1 -- "returns checkpoint data" --> A1
    end
    subgraph gRPC
        A2["Indexer"] -- "gRPC connect" --> B2["Node gRPC API"]
        B2 -- "streams checkpoints" --> A2
        B2 -- "fetches from" --> C2["Node State"]
    end
```

## Key Differences

| Aspect               | REST API Flow                           | gRPC Flow                                           |
| -------------------- | --------------------------------------- | --------------------------------------------------- |
| **Server**           | Node REST API                           | Node gRPC API                                       |
| **Client**           | Indexer (HTTP client)                   | Indexer (gRPC client)                               |
| **Data Transfer**    | Polling (pull)                          | Streaming (push)                                    |
| **Protocol**         | HTTP/1.1 or HTTP/2, JSON/BCS            | HTTP/2, Protocol Buffers (protobuf)                 |
| **Efficiency**       | Higher latency (polling interval)       | Lower latency (real-time streaming)                 |
| **Setup**            | `enable_rest_api = true` in node config | `grpc_api_address` set in node config               |
| **Integration Test** | Yes (REST tests)                        | Yes (`grpc_ingestion.rs`, `grpc_blob_ingestion.rs`) |

## In summary

- **REST API:** Indexer pulls checkpoints from the node by polling HTTP endpoints.
- **gRPC API:** Indexer receives checkpoints as a real-time stream from the node.

> **Note:**
> The gRPC API now provides an endpoint for querying the first checkpoint of a given epoch (`GetEpochFirstCheckpointSequenceNumber`), making robust reset and epoch boundary handling possible for clients. Handling epoch boundaries or resets can be implemented by the client by inspecting the streamed checkpoint data or by using this endpoint.

## Usage

The `iota-grpc-api` crate defines the gRPC service and its messages. The `iota-node` crate integrates and starts this gRPC server if a `grpc_api_address` is configured.

A shared gRPC client (`GrpcNodeClient`) is provided by this crate and should be used by downstream consumers (e.g., `iota-indexer`, `iota-data-ingestion`) to connect and stream checkpoints. This ensures all consumers use the same, up-to-date protocol and data model.

**Example:**

```rust
use iota_grpc_api::client::GrpcNodeClient;

let mut client = GrpcNodeClient::connect("http://localhost:50051").await?;
let mut stream = client.stream_checkpoints(0, Some(10), Some(false)).await?;
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

## Testing

You can run the tests for the new gRPC API to see detailed results using the following command:

```bash
cargo test -p iota-grpc-api -- --nocapture --test-threads=1
```

This command specifically targets the `iota-grpc-api` crate (`-p iota-grpc-api`), ensures that all test output is captured and displayed (`--nocapture`), and runs the tests sequentially with a single thread (`--test-threads=1`) to avoid potential conflicts or interleaved output, making it easier to review the results.

## gRPC Checkpoint Streaming: Test Suite

The following tests have been added to ensure the correctness and robustness of the gRPC checkpoint streaming API:

### **Integration Tests**

Located in `crates/iota-grpc-api/tests/`:

- **`checkpoint_stream.rs`**
  - **`test_start_index_only`**: Streams all available checkpoints starting from the specified `start_index` (5). The test collects checkpoints from 5 up to 15, covering both buffered and live-streamed checkpoints, and then ends.
  - **`test_start_and_end_index`**: Streams checkpoints within the inclusive range defined by `start_index` (3) and `end_index` (7). The test collects checkpoints `[3, 4, 5, 6, 7]` and then ends, ensuring no live checkpoints are collected beyond the end index.
  - **`test_end_index_only`**: Streams only the checkpoint at the specified `end_index` (4). The test collects `[4]` and then ends.
  - **`test_both_indices_omitted`**: Streams all available buffered checkpoints (0..=10) and then continues to collect live checkpoints as they are produced, up to index 24. The test collects checkpoints `[10, 11, 12, ..., 24]` to verify both buffered and live streaming.

- **`checkpoint_e2e.rs`**
  - **`e2e_stream_checkpoints`**: End-to-end test that connects to a real node and streams checkpoints from the gRPC API with both indices omitted. The test collects the first two checkpoints (e.g., genesis and the next one) to verify that streaming works and new checkpoints are delivered in real time.
  - **`test_get_epoch_first_checkpoint_sequence_number`**: End-to-end test that streams all checkpoints from the node and verifies the epoch for each. It also tests the gRPC endpoint for querying the first checkpoint of a given epoch, ensuring that the correct sequence number is returned for both epoch 0 and epoch 1.

### **How to Run the Tests**

- **Run all tests for the crate:**
  ```bash
  cargo test -p iota-grpc-api -- --nocapture --test-threads=1
  ```

These tests ensure that the gRPC streaming API behaves as expected for all supported request patterns and edge cases, including epoch boundary and reset handling. All downstream consumers are encouraged to run these tests when upgrading or integrating the gRPC API.

## Downstream Integration Tests: gRPC Checkpoint Streaming

The following integration tests have been added in downstream crates to ensure that the gRPC checkpoint streaming API works as expected for real consumers:

### **iota-data-ingestion**

- **`tests/grpc_blob_ingestion.rs`**
  - **`test_grpc_blob_worker_logic`**: Streams the full CheckpointData for a single checkpoint (using full=true, start_index=None, end_index=Some(idx)), decodes it, and passes it to the GrpcBlobWorker to verify ingestion logic. This test ensures the worker can process a full checkpoint streamed via gRPC.

  **How to run:**
  ```bash
  cargo test -p iota-data-ingestion --test grpc_blob_ingestion -- --nocapture
  ```

### **iota-indexer**

- **`tests/grpc_ingestion.rs`**
  - **`test_grpc_checkpoint_ingestion`**: This test starts a test cluster with gRPC enabled, launches the indexer with gRPC ingestion, and waits for the indexer to process at least six checkpoints. It verifies that the indexer can successfully connect to the gRPC service and stream both previously produced (buffered) and newly created checkpoints in real time. The test ensures the end-to-end gRPC ingestion path is working, robust to cluster startup, and that the indexer can handle both historical and live checkpoint streaming scenarios.

  **How to run:**
  ```bash
  cargo test -p iota-indexer --test grpc_ingestion -- --nocapture
  ```

This test provides confidence that the indexer is correctly integrated with the gRPC API and can process checkpoints as soon as they are available from the node, whether they are old or new.

These tests are recommended for all downstream consumers and contributors to verify that gRPC checkpoint streaming works as expected in real-world integration scenarios.
