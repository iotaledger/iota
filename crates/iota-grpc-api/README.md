# IOTA Checkpoint gRPC API (Proof of Concept)

This crate introduces a proof-of-concept (PoC) gRPC API for streaming IOTA checkpoints. The primary goal of this API is to provide a more efficient and lower-latency method for fetching checkpoints, specifically intended to replace the existing REST-API polling or filesystem-based synchronization used by the indexer. This aims to reduce the delay between checkpoint creation and their subsequent indexing.

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

1.  **Only `start_index` provided**: The service will stream all available checkpoints starting from `start_index` up to the latest confirmed checkpoint.
2.  **Only `end_index` provided**: The service will stream only the checkpoint at the specified `end_index`.
3.  **Both `start_index` and `end_index` provided**: The service will stream checkpoints within the specified range, from `start_index` up to `min(latest_checkpoint_index, end_index)`.
4.  **Neither `start_index` nor `end_index` provided**: The service will stream all available checkpoints, starting from checkpoint index `0` up to the latest confirmed checkpoint.

## Usage (PoC)

The `iota-grpc-api` crate defines the gRPC service and its messages. The `iota-node` crate has been updated to integrate and start this gRPC server if a `grpc_api_address` is configured.

**Note**: In this PoC, the `CheckpointGrpcService` currently utilizes `iota_rest_api::stream_checkpoints_public` for data retrieval. In a full implementation, the checkpoint streaming logic would be moved directly into the `iota-grpc-api` crate for optimized performance and reduced dependencies.

## Testing

You can run the tests for the new gRPC API to see detailed results using the following command:

```bash
cargo test -p iota-grpc-api -- --nocapture --test-threads=1
```

This command specifically targets the `iota-grpc-api` crate (`-p iota-grpc-api`), ensures that all test output is captured and displayed (`--nocapture`), and runs the tests sequentially with a single thread (`--test-threads=1`) to avoid potential conflicts or interleaved output, making it easier to review the results.

The tests in `crates/iota-grpc-api/tests/checkpoint_stream.rs` validate the `StreamCheckpoints` functionality across various scenarios:

*   **`test_start_index_only`**: This test case verifies that when *only* `start_index` is provided, the stream correctly begins from the specified starting point and retrieves all subsequent checkpoints up to the latest available.
*   **`test_start_and_end_index`**: This test case validates the behavior when *both* `start_index` and `end_index` are provided. It asserts that the streamed checkpoints are precisely within the inclusive range defined by both bounds.
*   **`test_end_index_only`**: This test case focuses on the scenario where *only* `end_index` is specified. It confirms that in this case, the service streams solely the checkpoint at the specified `end_index`.

Additionally, an end-to-end test in `crates/iota-grpc-api/tests/checkpoint_e2e.rs` (`e2e_stream_checkpoints`) covers the scenario where neither `start_index` nor `end_index` is provided, verifying that all available checkpoints from genesis are streamed.
