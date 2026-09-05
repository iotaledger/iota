# Follow-up PR description: bring the indexer's JSON-RPC onto the node's simulation behavior

Now open as #12669 on `feat/unify-json-rpc-behaviour`, off `develop` — #12508
merged on 2026-08-07, so this is a plain follow-up rather than a stacked branch.
Two commits, both confined to `crates/iota-indexer`. Tracked under #12671.

The PR body is pushed and current; this file keeps the design record below it.

---

# Description of change

#12508 made the node's `iota_dryRunTransactionBlock` and
`iota_devInspectTransactionBlock` agree with gRPC `simulate_transactions` on the
gas a simulation runs with and reports. The indexer serves the same two JSON-RPC
methods by calling that gRPC endpoint, but shaped its own response — so the
contract #12508 established held on the node's own server and not on the path
clients reach in a deployment.

This brings the indexer onto the same behavior. Both commits are
`crates/iota-indexer` only.

## 1. Report the gas and the changes the simulation produced

- Drop the indexer's own `reference_gas_price_and_max_tx_gas` lookup. It
  resolved a missing gas price and budget client-side, from an extra `get_epoch`
  call, before sending the transaction on. The node resolves both now, so the
  caller's values — including a zero — are passed through untouched and the
  simulation fills them in.
- Report the transaction the simulation ran, read back over gRPC
  (`SimulateExecutedTransactionField::TRANSACTION_BCS`), rather than the one the
  indexer assembled. That transaction carries the price the run charged at, the
  gas charged in place of a zero budget, and the mock gas coin minted for an
  empty payment. For dev inspect the field is only requested when
  `show_raw_txn_data_and_effects` asks for the transaction back, since it costs
  bytes on the wire.
- Key the response off the effects' transaction digest — the digest of what
  actually ran. It differs from the digest of the transaction as sent whenever a
  mock gas coin was added, and the effects and events are keyed by the former.
- Exclude the mock gas coin from balance changes. A transaction sent without a
  gas payment is simulated against a coin the caller does not own, so the
  balance change of paying gas out of it is not theirs either. The node
  excludes it; the indexer passed `None` and reported it. The coin is
  identified by diffing the payment that was sent against the payment the
  response reports, which is possible because #12508 made the response name it.
  Object changes keep the mock coin, which is what the node and gRPC both do.
- `TxObjectResolver::get_changes` goes away with that: it was used only by the
  dry run, and the two derivations it wrapped are now called directly with the
  arguments the node uses. The ingestion path has its own copy in
  `ingestion::primary::prepare` and is untouched.

Net effect: gas price, gas budget, mock gas coin and input checks now behave
identically on the indexer and on the node, and the balance changes, object
changes and reported digest come from the same functions with the same
arguments.

Two things are deliberately _not_ the same code, and are worth a reviewer's
attention:

- `input` and `events` resolve types through different machinery — the node uses
  the epoch's module cache and the executor's layout resolver, the indexer uses
  `iota-package-resolver`. After commit 2 both consult the simulation's written
  packages first with a store behind them, so the outcome should agree, but it is
  not the same implementation.
- Clever-error rendering in `effects` now goes _further_ on the indexer than on
  the node. The node resolves clever errors through
  `TransactionExecutionApi as PackageStore`, which reads only the committed
  backing store, so an abort raised from a package the dry run just published
  does not render there. The indexer resolves it through the same
  `SimulationPackageStore` as everything else, so it does. The node is the one
  that should be brought up; see the note at the end of this file.

## 2. Resolve types against the packages the simulation published

A dry run that publishes a package and emits an event from its `init` could not
decode that event through the indexer: types were resolved only against
`IndexerStorePackageResolver`, the database, which cannot contain a package that
was never committed. The node resolves over the objects the simulation wrote
first and falls back to its store, so the same request decodes there.

`SimulationPackageStore<F>` does the same for the indexer — the packages among
the objects the simulation wrote, then `F`. It keeps them as objects and calls
`Package::read_from_object` on demand, since a simulation usually publishes
nothing and, when it does, the package is only read if a type refers to it. Both
methods build one per request, taking the fallback from
`Resolver::package_store().clone()`; that is a `ConnectionPool` clone, and the
write path holds no package cache to lose.

Applied to dev inspect as well as the dry run, which costs
`OUTPUT_OBJECTS_BCS` on the dev-inspect read mask: the written objects now go
over the wire for every dev inspect, whether or not it publishes. Requesting
them only when the effects report a publish would save the common case at the
cost of branching the read mask; that trade is worth revisiting if dev-inspect
response size becomes a problem.

## Links to any relevant issues

Follow-up to #12508.

## How the change has been tested

- [x] Basic tests (linting, compilation, formatting, unit/integration tests)
- [x] Patch-specific tests (correctness, functionality coverage)
- [ ] I have added tests that prove my fix is effective or that my feature works
- [x] I have checked that new and existing unit tests pass locally with my changes

`cargo clippy -p iota-indexer --all-targets --all-features -- -D warnings`,
`cargo +nightly fmt`, `dprint check` and `cargo check --workspace --all-targets`
are clean. The 27 `iota-indexer` unit tests pass.

**The behavior this PR changes is not covered locally.** The indexer's JSON-RPC
dry-run and dev-inspect tests need Postgres and were not run. Two existing tests
in `crates/iota-indexer/tests/rpc-tests/write_api.rs` bracket the change and
should both still hold:

- `dry_run_transaction_block` compares the reported `input` against an executed
  transaction built with a real gas coin and a non-zero price and budget, so the
  transaction the response now reports is the one that was sent.
- `dev_inspect_transaction_block` asserts nothing about gas.

Worth adding before merge, once a Postgres run is available:

- a gasless dry run, asserting the reported payment names the mock coin and that
  no balance change is attributed to it;
- a dry run that publishes a package emitting an event from `init`, asserting
  `parsed_json` decodes — the indexer counterpart of
  `test_dry_run_resolves_events_of_newly_published_package`, and the only thing
  that would exercise commit 2 at all.

### Release Notes

- [ ] Protocol:
- [ ] Nodes (Validators and Full nodes):
- [x] Indexer: `iota_dryRunTransactionBlock` and `iota_devInspectTransactionBlock` now behave the way the node's own JSON-RPC does. Gas the caller leaves unset is filled in by the node rather than by the indexer — an empty payment gets a mock gas coin, a zero `gas_price` the epoch's reference gas price, and a zero `gas_budget` the protocol maximum — and the reported transaction carries what the simulation charged, including the mock gas coin, instead of the transaction as sent. A gasless `iota_dryRunTransactionBlock` reports the digest of what ran, so created object IDs change accordingly, and no longer attributes a balance change to the mock gas coin. Event types are now resolved against packages the simulated transaction itself published, so a dry run that publishes a package can decode the events its `init` emits.
- [ ] JSON-RPC:
- [ ] GraphQL:
- [ ] CLI:
- [ ] Rust SDK:
- [ ] gRPC:

<!-- Do not remove: everything below this line is ignored by the release-notes check. -->

---

# Design decision for commit 2 (not part of the PR body)

Both approaches need the same two pieces: a `PackageStore` over the objects the
simulation wrote (via `Package::read_from_object`) and a primary-then-fallback
combinator. `iota_package_resolver::PackageStore` is a one-method async trait
and is `'static`, so the fallback has to be owned — it cannot borrow.

Three things that are _not_ constraints, contrary to first appearances:

- `Resolver<S>` does not cache. It is `{ package_store, limits }`, so building
  one per request loses no cache.
- `WriteApi` holds a bare `IndexerStorePackageResolver`, not the
  `PackageStoreWithLruCache` that `read.rs` uses, so the write path already hits
  Postgres per package.
- `Resolver::package_store()` exists and `IndexerStorePackageResolver` is
  `Clone` (it clones a `ConnectionPool`), so obtaining an owned fallback is
  cheap.

**Chosen: indexer-local.** Both types live in
`crates/iota-indexer/src/store/package_resolver.rs`. The fallback is an owned
`IndexerStorePackageResolver` clone, so no `Arc<dyn PackageStore>` and no extra
generic bounds. `dry_run_transaction_block_impl` has one caller passing the
concrete field, so its signature is free to change. The commit stays inside
`crates/iota-indexer` and remains splittable.

The cost is that this is the third place a primary-plus-fallback package store
is written — `iota_types::inner_temporary_store::PackageStoreWithFallback`
already does it for `BackingPackageStore`. That is the same duplication class
#12508 exists to remove, and it is recorded in
`SIMULATION_INPUT_CHECKS_FOLLOWUP_PLAN.md`.

**Rejected: reusable combinator in `iota-package-resolver`.** A generic
`PackageStoreWithFallback<P, F>` plus an objects-backed store, which the
indexer, node and GraphQL could all compose. Better long-term shape, but it
makes the commit cross-crate and so unsplittable from the indexer PR. Worth
doing as its own PR landing first, after which the indexer composes it and the
local types are deleted.

**Also rejected: taking balance and object changes from gRPC.** The read mask
offers them, `DerivedBalanceChange` is field-identical to
`iota_json_rpc_types::BalanceChange`, and `DerivedObjectChange` mirrors
`ObjectChange` variant for variant — and it would let the read mask drop
`INPUT_OBJECTS_BCS` and `OUTPUT_OBJECTS_BCS`, which exist only to feed the local
derivation. But `iota-grpc-server::changes` is a second implementation, separate
from the `iota-json-rpc` one the node uses, with no test pinning them
equivalent; one difference is already visible, in that `derive_balance_changes`
errors when a required object is missing from the response sets while the
indexer's `ObjectProvider` falls back to its own store. Since the goal is that
the indexer match the node, using the node's own functions guarantees it where
using gRPC's only makes it conditional. Revisit once the two derivations are
unified — at which point dropping the objects from the read mask is a real
wire-size win.

**Settled: dev inspect gets the fix too**, unconditionally. The node resolves
dev-inspect types over written objects as well, so leaving the indexer's dev
inspect out would have traded one divergence for another. The cost is
`OUTPUT_OBJECTS_BCS` on its read mask for every call.

## Left out on purpose

- **The LRU cache.** The write path has none, so every resolution hits Postgres.
  Wrapping the fallback in `PackageStoreWithLruCache` is pointless per request
  and would need to live on `WriteApi` to help across requests. Unrelated
  latency work.
- **Bringing the node's clever-error resolution up to match.** The node reads
  only its committed backing store when rendering clever errors
  (`impl PackageStore for TransactionExecutionApi`), so an abort from a
  just-published package renders on the indexer and not on the node. Fixing it
  means resolving clever errors over `simulation.output_objects` with the
  backing store behind them, in `iota-json-rpc` — outside this PR's crate, and
  a change to the node's output that wants its own release note.
