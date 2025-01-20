# iota-tool

`iota-tool` contains assorted debugging utilities for IOTA.

You can build and run `iota-tool` from source with:

```sh
cargo run --bin iota-tool -- <args>
```

## `anemo` tools

You can use the anemo CLI tools to ping or call an RPC on an Anemo server. Note that (for now) this uses randomly generated keys, so a server or method that restricts access to allowlisted peers will reject connections from this tool.

Anemo networks are identified by a "server name" that the client must match. Server names you may want to use:

- IOTA discovery and state sync: `iota`

### ping

Example command to ping an anemo server:

```sh
SERVER_NAME="iota"; \
ADDRESS="1.2.3.4:5678"; \
cargo run --bin iota-tool -- anemo ping --server-name "$SERVER_NAME" "$ADDRESS"
```

### call

`iota-tool` has been preconfigured to support RPC calls using [RON (Rusty Object Notation)](https://crates.io/crates/ron) for the following servivces:

- IOTA: `Discovery` and `StateSync`

Example command to send an RPC:

```sh
SERVER_NAME="iota"; \
ADDRESS="1.2.3.4:5678"; \
SERVICE_NAME="StateSync"; \
METHOD_NAME="GetCheckpointSummary"; \
REQUEST="BySequenceNumber(123)"; \
cargo run --bin iota-tool -- \
    anemo call --server-name "$SERVER_NAME" "$ADDRESS" "$SERVICE_NAME" "$METHOD_NAME" "$REQUEST"
```
