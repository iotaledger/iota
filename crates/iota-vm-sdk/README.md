# iota-vm-sdk

Run and inspect IOTA transactions against the same Move execution engine a full
node uses — locally, with no network connection.

It is built around a three-part surface:

- **Store** — hold the objects a run needs (`InMemoryStore`, or resolve them on
  demand from a node via the optional `grpc` / `graphql` stores).
- **Execute** — run a transaction through `LocalVm` in one of three modes:
  dev-inspect, dry-run, or execute (which commits effects back to the store).
- **Inspect** — read the effects, events, gas, and signature status of a run,
  plus optional gas-profile and instruction-trace debug artifacts.

Typical uses: simulating a transaction before signing, estimating gas,
debugging a Move call, or verifying a `MoveAuthenticator` — in a CLI or a test.

Signed runs report a `SignatureStatus` next to the execution status: standard
schemes are verified cryptographically up front, a `MoveAuthenticator` by
running its function in the VM — so a failing transaction body never shows up
as a signature failure.

Object references are resolved against the versions the store holds, so a
simulation keeps working with objects fetched at "latest" — but a stale
version or digest that a node would reject at signing time is not detected.
Networked stores fetch dynamic-field children at "latest" too, so historical
replay against a pinned older version may report such a child as missing.

How each entry point maps to the node's execution phases and gas budgets is
documented in [docs/execution-model.md](docs/execution-model.md).

## Features

All features are off by default:

- `grpc` — resolve objects on demand from a node over gRPC.
- `graphql` — resolve objects on demand from an indexer over GraphQL.
- `tracing` — compile the Move VM gas profiler and instruction tracer into the
  engine so the gas-profile and instruction-trace debug artifacts are captured.
