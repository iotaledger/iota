# iota-json-rpc

The `iota-json-rpc` library crate provides a flexible framework for building JSON-RPC servers to expose IOTA APIs.
It serves requests over HTTP, and is primarily used to spawn a nested service in the `iota-indexer`
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
// Create a new `JsonRpcServerBuilder` with version information and a Prometheus registry. The last
// two arguments are an optional traffic controller and its policy.
let mut builder = JsonRpcServerBuilder::new(
    env!("CARGO_PKG_VERSION"),
    prometheus::default_registry(),
    None,
    None,
);

// Register the RPC modules that should be included in the server. Here, `reader` is the indexer's
// `IndexerReader`.
builder.register_module(TransactionBuilderApi::from(reader.clone()))?;
builder.register_module(GovernanceReadApi::new(reader.clone()))?;
builder.register_module(CoinReadApi::new(reader.clone())?)?;

// The address the server listens on.
let listen_address = SocketAddr::new("0.0.0.0".parse().unwrap(), 9000);

// Run the server on a specific Tokio runtime instead of the current one, if needed.
let custom_runtime: Option<tokio::runtime::Handle> = None;

// Cancelling this token shuts the server down gracefully. The server also cancels it when it
// stops for another reason. Pass `None` to run the server until the process exits.
let cancel = CancellationToken::new();

// Start the server.
let handle = builder
    .start(listen_address, custom_runtime, Some(cancel.clone()))
    .await?;
```
