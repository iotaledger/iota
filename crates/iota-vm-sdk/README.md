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

A signed run reports two separate results:

- the **execution status** — did the transaction succeed or abort;
- the **`SignatureStatus`** — did its signatures pass.

Keeping them apart lets you tell a genuinely bad signature from a transaction
whose signatures were fine but whose body failed — the latter still reports
`Verified`.

How a signature is checked depends on its type:

- standard schemes (e.g. ed25519) are verified cryptographically before the
  transaction runs;
- a `MoveAuthenticator` is a programmable authenticator, checked by running its
  Move function in the VM — so its result comes out of execution itself.

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
