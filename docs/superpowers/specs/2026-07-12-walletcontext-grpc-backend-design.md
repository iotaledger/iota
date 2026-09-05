# WalletContext gRPC backend — design (real P1)

**Date:** 2026-07-12
**Status:** Design
**Repo:** iota_2
**Base branch:** `origin/feat/12097-grpc-wallet-context` (PR #12197)
**Supersedes:** the earlier `…-p1-walletcontext-grpc-backend-design.md` (written before this
session's findings; kept for history on the specs branch).

## Purpose

Make `WalletContext`'s own chain operations run over the node's **gRPC** API instead of
JSON-RPC, so every consumer (the ~28 wallet-driven e2e tests, and the CLI) can drive the chain
without the node serving JSON-RPC. `WalletContext` moves to **`iota-rust-sdk` (SDK-native)
return types**; the gRPC path produces them natively, and a JSON-RPC fallback converts into them.
Callers that consumed the old `iota_json_rpc_types` returns (~22 execute-consumers + 5
gas-object consumers + the CLI's transaction output) are updated to the SDK-native shapes.

## Foundation already in place (#12197)

`crates/iota-sdk/src/wallet_context.rs` on the base branch already has:

- `grpc_client: Arc<RwLock<Option<iota_grpc_client::Client>>>`
- `get_grpc_client()` — builds it from the active env's `grpc` URL (errors if unset).
- `IotaEnv.grpc` + `create_grpc_client()` in `iota_client_config.rs`; `IOTA_*_GRPC_URL` constants.

All chain methods (`get_object_ref`, `get_object_owner`, `gas_objects`,
`get_gas_objects_owned_by_address`, `get_reference_gas_price`, `execute_transaction_may_fail`)
still call `get_client()` (JSON-RPC). This spec changes those.

## Decisions (locked)

1. **Default gRPC; opt into JSON-RPC.** `WalletContext` gets a `WalletBackend` toggle whose
   default is **gRPC**, with `with_jsonrpc_backend()` to force JSON-RPC. Non-breaking fallback:
   when the default is gRPC but the active env has **no `grpc` URL**, fall back to JSON-RPC (so
   existing `client.yaml` files keep working; new/test envs with a `grpc` URL get gRPC).
2. **All methods at once.** One plan migrates every chain method listed above.
3. **SDK-native return types.** Methods return `iota-rust-sdk` types, not `iota_json_rpc_types`.
   The gRPC path produces them ~natively; the JSON-RPC fallback converts `json_rpc_types → SDK`.
   The conversion therefore lives on the _dying_ JSON-RPC path and is removed when JSON-RPC is.
   Concrete SDK return types are pinned in the plan (candidates: gRPC/SDK `ExecutedTransaction`
   for execute; `iota_sdk_types::Object` for gas objects; SDK object-ref/`Address` for the ref/
   owner getters).

## What this session resolved (why P1 is now small)

- **Wait-for-execution:** `ExecuteTransactions` takes `checkpoint_inclusion_timeout_ms`; the
  server waits for checkpoint inclusion before returning populated results
  (`iota-grpc-server/.../transaction_execution_service/mod.rs:338`). **No client polling.**
- **Effects/events/object-changes/balance-changes:** native gRPC read-mask fields on
  `ExecutedTransaction` (grpc-types rev `aee56356`). **Map, don't derive.**
- **Owned-object completeness:** the gRPC index already contains genesis/migration objects
  (proven this session). `list_owned_objects` is sufficient for gas selection.
- **Build/execute:** the gRPC client implements `iota-sdk-transaction-builder`'s
  `TransactionBuilderClient`; `iota-vm-sdk/examples/stake_grpc.rs` is a working reference.

## Architecture

```
WalletContext { backend: WalletBackend (default Grpc), client, grpc_client, ... }
   method() ──► resolve backend: Grpc by default; fall back to JsonRpc if env has no `grpc` URL;
                                  with_jsonrpc_backend() forces JsonRpc
                   Grpc    ──► GrpcWalletOps ──► SDK-native types  (produced ~natively)
                   JsonRpc ──► IotaClient ──► json_rpc_types → SDK  (From impls, see below)
```

### Component: backend toggle

A `WalletBackend` enum (`Grpc` | `JsonRpc`) on `WalletContext`, default `Grpc`, plus
`with_jsonrpc_backend()`. Method dispatch resolves the effective backend per the fallback rule
in Decision 1.

### Component: `GrpcWalletOps`

Per-method gRPC implementations returning SDK-native types:

- `get_object_ref(id)` → `GetObjects` (minimal mask) → SDK object reference.
- `get_object_owner(id)` → `GetObjects` (owner mask) → `Address`.
- `gas_objects(addr)` / `get_gas_objects_owned_by_address` → `list_owned_objects(addr, gas-coin
  type filter, page, cursor)` paginated → `Vec<(u64, iota_sdk_types::Object)>` / `Vec<ref>`,
  preserving the `PagedFn::stream` paging behaviour.
- `get_reference_gas_price()` → `GetEpoch` → `u64`.
- `execute_transaction_may_fail(tx)` → `ExecuteTransactions` with `checkpoint_inclusion_timeout_ms`
  and a read mask covering effects/input/events/object-changes/balance-changes → the SDK-native
  executed-transaction result (pinned in the plan). Build via the SDK `TransactionBuilder` +
  the gRPC client's `TransactionBuilderClient` impl (ref: `iota-vm-sdk/examples/stake_grpc.rs`).

### Component: `json_rpc_types → SDK` conversions (JSON-RPC fallback only)

Because the JSON-RPC fallback (`IotaClient`) returns `iota_json_rpc_types`, it converts them to
the SDK-native return types. These conversions belong as `From`/`TryFrom` impls in
**`iota-json-rpc-types`** (which already depends on `iota-sdk-types`), so both `iota-sdk` and
`iota-indexer` can use them — **no new crate**. This is the entirety of Decision-#2's "shared
conversions": there is no gRPC→json_rpc shim on the gRPC path, so nothing throwaway rides the
surviving path. Audit `iota-indexer`'s existing conversions and move/share rather than duplicate.

### Component: CLI transaction output

CLI commands print `IotaTransactionBlockResponse` via its `Display`/formatting. With the
SDK-native return type, the CLI's transaction-result formatting is updated to render the
SDK-native executed-transaction result (either new formatting or a thin display adapter). This
is the sharpest edge of the change; the plan enumerates the affected CLI commands.

## Testing

- **Unit:** `json_rpc_types → SDK` conversion impls against fixed fixtures (object with each
  option; execution response with effects/events/changes).
- **Parity:** against a test-cluster node serving both protocols, assert the gRPC and JSON-RPC
  backends return equivalent **SDK-native** values for the same state (object ref/owner, gas
  objects, gas price, execution result).
- **Opt-in wiring:** `test-cluster` builds its `WalletContext` with the gRPC backend + enables
  the fullnode gRPC API; run the wallet-driven e2e/sim suite green over gRPC. (Node still serves
  JSON-RPC during this phase, so the JSON-RPC path stays covered too.)
- Determinism: confirm the gRPC client works under `#[sim_test]` (the migration test already
  proved gRPC works in-simulator this session).

## Scope note

Updating every consumer of the changed `WalletContext` methods to the SDK-native return types is
**in scope** — it is mandatory for the tree to compile (~22 execute-consumers, 5 gas-object
consumers, CLI transaction output). This is the bulk of the churn.

## Out of scope

- Removing the node's JSON-RPC server (later phase).
- Migrating CLI/faucet/tool code paths that use `IotaClient` **directly** (not via the changed
  `WalletContext` methods) onto gRPC — that is the separate production-client phase.
- Any `WalletContext` method not listed above.

## Risks

- **Mapping fidelity** is the main risk — subtle field differences between gRPC and JSON-RPC
  object/effects shapes. Mitigated by the parity tests and by keeping the node dual-serving.
- **`ExecuteTransactions` semantics vs `WaitForLocalExecution`:** verify that with
  `checkpoint_inclusion_timeout_ms` set, an immediately-following read observes the effects
  (tests that execute-then-read must stay green).
- **Caller churn / CLI output:** ~22 execute-consumers + CLI transaction formatting move to
  SDK-native shapes. Risk of subtle CLI output changes; the plan should snapshot/verify CLI
  output before and after for the affected commands.
- **Coupling to #12197:** base branch is an open PR; rebase when it merges.
