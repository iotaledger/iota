# JSON-RPC index rebuild (PR #12332) — review findings & follow-up decisions

Status of PR #12332 (`fix/rebuild-json-rpc-indexes`) after review on 2026-07-24.

## Decision: drop existing databases instead of adopting them — IMPLEMENTED

The PR originally adopted a pre-upgrade index database in place. Changed on the branch
(2026-07-24): a database without a `meta` row is always wiped and rebuilt.

Rationale:

- Existing databases are likely wrong. Any fullnode restored from a formal snapshot has
  a corrupted owner index (from the now-deleted `create_owner_index_if_empty`) and
  transaction numbering that restarted near 0. Adoption would preserve both defects and
  permanently stamp the database as healthy.
- Adoption also can't distinguish a healthy pre-upgrade DB from one with a gap (node ran
  with `enable_index_processing` off for a while) — review issue below.
- A forced rebuild makes the whole fleet uniform: canonical network-order numbering and
  checkpoint timestamps everywhere, instead of only on rebuilt nodes.
- Validators are unaffected: `IndexStore` is only created when
  `is_full_node && enable_index_processing` (`iota-node/src/lib.rs`).

Implementation notes:

- `needs_to_do_initialization` already returns true when `meta` is absent; remove the
  watermark seeding for non-empty databases from `seed_meta_and_watermark` (and the
  `test_pre_watermark_database_is_adopted_in_place` test). The "data without `meta`"
  special case disappears entirely.
- Keep the skip-already-indexed check in `index_checkpoint` — it is still required for
  crash-recovery replay (index commit happens before the executed-watermark bump).
- "Actively wait" is already the behavior: `IndexStore::new` blocks node startup until
  the rebuild finishes.

Operational cost to state in the PR/release notes: every fullnode pays one rebuild on
first start after the upgrade. Pruned fullnodes replay only their retention window;
**unpruned/archival fullnodes replay their entire local history**, which can take hours —
operators need to schedule for it.

## Review findings (from the PR #12332 review)

Implemented on the branch (2026-07-24):

1. ~~Rebuild does redundant live-state work during history replay.~~ Done: replay now
   writes only the history tables from (transaction, effects, events) — no object
   loads, no layout resolution — and is bounded only by the checkpoint-contents pruner
   (the object pruner no longer shrinks the replayed range).
2. ~~Adoption can stamp a gappy pre-upgrade DB as healthy.~~ Resolved by the
   drop-instead-of-adopt decision above (implemented).
3. ~~Inconsistent handling of a future `Owner` variant.~~ Done: `process_object_index`
   now uses the same explicit match as the live scan and the gRPC indexer.
4. ~~`std::sync::Mutex` for `pending_updates`.~~ Done: `parking_lot::Mutex`.
5. ~~Long blocking init inside an async fn.~~ Done: `init` runs in `spawn_blocking`
   (`checkpoint_store` param became `&Arc<CheckpointStore>` for that).
6. ~~Numbering anchor silently falls back to 0.~~ Done: `warn!` added.

7. ~~PR description needs a Release Notes section.~~ Done: description updated
   (adoption bullet replaced with always-rebuild, one-time upgrade cost stated) and
   Release Notes added for Nodes and JSON-RPC; `release_notes.py check-pr 12332`
   passes.

8. ~~Test gaps.~~ Done: `test_stale_database_is_wiped_and_rebuilt_on_open` covers the
   full wipe path (bulk-ingestion open, flush, reopen; stale rows do not survive), and
   `test_watermark_ahead_of_executed_needs_no_rebuild` plus the existing
   `test_index_checkpoint_skips_already_indexed` cover the crash window between index
   commit and the executed-watermark bump at the store level. A node-level simtest
   killing a fullnode at the executor's `crash` fail point was considered and skipped:
   no existing restart-at-fail-point machinery to reuse, and the pipeline ordering it
   would exercise is already enforced structurally.

## Formal-snapshot restore and the JSON-RPC index — IMPLEMENTED

`DownloadFormalSnapshot` now builds the JSON-RPC index too, teed from the same object
stream that restores the state, exactly like the gRPC indexes (`RestoreWithIndexes`
feeds both stores per partition; opt-out via `--skip-jsonrpc-indexes`).
`JsonRpcIndexRestorer::finalize` seeds `meta`, `watermark = restore checkpoint`, and
`history_watermark = restore checkpoint + 1`, so the node's first open adopts the
database and the history backfill has nothing to do.

## Per-epoch history buckets — IMPLEMENTED (branch feat/jsonrpc-index-epoch-buckets)

Follows #12144's design: the 13 history tables live in per-epoch column families of
the jsonrpc_indexes database, created at runtime (via the typed-store changes ported
from #12144 — drop that commit when rebasing over its merge). Queries chain per-bucket
scans in epoch order; digest lookups probe newest first through bloom filters; pruning
drops whole epoch buckets in constant time, replacing the compaction-filter machinery
entirely. `num_epochs_to_retain_for_indexes` now keeps exactly the newest N bucket
epochs. Coordinate the merge order with #12144's author — both PRs touch the pruner
entry points, and the typed-store commit must be dropped on rebase once #12144 lands.

## Key-only dynamic-field index — IMPLEMENTED

The dynamic-field table stores only `(parent, field)` keys, like the gRPC index;
`DynamicFieldInfo` is resolved at query time in `AuthorityState` from the live `Field`
object, bounded by the query's page size. This removed layout resolution and
dynamic-object-field child lookups from the indexing hot path, the rebuild, and the
restore (which is what allowed the restore to become an in-stream tee), and results
always reflect the live child instead of an indexing-time snapshot. No schema version
bump: databases from before this change carry no `meta` row and are wiped on open.

## Decision: startup phasing for the rebuild — IMPLEMENTED

Implemented on the branch (2026-07-24, commit 804b3ffec6): only the live-object scan
blocks startup; the history replay runs in the background, newest first, resuming from
the `history_watermark` marker that commits atomically with each checkpoint's rows.

- **Live scan stays blocking.** It assumes a frozen live object set; running it
  concurrently with checkpoint execution races with the per-checkpoint updates (scan
  writes back a stale owner row after a transfer already indexed it away). Making that
  safe needs a snapshot-consistent scan plus delta reconciliation — not worth it for the
  fast, live-set-bounded phase. Matches the gRPC index rebuild behavior.
- **History replay moves to the background.** Precondition: finding 1 below (replay must
  write only the history tables). Then the two writers are disjoint by construction —
  replay covers sequence numbers ≤ the anchor at the watermark, live indexing continues
  above it. History-only replay needs no layout resolution and no epoch store.
- Replay newest-first (descending from the watermark), with a progress marker for the
  backfill lower bound so a crash resumes instead of re-wiping, and so the covered
  range is known.
- Background replay uses normal write options — the bulk-ingestion open (WAL off,
  exclusive reopen) doesn't compose with a concurrently-serving DB. Slower, but nobody
  waits on it.
- Query behavior during backfill: serve partial history. The node is indistinguishable
  from a heavily-pruned node whose retention window grows backwards, and pruned nodes
  already serve partial history through these endpoints. Add an explicit "backfill in
  progress" signal only if consumers need it.
- Both phases in the background: rejected (snapshot-reconciliation complexity for the
  cheap phase).

Startup sequence: wipe → live scan (blocking) → node/API start, per-checkpoint indexing
begins → history backfill runs down from the watermark in the background.

Not implemented (deliberately): an explicit "backfill in progress" signal on the
history endpoints — during backfill the node behaves like a pruned node, which the API
contract already tolerates. Add signaling only if RPC consumers ask for it.

## Sharing tables between the gRPC and JSON-RPC indexes

**Decision: not pursuing.** The compat layers (cursor reconstruction, on-demand
digest/previous-transaction lookups, result-order changes) don't pay for themselves,
especially with the JSON-RPC removal on the roadmap. The single-scan optimization (last
bullet below) remains worth considering independently. Analysis kept for reference.

Comparison of `grpc_indexes.rs` and `jsonrpc_index.rs` tables. The history tables — the
bulk of the disk — have no gRPC counterpart and can't be shared; the candidates are the
live-state tables.

| gRPC table                                                                                               | JSON-RPC counterpart                                                                                                                   | Shareable?                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| -------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `owner`: `(owner, type_id_hash, type_params_hash, inverted_balance, object_id)` → `(StructTag, version)` | `owner_index`: `(owner, object_id)` → `ObjectInfo{version, digest, type, owner, previous_transaction}`                                 | **Yes, with a compat layer.** Missing `digest`/`previous_transaction` can be fetched from the object store by `(id, version)` at query time. Caveats: iteration order changes (type-grouped, richest-first vs. by object id), and resuming from a JSON-RPC cursor (a bare `ObjectId`) requires fetching the cursor object to reconstruct the gRPC key — breaks if the cursor object was deleted between pages.                                         |
| `dynamic_field`: `(parent, child)` → `()` — metadata loaded on demand at query time                      | `dynamic_field_index`: `(parent, child)` → full `DynamicFieldInfo` (JSON-rendered name, types, version, digest) built at indexing time | **Yes — best candidate.** Adopt the key-only design and build `DynamicFieldInfo` at query time from the live child object (the gRPC server already does exactly this in `list_dynamic_fields`). This removes layout resolution from the JSON-RPC write path and rebuild entirely — the most expensive and bug-prone part (the DOF-fallback fix in this PR exists only because of it). Page sizes are bounded, so per-query resolution cost is bounded. |
| `owner` (balance embedded in key as `inverted_balance`)                                                  | `coin_index`: `(owner, coin_type_string, object_id)` → `CoinInfo{version, digest, balance, previous_transaction}`                      | **Falls out of owner-table sharing.** `getCoins` = owner iteration with exact-type filter (richest-first order built into the key); `getBalance`/`getAllBalances` = summation over the same iteration, with the existing LRU balance caches kept on top. Same digest/previous-transaction on-demand caveat.                                                                                                                                            |
| `coin`: `coin_type` → metadata object ids (CoinMetadata/Treasury/RegulatedCoinMetadata)                  | none (different thing than `coin_index` despite the name)                                                                              | Could serve `iota_getCoinMetadata` and friends; small, independent win.                                                                                                                                                                                                                                                                                                                                                                                |
| `transaction_checkpoints`: digest → checkpoint seq                                                       | `transactions_seq`/`transaction_order`: digest ↔ global tx sequence                                                                    | **No.** JSON-RPC cursors embed the global tx sequence number; deriving it per lookup from checkpoint contents position is possible but adds an indirection to every query. Keep separate until the JSON-RPC query APIs are removed.                                                                                                                                                                                                                    |
| `package_version`                                                                                        | none                                                                                                                                   | N/A.                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| none                                                                                                     | `transactions_{from,to}_addr`, `transactions_by_{input,mutated}_object_id`, `transactions_by_move_function`, `event_*`                 | **Not shareable** — `queryTransactionBlocks`/`queryEvents` exist only in JSON-RPC. These grow with the retention window and dominate disk; they only go away with the JSON-RPC → gRPC migration (P5).                                                                                                                                                                                                                                                  |

Recommended order:

1. `dynamic_field` (key-only, resolve on read) — biggest write-path and rebuild savings,
   no cursor-semantics questions.
2. `owner` + `coin_index` unification — one live-object table instead of three, one less
   rebuild scan; needs the cursor compat layer and accepts a result-order change.
3. Independent cheap alternative if sharing is deferred: run the gRPC and JSON-RPC
   rebuilds over a **single** `par_index_live_object_set` pass instead of scanning the
   live set twice on a restored node.

Context: given the JSON-RPC → gRPC migration plan, sharing the live-state tables also
shrinks what has to be deleted later; the unshareable history tables die with the
JSON-RPC query APIs.
