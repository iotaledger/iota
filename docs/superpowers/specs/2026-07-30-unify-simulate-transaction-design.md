# Unify dry-run and dev-inspect onto `simulate_transaction`

## Goal

`AuthorityState` has four near-duplicate simulation entry points:
`dry_exec_transaction`, `dry_exec_transaction_for_benchmark`,
`dev_inspect_transaction_block`, and `simulate_transaction`. The first three carry
their own copies of the preflight checks, mock-gas handling, and executor
invocation. Remove them and route every caller through `simulate_transaction`,
which already expresses dry-run as `VmChecks::Enabled` and dev-inspect as
`VmChecks::Disabled`.

## Differences between the functions

### `dry_exec_transaction` vs `simulate_transaction(VmChecks::Enabled)`

| Aspect                                     | `dry_exec_transaction`                                                               | `simulate_transaction(Enabled)`                                          | Verdict                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------ | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mock gas coin ID                           | `ObjectId::random()`                                                                 | `ObjectId::MAX`                                                          | Becomes deterministic. Both report it via `mock_gas_id`, so balance-change filtering is unaffected.                                                                                                                                                                                                                                                                                                                        |
| Mock gas owner / value / version / prev-tx | `gas_owner()`, `SIMULATION_GAS_COIN_VALUE`, `OBJECT_START_VERSION`, `GENESIS_MARKER` | `gas_data().owner`, same values                                          | Identical — `gas_owner()` returns `gas_data().owner`.                                                                                                                                                                                                                                                                                                                                                                      |
| Gas check with mock gas                    | `check_transaction_input_with_given_gas`                                             | mock pushed into input objects, then `check_transaction_input`           | Equivalent. The two differ only in `gas_override` (moot: the mock ref is already written into `gas_data`) and `is_execute_transaction_to_effects`, which `check_gas` reads only when `authentication_gas_budget > 0` — hard-coded to 0 in every simulation path.                                                                                                                                                           |
| Executor entry                             | `execute_transaction_to_effects` → `Normal`                                          | `dev_inspect_transaction(skip_all_checks = false)` → `DevInspect<false>` | Equivalent checks. `DevInspect<false>` returns exactly `Normal`'s values for `allow_arbitrary_function_calls`, `allow_arbitrary_values`, `skip_conservation_checks`, `packages_are_predefined`, and `allow_auth_context`. It additionally collects per-command return values and mutable-reference outputs: extra BCS and type-tag work per command, plus a theoretical extra error path through `value_to_bytes_and_tag`. |
| Transaction digest                         | caller-supplied                                                                      | computed `transaction.digest()`                                          | Same value at every call site. `SenderSignedData::digest()` delegates to `TransactionData::digest()`, and `TransactionDigest::new(intent_msg.value.digest().into_inner())` is that same hash — the `IntentMessage` wrapper is a no-op there.                                                                                                                                                                               |
| Preflight order                            | fullnode check, then system-tx check                                                 | system-tx, then fullnode                                                 | Only changes which error text a system transaction on a validator receives.                                                                                                                                                                                                                                                                                                                                                |
| Events                                     | always `inner_temp_store.events`                                                     | `None` when `effects.events_digest()` is `None`                          | Callers need `unwrap_or_default()`.                                                                                                                                                                                                                                                                                                                                                                                        |
| Return                                     | `DryRunTransactionBlockResponse` + `written_with_kind` + effects + `mock_gas`        | `SimulateTransactionResult`                                              | Response shaping moves to the RPC layer.                                                                                                                                                                                                                                                                                                                                                                                   |

### `dry_exec_transaction_for_benchmark`

Differs from `dry_exec_transaction` only by skipping the fullnode and system-tx
checks — it runs against a validator in `iota-single-node-benchmark`. Its single
caller, `execute_sample_transaction`, uses only `effects` to log a sample
transaction and sits outside the timed loop, so `DevInspect<false>`'s extra
per-command work does not affect any benchmark number.

### `dev_inspect_transaction_block` vs `simulate_transaction(VmChecks::Disabled)`

| Aspect                                | `dev_inspect_transaction_block`                                                                                                                                | `simulate_transaction(Disabled)`                              | Verdict                                                                                                |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Input                                 | `sender` + `TransactionKind` + optional price/budget/sponsor/gas objects; builds `TransactionData` with `price = rgp`, `budget = max_tx_gas`, `owner = sender` | full `TransactionData`                                        | Assembly moves to the RPC layer.                                                                       |
| Gas budget for metering               | `IotaGasStatus::new(max_tx_gas, …)` — silently ignores a caller-supplied `gas_budget`                                                                          | `IotaGasStatus::new(transaction.gas_budget(), …)` — honors it | The one genuine semantic change. See "Decisions" below.                                                |
| Mock gas coin                         | created in both branches, `ObjectId::random()`, pushed as `ImmOrOwnedMoveObject`                                                                               | `ObjectId::MAX`, pushed via `new_from_gas_object`             | `new_from_gas_object` builds precisely `ImmOrOwnedMoveObject` + `Object`; identical apart from the ID. |
| Digest                                | `TransactionDigest::new(IntentMessage{…}.value.digest().into_inner())`                                                                                         | `transaction.digest()`                                        | Identical value.                                                                                       |
| Raw txn/effects BCS                   | built here behind `show_raw_txn_data_and_effects`                                                                                                              | not available                                                 | Moves to the RPC layer.                                                                                |
| `suggested_gas_price`                 | not computed                                                                                                                                                   | always computed                                               | `DevInspectResults` has no such field; dropped.                                                        |
| Non-skip-checks branch with empty gas | `check_transaction_input_with_given_gas`                                                                                                                       | equivalent path                                               | No difference.                                                                                         |

Net: the only substantive semantic change is the dev-inspect gas budget.
Everything else is either provably equivalent or a mechanical move of response
shaping out of `AuthorityState`.

## Decisions

**Package and type resolution.** Preserve simulation-aware resolution rather than
adopting the indexer's store-only approach, so a dry-run that publishes a package
and then calls into it or emits its events still decodes correctly.

**Dev-inspect gas budget.** Honor the caller's budget. Default `budget` to
`max_tx_gas` when the caller omits it — identical to today for every caller that
does not set one, including all transactional tests, so no snapshot churn. Callers
who do set a budget now get it enforced instead of silently overridden, matching
the indexer's JSON-RPC and the gRPC `simulate_transactions` behavior.

## Design

### 1. `iota-core/src/authority.rs` — one execution path

Delete `dry_exec_transaction`, `dry_exec_transaction_impl`,
`dry_exec_transaction_for_benchmark`, and `dev_inspect_transaction_block`. Split
the surviving `simulate_transaction` into:

```rust
/// Simulate a transaction without committing it. `checks` selects dry-run
/// (`Enabled`) or dev-inspect (`Disabled`) Move VM semantics.
pub fn simulate_transaction(&self, transaction: TransactionData, checks: VmChecks)
    -> IotaResult<SimulateTransactionResult>          // keeps the is_fullnode guard

pub fn simulate_transaction_for_benchmark(&self, transaction: TransactionData)
    -> IotaResult<SimulateTransactionResult>          // no is_fullnode guard

fn simulate_transaction_inner(&self, epoch_store: &AuthorityPerEpochStore,
    transaction: TransactionData, checks: VmChecks) -> IotaResult<SimulateTransactionResult>
```

`simulate_transaction_for_benchmark` mirrors the existing
`prepare_transaction_for_benchmark` beside it. The system-transaction rejection
stays in `_inner` so the benchmark path keeps it.

`simulate_transaction`'s body is otherwise unchanged — it already performs every
check the deleted functions ran.

### 2. `iota-types` — two enablers

`TemporaryModuleResolver` takes `(&WrittenObjects, BinaryConfig, fallback)`
instead of `&InnerTemporaryStore`. It reads only `written` and `binary_config`,
and `authority.rs` is its sole caller today.

`InMemoryStorage` is used as-is; it already implements `BackingPackageStore`,
`ObjectStore`, and `GetModule`, so no new trait impls are needed.

### 3. `iota-json-rpc` — response shaping moves here

`StateRead` loses `dry_exec_transaction` and `dev_inspect_transaction_block` and
gains:

```rust
fn simulate_transaction(&self, transaction: TransactionData, checks: VmChecks)
    -> StateReadResult<SimulateTransactionResult>;
```

`MockStateRead` is derived by `automock` and no test stubs the removed methods, so
mocks need no edits.

`transaction_execution_api.rs` builds both responses:

- `dry_run_transaction_block`: `simulate_transaction(tx, Enabled)`, then `input`
  via the generalized `TemporaryModuleResolver` over `output_objects`, `events`
  via a `type_layout_resolver` over
  `PackageStoreWithFallback::new(&InMemoryStorage::new(output_objects…), backing_package_store)`,
  and `execution_error_source` from `execution_result.as_ref().err()`. `events`
  needs `unwrap_or_default()` for the `None` case.
- `dev_inspect_transaction_block`: assembles the `TransactionData`
  (`price = rgp`, `budget = max_tx_gas` when omitted, `owner = sender`), performs
  the two `bcs::to_bytes` calls for `show_raw_txn_data_and_effects`, calls
  `simulate_transaction` with `Disabled` when `skip_checks` (default `true`) and
  `Enabled` otherwise, then `DevInspectResults::new(...)`.

`simulate_transaction` patches the mock gas reference into a local copy of the
transaction and does not return it, but `DryRunTransactionBlockResponse::input`
reflects the patched transaction today. Because `mock_simulation_gas_coin` is
deterministic, the RPC layer reproduces the identical `ObjectReference` when
`mock_gas_id.is_some()`, keeping the response byte-identical.

### 4. `WriteKind` plumbing disappears

`written_with_kind` existed only to feed `ObjectProviderCache::new_with_cache`,
which uses just `object_id`, `object_ref.version`, and `object` — and
`object_ref.version == object.version()`. So `new_with_cache` takes
`&BTreeMap<ObjectId, Object>` (i.e. `output_objects` directly), and the
created/unwrapped/mutated → `WriteKind` mapping is deleted rather than relocated.

### 5. Remaining call sites

- `iota-transactional-test-runner`: both `TransactionalAdapter` methods collapse
  into one `simulate_transaction(tx, checks)`. `test_adapter.rs`'s `dry_run` and
  `dev_inspect` only destructure `effects` and `events`, and the redundant
  `digest` parameter is dropped. The `budget = max_tx_gas` default keeps the
  `.exp` snapshots unchanged.
- `iota-single-node-benchmark`: `execute_dry_run` calls
  `simulate_transaction_for_benchmark`, taking `.effects`.
- `iota-core/src/unit_tests/authority_tests.rs`: roughly 15 sites. Add a local
  test helper with the old `(sender, kind, gas_price)` shape returning
  `SimulateTransactionResult` to keep the diff small, and assert on
  `execution_result` / `events` / `effects` instead of `DevInspectResults`.
- `iota-graphql-rpc` and `iota-indexer` reach the node over JSON-RPC and gRPC and
  are unchanged.

## Verification

- `cargo ci-clippy`
- `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-core -p iota-json-rpc -p iota-types`
- The transactional-test-runner `.exp` snapshot suites; expect zero diffs.
- `cargo simtest -p iota-e2e-tests` for the gRPC simulate tests and
  `tx_bytes_validity_check`.
