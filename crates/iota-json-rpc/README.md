# iota-json-rpc

The `iota-json-rpc` library crate provides a flexible framework for building JSON-RPC servers to expose IOTA APIs.
It supports requests over HTTP and WebSocket, and is primarily to spawn a nested service in the `iota-indexer`
or `iota-node` applications.

It provides module implementations for various JSON-RPC server traits (
e.g. `IndexerApiServer`, `GovernanceReadApiServer`, `MoveUtilsApiServer`) which are derived in the `iota-json-rpc-api`
crate from JSON-RPC API definitions (e.g. `IndexerApi`, `GovernanceReadApi`, `MoveUtilsApi`).

These modules are not automatically registered with the server and can be explicitly included based on the server's
requirements. They are registered for `iota-node` in particular, using `AuthorityState` for the internal state.

Modules can be registered with the server using the `register_module` method, which expects a module struct of
type `IotaRpcModule` trait. The trait provides module information such as a list of supported JSON-RPC methods and OpenRPC documentation.

## Usage

The following example shows how to build, register RPC modules and start the server:

```rust
// Create a new `JsonRpcServerBuilder` with version information and a Prometheus registry
let mut builder = JsonRpcServerBuilder::new(env!("CARGO_PKG_VERSION"), prometheus::default_registry());

// Register the RPC modules that should be included in the server.
let reader = IndexerReader::new(env!("DATABASE_CONNECTION_URL")) ?;
builder.register_module(TransactionBuilderApi::new(reader.clone())) ?;
builder.register_module(GovernanceReadApi::new(reader.clone())) ?;
builder.register_module(CoinReadApi::new(reader.clone())) ?;

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
