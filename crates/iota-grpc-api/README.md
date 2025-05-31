# IOTA Checkpoint gRPC API (Proof of Concept)

This crate introduces a proof-of-concept (PoC) gRPC API for streaming IOTA checkpoints. The primary goal of this API is to provide a more efficient and lower-latency method for fetching checkpoints, specifically intended to replace the existing REST-API polling or filesystem-based synchronization used by the indexer and data ingestion services. This reduces the delay between checkpoint creation and their subsequent indexing or processing.

The gRPC API supports subscriptions, similar to the `INX` (IOTA Node Extension) component in Hornet, allowing clients to receive new checkpoints as they are confirmed ([reference](https://github.com/iotaledger/hornet/blob/3ab964191f30ec70f4d54dc121ea01bc48497bc1/components/inx/server_milestones.go#L169)).

## Features

The `CheckpointService` provides a `StreamCheckpoints` RPC endpoint, allowing clients to stream checkpoint data based on various criteria.

The `StreamRequest` message allows for flexible specification of the desired checkpoint range:

```protobuf
message StreamRequest {
  optional uint64 start_index = 1;
  optional uint64 end_index = 2;
}
```

The `StreamCheckpoints` method handles the following scenarios:

1. **Only `start_index` provided**: The service will stream all available checkpoints starting from `start_index` up to the latest confirmed checkpoint.
2. **Only `end_index` provided**: The service will stream only the checkpoint at the specified `end_index`.
3. **Both `start_index` and `end_index` provided**: The service will stream checkpoints within the specified range, from `start_index` up to `min(latest_checkpoint_index, end_index)`.
4. **Neither `start_index` nor `end_index` provided**: The service will stream all available checkpoints, starting from checkpoint index `0` up to the latest confirmed checkpoint.

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

## Usage

The `iota-grpc-api` crate defines the gRPC service and its messages. The `iota-node` crate integrates and starts this gRPC server if a `grpc_api_address` is configured.

A shared gRPC client (`GrpcNodeClient`) is provided by this crate and should be used by downstream consumers (e.g., `iota-indexer`, `iota-data-ingestion`) to connect and stream checkpoints. This ensures all consumers use the same, up-to-date protocol and data model.

**Example:**

```rust
use iota_grpc_api::client::GrpcNodeClient;

let mut client = GrpcNodeClient::connect("http://localhost:50051").await?;
let mut stream = client.stream_checkpoints(0, Some(10)).await?;
while let Some(Ok(checkpoint)) = stream.next().await {
    // Deserialize and process checkpoint.data (BCS-encoded CheckpointData)
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
  - **`test_start_index_only`**: Verifies that when only `start_index` is provided, the stream begins from the specified starting point and retrieves all subsequent checkpoints up to the latest available.
  - **`test_start_and_end_index`**: Validates that when both `start_index` and `end_index` are provided, the streamed checkpoints are precisely within the inclusive range defined by both bounds.
  - **`test_end_index_only`**: Focuses on the scenario where only `end_index` is specified, confirming that only the checkpoint at the specified `end_index` is streamed.

- **`checkpoint_e2e.rs`**
  - **`e2e_stream_checkpoints`**: An end-to-end test that covers the scenario where neither `start_index` nor `end_index` is provided, verifying that all available checkpoints from genesis are streamed.

### **How to Run the Tests**

- **Run all tests for the crate:**
  ```bash
  cargo test -p iota-grpc-api -- --nocapture --test-threads=1
  ```


These tests ensure that the gRPC streaming API behaves as expected for all supported request patterns and edge cases. All downstream consumers are encouraged to run these tests when upgrading or integrating the gRPC API.

## Downstream Integration Tests: gRPC Checkpoint Streaming

The following integration tests have been added in downstream crates to ensure that the gRPC checkpoint streaming API works as expected for real consumers:

### **iota-data-ingestion**

- **`tests/grpc_blob_ingestion.rs`**
  - **`test_grpc_blob_ingestion`**: Spins up a test cluster with a gRPC-enabled node, streams checkpoints using the shared gRPC client, decodes and processes the streamed checkpoint data, and verifies successful ingestion. This test ensures the gRPC ingestion path is fully functional and aligned with the REST path for checkpoint data.

  **How to run:**
  ```bash
  cargo test -p iota-data-ingestion --test grpc_blob_ingestion -- --nocapture
  ```

### **iota-indexer**

- **`tests/grpc_ingestion.rs`**
  - **`test_grpc_checkpoint_stream`**: Spins up a test cluster with a gRPC-enabled node, streams checkpoints using the shared gRPC client, and verifies that at least one checkpoint is received. This test ensures the indexer can connect to a gRPC node and stream checkpoints as expected.

  **How to run:**
  ```bash
  cargo test -p iota-indexer --test grpc_ingestion -- --nocapture
  ```

These tests are recommended for all downstream consumers and contributors to verify that gRPC checkpoint streaming works as expected in real-world integration scenarios.
