# Follow-up PR plan: fold `PruningCoordinator` into `AuthorityStorePruner`

Follow-up to #12186 (execution-driven pruning). Pure refactor, **no behavior
change**. Addresses review feedback: "put the `PruningCoordinator` implementation
details (the two watch channels) into `AuthorityStorePruner`, no need to add
another glue component."

Branch off `develop` **after #12186 merges** (e.g. `refactor/fold-pruning-coordinator`).

## Why

`PruningCoordinator` is a thin type holding two `watch` channels that shuttle
signals between the checkpoint executor and the pruner task. `watch::Sender` is
`Clone` (tokio 1.52.2), so the pruner can own the senders directly and hand clones
to its spawned task — no separate `Arc`-shared component, one fewer type, and the
pruner becomes the natural owner of its own coordination channels.

## Current shape (to remove)

- `PruningCoordinator { executed: watch::Sender<CheckpointSequenceNumber>,
  frontier_ms: watch::Sender<CheckpointTimestamp> }` with `nudge` / `await_leash`
  (pub) and `subscribe_executed` / `set_frontier` (private).
- `AuthorityStorePruner { _objects_pruner_cancel_handle }`.
- `AuthorityState` stores both `_pruner: AuthorityStorePruner` **and**
  `pruning_coordinator: Arc<PruningCoordinator>`, with a `pruning_coordinator()`
  getter; `AuthorityStorePruner::new(..)` takes `coordinator: Arc<PruningCoordinator>`.
- Executor calls `self.state.pruning_coordinator().nudge(..)` / `.await_leash(..)`.

## Target shape

```rust
pub struct AuthorityStorePruner {
    _objects_pruner_cancel_handle: oneshot::Sender<()>,
    executed: watch::Sender<CheckpointSequenceNumber>,   // executor -> pruner (nudge)
    frontier_ms: watch::Sender<CheckpointTimestamp>,     // pruner -> executor (leash)
}
```

with `pub fn nudge(&self, seq)` (→ `self.executed.send_replace(seq)`) and
`pub async fn await_leash(&self, ts)` (→ `self.frontier_ms.subscribe()` then the
same slack loop) as methods on the pruner.

## Changes

### `crates/iota-core/src/authority/authority_store_pruner.rs`

- Delete the `PruningCoordinator` struct + impl (fold `nudge` / `await_leash` onto
  `AuthorityStorePruner`; inline `subscribe_executed` / `set_frontier`).
- Add the two `watch::Sender` fields to `AuthorityStorePruner`.
- `new()`: create both channels; drop the `coordinator: Arc<PruningCoordinator>`
  param; store the two senders + the cancel handle; pass into `setup_pruning` the
  channel ends the task needs — the executed **receiver** (`executed.subscribe()`)
  and a **clone** of the frontier sender (`frontier_ms.clone()`).
- `setup_pruning()`: take `executed_rx: watch::Receiver<CheckpointSequenceNumber>`
  and `frontier_tx: watch::Sender<CheckpointTimestamp>`; the task uses them
  directly (loop waits on `executed_rx.changed()`, publishes via
  `frontier_tx.send_replace(caught_up_to)`). Keep the `u64::MAX` frontier init,
  the leash-slack loop, the catch-up debounce, and `leash_enabled` gating exactly
  as-is.
- Keep `PRUNING_LEASH_SLACK_MS` doc references as plain code spans (not intra-doc
  links) so the pub method docs don't warn about linking a private const.

### `crates/iota-core/src/authority.rs`

- Remove the `pruning_coordinator: Arc<PruningCoordinator>` field, its creation
  (`PruningCoordinator::new()`), the `.clone()` passed to the pruner, the struct
  literal entry, the `pruning_coordinator()` getter, and the `PruningCoordinator`
  import.
- Drop the `coordinator` argument from the `AuthorityStorePruner::new(..)` call.
- Rename `_pruner` → `pruner`; add `pub fn pruner(&self) -> &AuthorityStorePruner`.

### `crates/iota-core/src/checkpoints/checkpoint_executor/mod.rs`

- `self.state.pruning_coordinator().nudge(..)` → `self.state.pruner().nudge(..)`.
- `self.state.pruning_coordinator().await_leash(..)` → `self.state.pruner().await_leash(..)`.

### Tests (same file)

- Rework the three coordinator unit tests (`test_leash_passes_within_slack`,
  `test_leash_blocks_until_frontier_advances`, `test_nudge_wakes_subscriber`) to
  build the pruner's channels directly rather than via `PruningCoordinator::new()`.
  `AuthorityStorePruner::new()` spawns the task + needs the DBs, so do **not** use
  it here; construct the struct literal in-module with a dummy cancel handle
  (`oneshot::channel().0`) and the two `watch` channels, then exercise
  `nudge` / `await_leash` and drive `frontier_ms.send_replace(..)` / subscribe on
  `executed` to assert the same behavior.

## Verification

- `cargo check -p iota-core -p iota-node -p iota-tool`
- `cargo clippy -p iota-core --tests` (clean)
- `cargo +nightly fmt`
- `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core authority_store_pruner`
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p iota-core` (no private
  intra-doc-link warnings)

## Notes / invariants to preserve

- **No behavior change**: same nudge-on-availability, same leash semantics
  (`executed_ts − frontier ≤ SLACK`, frontier = caught-up executed timestamp),
  same `u64::MAX` init, same catch-up debounce + `PRUNING_DEBOUNCE_MIN_LAG` gate.
- Relies on `watch::Sender: Clone` to give the task its frontier sender clone.
- `await_leash(&self)` borrow across `.await` stays fine: the executor holds
  `Arc<AuthorityState>`, so `&pruner` outlives the await (same as today).
- No config or docs changes.
- Scope is confined to `iota-core` (pruner, `authority.rs`, executor).
