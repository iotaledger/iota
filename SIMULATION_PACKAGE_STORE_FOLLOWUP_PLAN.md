# Follow-up PR plan: resolve a simulation's own packages everywhere, from one combinator

Third follow-up to #12508 (unify dry run and dev inspect on
`simulate_transaction`), landing after the indexer parity PR. Fixes a gap that
PR exposed rather than caused, and removes the indexer-local types it had to
introduce.

Branch off `develop` **after both earlier PRs merge** (e.g.
`fix/resolve-simulated-packages`).

**This changes node behavior** and needs a release note.

## Why

A simulated transaction can publish a package and then refer to its types. The
package was never committed, so only the simulation's own output holds it, and
anything resolving types against the committed store alone cannot decode it.

#12508 fixed that for the node's dry-run **events** and **input**, by resolving
over `simulation.output_objects` with the backing store behind them. The indexer
parity PR did the same for its two methods. Neither touched **clever errors**,
and the node still resolves those against the committed store only:

```rust
// crates/iota-json-rpc/src/transaction_execution_api.rs:417
let resolver = Resolver::new(self.clone());
let effects = IotaTransactionBlockEffects::from_native_with_clever_error(
    simulation.effects,
    &resolver,
)
```

`impl PackageStore for TransactionExecutionApi` (`:654`) reads only
`self.state.get_backing_package_store()`. So a dry run whose published package
aborts with an `#[error]` constant in its `init` reports the raw abort code where
the same request on the indexer now reports the message — the indexer is ahead of
the node, which is backwards.

Fixing it needs a primary-plus-fallback `PackageStore`, which the indexer PR
already had to write locally as `SimulationPackageStore`. That makes two crates
wanting the same thing, so it belongs in `iota-package-resolver` — and the
indexer's copy goes away.

## Scope: where the gap is, and where it is not

Every caller of `from_native_with_clever_error`, checked:

| Call site                                                                  | Simulated effects?                             | Gap?                                             |
| -------------------------------------------------------------------------- | ---------------------------------------------- | ------------------------------------------------ |
| `iota-json-rpc/src/transaction_execution_api.rs:418` (dry run)             | yes                                            | **yes** — this PR                                |
| `iota-graphql-rpc/src/types/transaction_block_effects.rs:134` (`errors()`) | yes, via `TransactionBlockEffectsKind::DryRun` | **yes** — see [GraphQL](#graphql)                |
| `iota-indexer/src/apis/write_api.rs:213` (dry run)                         | yes                                            | already fixed, switches to the shared types here |
| `iota-json-rpc/src/transaction_execution_api.rs:293` (execute)             | no                                             | no — the package is committed by then            |
| `iota-json-rpc/src/read_api.rs:1379`                                       | no                                             | no — reading a committed transaction             |
| `iota-indexer/src/models/transactions.rs:425`                              | no                                             | no — reading a committed transaction             |

**Dev inspect has no clever errors at all** on either surface:
`DevInspectResults` builds its effects with `effects.try_into()?`
(`iota-json-rpc-types/src/iota_transaction.rs:1446`), which does not resolve
aborts. Nothing to fix there, and no reason to add it in this PR.

## Target shape

Two types in `crates/iota-package-resolver/src/lib.rs`, next to the existing
`PackageStoreWithLruCache`. The crate already imports `iota_types::object::Object`
(`:21`) for `Package::read_from_object`, so no new dependency.

```rust
/// Reads packages from `primary`, falling back to `fallback` for any it does not
/// hold.
pub struct PackageStoreWithFallback<P, F> {
    primary: P,
    fallback: F,
}

impl<P, F> PackageStoreWithFallback<P, F> {
    pub fn new(primary: P, fallback: F) -> Self {
        Self { primary, fallback }
    }
}

#[async_trait]
impl<P: PackageStore, F: PackageStore> PackageStore for PackageStoreWithFallback<P, F> {
    async fn fetch(&self, id: Address) -> Result<Arc<Package>> {
        match self.primary.fetch(id).await {
            Ok(package) => Ok(package),
            Err(_) => self.fallback.fetch(id).await,
        }
    }
}

/// Reads packages out of a fixed set of objects — the ones an execution wrote,
/// for a caller resolving types a simulated transaction introduced.
///
/// Objects are kept as they came and deserialized on demand: an execution
/// usually publishes nothing, and when it does the package is only read if a
/// type refers to it.
pub struct ObjectsPackageStore {
    packages: BTreeMap<Address, Object>,
}

impl ObjectsPackageStore {
    pub fn new<'a>(objects: impl IntoIterator<Item = &'a Object>) -> Self { .. }
}

#[async_trait]
impl PackageStore for ObjectsPackageStore {
    async fn fetch(&self, id: Address) -> Result<Arc<Package>> {
        let Some(object) = self.packages.get(&id) else {
            return Err(Error::PackageNotFound(id));
        };
        Package::read_from_object(object).map(Arc::new)
    }
}
```

Note the name collision with `iota_types::inner_temporary_store::PackageStoreWithFallback`,
which does the same job for `BackingPackageStore`. Two traits, two crates, same
concept — keeping the names parallel is clearer than coining a second word, and
`PackageStoreWithLruCache` already sets that pattern in this crate. Call it out
in the doc comment so a reader who has met the other one is not surprised.

### Sub-option considered and not taken

An adapter `impl PackageStore for T where T: BackingPackageStore` would let the
resolver crate reuse `iota-types`'s existing `ObjectMapPackageStore` and
`PackageStoreWithFallback` instead of adding two more. It removes the
duplication more thoroughly, but it welds the two traits together and produces
types like `Adapter<PackageStoreWithFallback<ObjectMapPackageStore, …>>` at every
call site. Two small types in the resolver crate read better. Revisit only if a
third trait pairing shows up.

## Work, in order

Each step builds, tests and commits on its own.

1. **Add the two types.** `crates/iota-package-resolver/src/lib.rs`, with the doc
   comments above. Unit tests in that crate's existing `mod tests`, which already
   has `InMemoryPackageStore` to compose against: assert `ObjectsPackageStore`
   resolves a package it holds and reports `PackageNotFound` for one it does not,
   and that `PackageStoreWithFallback` prefers the primary and falls through
   otherwise. Nothing consumes them yet.

2. **Fix the node's dry run.** In
   `crates/iota-json-rpc/src/transaction_execution_api.rs`, replace
   `Resolver::new(self.clone())` at `:417` with
   ```rust
   let resolver = Resolver::new(PackageStoreWithFallback::new(
       ObjectsPackageStore::new(simulation.output_objects.values()),
       self.clone(),
   ));
   ```
   Leave `:289` (the execute path) alone — see the scope table. `impl PackageStore
   for TransactionExecutionApi` stays; it is now the fallback rather than the whole
   story.

3. **Add the test that proves it.** A Move fixture under
   `crates/iota-json-rpc-tests/tests/data/`, modelled on the existing
   `publish_with_event` package added in #12508, whose `init` aborts with an
   `#[error]` constant — `crates/iota-indexer/tests/data/clever_errors/sources/clever_errors.move`
   is the shape to copy:
   ```move
   #[error]
   const ENotReady: vector<u8> = b"package is not ready";

   fun init(_ctx: &mut TxContext) {
       assert!(false, ENotReady);
   }
   ```
   Dry-run the publish through `iota_dryRunTransactionBlock` and assert
   `effects.status()` is `Failure` whose `error` contains the message, not a bare
   abort code. Confirm it fails before step 2 — an `init` abort is the only way a
   simulated transaction reaches its own package's clever error, since a PTB
   cannot move-call a package it publishes in the same transaction.

4. **Move the indexer onto the shared types.** Delete
   `SimulationPackageStore<F>` from
   `crates/iota-indexer/src/store/package_resolver.rs` and compose
   `PackageStoreWithFallback::new(ObjectsPackageStore::new(&output_objects), …)`
   at both call sites in `crates/iota-indexer/src/apis/write_api.rs`. Behavior is
   unchanged; this is the deduplication the PR exists for. Keep the `Clone` bound
   on both `_impl` signatures — the fallback still comes from
   `Resolver::package_store().clone()`.

5. **Record the remaining duplication.** `iota-types`'s
   `PackageStoreWithFallback` and `ObjectMapPackageStore` still exist for
   `BackingPackageStore`. Add a line to
   `SIMULATION_INPUT_CHECKS_FOLLOWUP_PLAN.md` noting the two pairs, so the next
   person sees it is known rather than missed.

## GraphQL

`TransactionBlockEffects::errors()` resolves clever errors through the ambient
`PackageResolver` from the async-graphql context, for every
`TransactionBlockEffectsKind` including `DryRun`. So GraphQL's
`dryRunTransactionBlock` has the same gap.

It is a bigger change than the node's: the `DryRun` variant would have to carry
the objects the simulation wrote, and `errors()` compose a resolver per call
rather than taking the shared one from the context. That is a different crate,
owned elsewhere, and touches a field resolver on a hot type.

**Recommendation: leave it out of this PR** and open an issue against
`iota-graphql-rpc` referencing the fix here, so whoever owns it can decide
whether the case is worth the plumbing. Note that GraphQL's dry run goes through
`dev_inspect_transaction_block` (`iota-graphql-rpc/src/types/query.rs:206`), so
its effects come from `DevInspectResults` — which does not resolve clever errors
at all today. Check whether the `DryRun` variant can actually carry a clever
abort before spending anything on it.

## Tests

- **New**, per step 1: the four unit assertions on the two types.
- **New**, per step 3: the dry-run clever-error test, confirmed red before step 2.
- **Must keep passing:** the whole `iota-json-rpc-tests` suite, run **without**
  `IOTA_SKIP_SIMTESTS=1` — with it set, `#[sim_test]`s report as passes without
  executing, so a green summary means nothing for these.
  `test_dry_run_resolves_events_of_newly_published_package` is the closest
  neighbour and exercises the same written-package path through a different
  resolver.
- **Must keep passing:** `iota-json-rpc/src/read_api.rs` and the execute path
  behavior — step 2 deliberately does not touch `:289`, and the read paths resolve
  committed packages, so `cargo nextest run -p iota-json-rpc -p iota-core` should
  be unchanged.
- **Cannot be run locally:** step 4's indexer call sites need Postgres. Since
  step 4 is a pure refactor with the same composition, the risk is a compile-time
  one, but say so in the PR rather than implying coverage.

## Out of scope

- Dev inspect. No clever-error resolution exists on that path on any surface.
- The execute and read paths. Their packages are committed.
- Unifying `iota-types`'s `BackingPackageStore` combinator with this one; see the
  sub-option above.
- The LRU cache on the indexer's write path, which still resolves every package
  straight from Postgres. Unrelated latency work, noted in
  `INDEXER_SIMULATION_PARITY_PR.md`.
