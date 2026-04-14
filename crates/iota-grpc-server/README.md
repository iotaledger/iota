# IOTA gRPC API

This crate implements a gRPC API for IOTA. Clients can subscribe to real-time data streams, enabling low-latency access to on-chain data as it is produced.

## Field Masks

Many endpoints support [field masks](https://protobuf.dev/reference/protobuf/google.protobuf/#field-mask) via an optional `read_mask` parameter. A field mask lets clients specify exactly which fields to include in the response, reducing bandwidth and processing overhead. When no field mask is provided, all fields are returned by default.

Endpoints that support field masks are marked with **[FM]** below.

## Features

The gRPC API provides the following services:

### Ledger Service

- `GetHealth`: Check node health with optional latency threshold.
- `GetServiceInfo`: Query service state (chain ID, epoch, checkpoint height, etc.).
- `GetObjects` **[FM]**: Stream objects by reference.
- `GetTransactions` **[FM]**: Stream transactions by digest.
- `GetCheckpoint` **[FM]**: Stream checkpoint data by sequence number, digest, or latest, with transaction and event filtering.
- `StreamCheckpoints` **[FM]**: Stream checkpoints with filtering and progress reporting.
- `GetEpoch` **[FM]**: Query epoch information.

### Transaction Execution Service

- `ExecuteTransactions` **[FM]**: Execute a batch of transactions sequentially, with per-item error handling and configurable checkpoint inclusion waiting.
- `SimulateTransactions` **[FM]**: Simulate a batch of transactions (with suggested gas price), with per-item error handling and configurable checkpoint inclusion waiting.

### State Service

- `ListDynamicFields` **[FM]**: List dynamic fields owned by a parent object with pagination.
- `ListOwnedObjects` **[FM]**: List objects owned by an address with optional type filtering and pagination.
- `GetCoinInfo`: Get coin metadata, treasury cap, and regulated coin metadata.

### Move Package Service

- `ListPackageVersions`: List all versions of a Move package with pagination.

## Usage

The `iota-grpc-server` crate implements the gRPC services. The `iota-node` crate integrates and starts this gRPC server if `enable-grpc-api` is set to `true` and `grpc-api-config` is configured.

Shared gRPC clients are provided by the [`iota-sdk-grpc-client`](https://github.com/iotaledger/iota-rust-sdk/tree/develop/crates/iota-sdk-grpc-client) crate in the [iota-rust-sdk](https://github.com/iotaledger/iota-rust-sdk):

- `Client`: Factory for creating service-specific clients (`ledger_service_client()`, `execution_service_client()`, `state_service_client()`, `move_package_service_client()`).

These clients should be used by downstream consumers to ensure all consumers use the same, up-to-date protocol and data model.

**Configuration Example:**

```yaml
# In your node config file (e.g., fullnode.yaml)
enable-grpc-api: true
grpc-api-config:
  address: "0.0.0.0:50051"
  broadcast-buffer-size: 100
  max-message-size-bytes: 134217728
  max-json-move-value-size: 1048576
  max-execute-transaction-batch-size: 20
  max-simulate-transaction-batch-size: 20
  max-checkpoint-inclusion-timeout-ms: 60000
```

**Client Example:**

```rust
use iota_sdk_grpc_client::Client;

// Connect to gRPC node
let client = Client::connect("http://localhost:50051").await?;

// Get a service-specific client
let mut ledger = client.ledger_service_client();
let mut execution = client.execution_service_client();
let mut state = client.state_service_client();
let mut packages = client.move_package_service_client();
```

Proto definitions are in the [`iota-sdk-grpc-types`](https://github.com/iotaledger/iota-rust-sdk/tree/develop/crates/iota-sdk-grpc-types/proto/iota/grpc/v1) crate in the [iota-rust-sdk](https://github.com/iotaledger/iota-rust-sdk).
