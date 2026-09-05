# Evaluate Deprecating `epoch_last_checkpoint_map` in Favor of `epoch_info` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Determine whether `epoch_last_checkpoint_map` can be deprecated in favor of `epoch_info`, and either narrow its external read dependencies or — the likely outcome — document why the table must stay.

**Architecture:** `epoch_last_checkpoint_map` (`EpochId → CheckpointSequenceNumber`) is written at _certification_ time, before a boundary executes (`checkpoints/mod.rs:604`). `epoch_info`'s closing-checkpoint info (`epoch_close_proof.last_checkpoint_summary`, from which `end_checkpoint()` derives) is only present once the row is _finalized_ at the boundary. **`epoch_info` is built from the map, not the other way around** — the local backfill calls `get_epoch_last_checkpoint_seq_number` (`epoch_info.rs:221`) to find which closing checkpoint to replay. So the deprecation direction in the review comment is inverted; this plan investigates first and commits to nothing until the audit is in.

**Tech Stack:** Rust, `iota-core` (`checkpoints/`, `authority.rs`, `storage.rs`), `iota-grpc-server`, `iota-node`.

## Global Constraints

- **No lint suppressions.** (CLAUDE.md)
- **Never disable or skip tests.** (CLAUDE.md)
- **Comment style:** doc comments for the caller; inline comments explain a non-obvious _why_. (RUST_CONVENTIONS.md)
- `cargo +nightly fmt` and `cargo ci-clippy` clean before commit.

## Known consumers (from the audit already done)

| Consumer                                                                        | Epoch queried             | Can migrate to `epoch_info`?                                      |
| ------------------------------------------------------------------------------- | ------------------------- | ----------------------------------------------------------------- |
| `epoch_info.rs:221` (`assemble_closing_checkpoint`, backfill)                   | closed, being rebuilt     | **No** — this is what _builds_ `epoch_info`; circular.            |
| `epoch_info.rs:294` (`current_epoch_start_checkpoint` fallback)                 | previous                  | No — fallback exists precisely for when the row is absent.        |
| `iota-node/src/lib.rs:2156` (reconfiguration)                                   | **current** (unfinalized) | **No** — `epoch_info` has no `end_checkpoint` for the open epoch. |
| `iota-core/src/authority.rs:3618` (`new_at_next_epoch_for_testing`)             | **current**               | No — testing-only, current epoch.                                 |
| `iota-grpc-server/src/types.rs:340` + `iota-core/src/storage.rs:515` (API read) | **arbitrary**             | Partially — closed epochs only.                                   |

Conclusion the audit already points to: the table cannot be removed (the backfill bootstraps from it, and reconfiguration needs the _current_ epoch's closing seq, which only the cert-time map holds). The single migration candidate is the gRPC API read path, and only for _closed_ epochs.

---

### Task 1: Confirm the audit and decide (decision gate)

**Files:**

- Read-only: the consumers in the table above.

**Interfaces:**

- Produces: a written conclusion (in the PR description) — either "stop, keep the table" or "proceed to Task 2 for the gRPC read path only."

- [ ] **Step 1: Re-grep consumers to catch anything the audit missed**

Run: `grep -rn "get_epoch_last_checkpoint\|get_epoch_last_checkpoint_seq_number\|epoch_last_checkpoint_map\|insert_epoch_last_checkpoint" crates/ | grep -v "test"`
Expected: the consumers in the table above (plus the cert-time writer and `simulacrum`). If a _new_ consumer appears, classify it as current-epoch vs closed-epoch.

- [ ] **Step 2: Verify the two reconfig-time callers query the current epoch**

Read `iota-node/src/lib.rs:2156` and `iota-core/src/authority.rs:3618`. Confirm both pass `cur_epoch_store.epoch()` / `epoch_store.epoch()` (the _open_ epoch) — for which `epoch_info` has no finalized `end_checkpoint`. If so, they cannot migrate. (Expected: confirmed.)

- [ ] **Step 3: Decide**

Given the table is load-bearing for the backfill and for current-epoch reconfiguration reads, the recommended decision is **keep the table**; do **not** deprecate. If so, do Task 3 (documentation) and stop. Only proceed to Task 2 if the team specifically wants the gRPC API to serve closed-epoch boundaries from the verified `epoch_info` chain.

---

### Task 2 (optional): Serve the gRPC closed-epoch read from `epoch_info`

Only do this if Step 3 chose to. It removes one _external_ dependency on the map for closed epochs, while the table stays for everything else.

**Files:**

- Modify: `crates/iota-core/src/storage.rs` (the `get_epoch_last_checkpoint` impl backing the RPC reader).
- Test: a unit/integration test exercising both the closed-epoch (`epoch_info`) and current-epoch (map fallback) paths.

**Interfaces:**

- Consumes: `CheckpointStore::get_epoch_info(EpochId)` (returns `EpochInfoV2` with `epoch_close_proof.last_checkpoint_summary: CertifiedCheckpointSummary`), `CheckpointStore::get_epoch_last_checkpoint`.
- Produces: a read path that prefers the finalized `epoch_info` row's certified closing summary and falls back to the map for the open/unfinalized epoch.

- [ ] **Step 1: Write the failing test**

Assert that for a closed epoch the value comes through even if the map entry is absent (proving the `epoch_info` source), and for the open epoch the map fallback still serves it.

```rust
// Closed epoch: epoch_info row finalized, map entry deleted → still returned.
// Open epoch: no finalized epoch_info row → served from the map.
// (Wire via a CheckpointStore staged like `missing_epochs_above_snapshot_prefix_*`.)
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo simtest -p iota-e2e-tests <test_name>`
Expected: FAIL (the map-only impl returns `None` once the map entry is deleted).

- [ ] **Step 3: Implement the prefer-`epoch_info` read**

In the `get_epoch_last_checkpoint` impl, try the finalized `epoch_info` row first; on `None`/unfinalized, fall back to the existing map lookup:

```rust
// Prefer the verified chain's certified closing summary for finalized
// (closed) epochs; the map still answers for the open epoch, whose row is
// not yet finalized.
if let Some(summary) = self
    .rocks
    .checkpoint_store
    .get_epoch_info(epoch_id)
    .map_err(/* ... */)?
    .and_then(|row| row.epoch_close_proof.map(|p| p.last_checkpoint_summary))
{
    return Ok(Some(summary.into()));
}
// fall back to the cert-time map (open epoch, or pre-backfill nodes)
```

- [ ] **Step 4: Run it to confirm it passes**

Run: `cargo simtest -p iota-e2e-tests <test_name>`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/iota-core/src/storage.rs crates/iota-e2e-tests/tests/<file>.rs
git commit -m "refactor(iota-core): serve closed-epoch last-checkpoint reads from epoch_info

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Document the table's role (the likely real outcome)

**Files:**

- Modify: `crates/iota-core/src/checkpoints/mod.rs` (the `epoch_last_checkpoint_map` field doc at ~180).

- [ ] **Step 1: Expand the field doc to record why it is not redundant with `epoch_info`**

```rust
/// A map from epoch ID to the sequence number of the last checkpoint in
/// that epoch.
///
/// Written at certification time, before the boundary executes, so it
/// records the closing sequence number independently of the later-finalized
/// `epoch_info` row. Never pruned. Not redundant with `epoch_info`:
/// `epoch_info` is *built from* this map (the local backfill reads it to
/// find each closing checkpoint to replay), and reconfiguration needs the
/// *current* epoch's closing seq, which `epoch_info` only records once that
/// epoch is finalized.
epoch_last_checkpoint_map: DBMap<EpochId, CheckpointSequenceNumber>,
```

- [ ] **Step 2: Build, lint, format**

Run: `cargo check -p iota-core && cargo ci-clippy && cargo +nightly fmt`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/iota-core/src/checkpoints/mod.rs
git commit -m "docs(iota-core): record why epoch_last_checkpoint_map is not redundant with epoch_info

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Recommendation

Do Task 1 → almost certainly conclude "keep the table" → do Task 3 (the doc clarification) and stop. Task 2 is genuinely optional and low-value: it swaps one read source for another only for closed-epoch gRPC queries, introduces a dual-source branch, and buys no storage savings (the map stays for the backfill and reconfiguration). Pursue Task 2 only if there's a concrete reason to want the API to reflect the cryptographically verified chain rather than the cert-time map.
