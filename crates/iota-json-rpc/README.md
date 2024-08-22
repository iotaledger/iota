# iota-json-rpc

The `iota-json-rpc` crate provides a flexible framework for building JSON-RPC servers.
It supports serving RPC modules over HTTP and WebSocket, primarily for integrating with crates such as `iota-indexer` or `iota-node`.

The `iota-json-rpc` crate provides implementation for various RPC module traits that are defined in the `iota-json-rpc-api` crate (e.g., `IndexerApi`, `ReadApi`, `MoveUtilsApi`).
These modules are not automatically registered and can be explicitly included based on the server's requirements.

Furthermore, the `iota-json-rpc` crate generates OpenRPC documentation automatically for all registered modules and provides prometheus metrics for tracking request times and counts.

## Usage

The example below demonstrates how to use the `iota-json-rpc` crate to build a JSON-RPC server:

```rust
// Create a new `JsonRpcServerBuilder` with version information and a Prometheus registry
let mut builder = JsonRpcServerBuilder::new(env!("CARGO_PKG_VERSION"), prometheus::default_registry());

// Register the RPC modules that should be included in the server.
let reader = IndexerReader::new(env!("DATABASE_CONNECTION_URL"))?;
builder.register_module(TransactionBuilderApi::new(reader.clone()))?;
builder.register_module(GovernanceReadApi::new(reader.clone()))?;
builder.register_module(CoinReadApi::new(reader.clone()))?;

// Define the default socket address for the server, parsing the IP address correctly.
let default_socket_addr: SocketAddr = SocketAddr::new(
    "0.0.0.0".parse().unwrap(),  // Correctly parse the IP address
    9000,
);

// Specify a custom runtime for the server, if needed.
let custom_runtime: Option<tokio::runtime::Handle> = None;

// Start the server.
builder
.start(default_socket_addr, custom_runtime, Some(ServerType::Http))
.await?;
```