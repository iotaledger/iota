# IOTA gRPC API

This crate implements a gRPC API for IOTA. The primary goal of this API is to provide a more efficient and lower-latency method for data access, intended to replace existing REST-API polling or filesystem-based synchronization. This reduces the delay between data creation and their subsequent processing by external services.

## Features

The gRPC API provides the following services:

### Ledger Service

- `GetHealth`: Check node health with optional latency threshold.
- `GetServiceInfo`: Query service state (chain ID, epoch, checkpoint height, etc.).
- `GetObjects`: Stream objects by reference with field mask support.
- `GetTransactions`: Stream transactions by digest with field mask support.
- `GetCheckpoint`: Stream checkpoint data by sequence number, digest, or latest, with transaction and event filtering.
- `StreamCheckpoints`: Stream checkpoints with filtering and progress reporting.
- `GetEpoch`: Query epoch information.

### Transaction Execution Service

- `ExecuteTransactions`: Execute a batch of transactions sequentially.
- `SimulateTransactions`: Simulate a batch of transactions (with suggested gas price).

Both support field masks to control response data, per-item error handling, and configurable checkpoint inclusion waiting.

### State Service

- `ListDynamicFields`: List dynamic fields owned by a parent object with pagination.
- `ListOwnedObjects`: List objects owned by an address with optional type filtering and pagination.
- `GetCoinInfo`: Get coin metadata, treasury cap, and regulated coin metadata.

### Move Package Service

- `ListPackageVersions`: List all versions of a Move package with pagination.

## Usage

The `iota-grpc-server` crate implements the gRPC services. The `iota-node` crate integrates and starts this gRPC server if `enable-grpc-api` is set to `true` and `grpc-api-config` is configured.

Shared gRPC clients are provided by the `iota-grpc-client` crate:

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
use iota_grpc_client::Client;

// Connect to gRPC node
let client = Client::connect("http://localhost:50051").await?;

// Get a service-specific client
let mut ledger = client.ledger_service_client();
let mut execution = client.execution_service_client();
let mut state = client.state_service_client();
let mut packages = client.move_package_service_client();
```

Proto definitions are in the `iota-grpc-types` crate at `proto/iota/grpc/v1/`.
