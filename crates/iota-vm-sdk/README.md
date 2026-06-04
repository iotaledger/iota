# iota-vm-sdk

Run and inspect IOTA transactions against the same Move execution engine a full
node uses — locally, with no network connection.

It is built around a four-part surface:

- **Decode** — read what a transaction or signature references (objects,
  `MoveAuthenticator`, dynamic-field IDs) without a VM or a store.
- **Store** — hold the objects a run needs (`InMemoryStore`, or pre-fetch from a
  node via the optional `grpc` / `graphql` stores).
- **Execute** — run a transaction through `LocalVm` in one of three modes:
  dev-inspect, dry-run, or execute (which commits effects back to the store).
- **Introspect** — read the effects, events, gas, and signature status of a run,
  plus optional gas-profile and instruction-trace debug artifacts.

Typical uses: simulating a transaction before signing, estimating gas,
debugging a Move call, or verifying a `MoveAuthenticator` — in a CLI or a test.

## Features

All features are off by default:

- `grpc` — pre-fetch objects from a node over gRPC.
- `graphql` — pre-fetch objects from a node over GraphQL.
