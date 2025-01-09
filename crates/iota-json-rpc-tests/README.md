# `iota-json-rpc-tests`

This crate wraps a suite of end-to-end tests of the `iota-json-rpc` library.

It uses the `iota-cluster` crate to spawn a test cluster of nodes, that runs either in the simulation testing framework set forth by `iota-simulator` or in `tokio` runtime, depending on whether the rust config variable `msim` is set.

The `iota-json-rpc` library exposes an rpc-server builder, and implements the
`iota-json-rpc-api` traits for `iota-node`.

As such the tests are limited to routing tests of a server instance, and on the
methods exposed by the `iota-node` inner rpc services.

## How to run the tests

Certain test cases rely on the test execution model, such as `nextest` or `simtest`, due to one test per process design. One example of this is the `get_all_coins_limit_zero_with_env_var` test case from the `CoinApi`.
When running all `coin_api` tests using `cargo test`, failures may occur because the `QUERY_MAX_RESULT_LIMIT` is initialized upon first access (as a Singleton). This behavior complicates testing, as subsequent tests
that rely on this data will be affected by the initial test's configuration. Moreover, some test cases may need different values for `QUERY_MAX_RESULT_LIMIT`, further complicating the testing process.

> [!NOTE]
>
> To speed up the tests running time we can use the `simulator` profile, it internally uses the `opt-level = 1` which gives about 5x speedup executing tests without slowing down build times very much.
>
> We can use this profile for `cargo nextest` or `cargo test`, when using `cargo simtest` this profile is enabled by default

### Using `tokio`

- `cargo nextest run --cargo-profile simulator -p iota-json-rpc-tests`

### Using the simulation testing framework

- Setup: `./scripts/simtest/install.sh`
- Run: `cargo simtest -p iota-json-rpc-tests`

## Coverage of `iota-json-rpc-api` methods (22/64)

The coverage is low, but it should be noted that serving RPC requests is not the
primary responsibility of nodes.

This is typically delegated to the RPC service exposed by `iota-indexer`
instances.

That is, expect for the `WriteApi` methods that serve requests relayed by `iota-indexer`

### `CoinReadApi` (6/6)

- [x] `get_coins`
- [x] `get_all_coins`
- [x] `get_balance`
- [x] `get_all_balances`
- [x] `get_coin_metadata`
- [x] `get_total_supply`

### `GovernanceReadApi` (8/8)

- [x] `get_stakes_by_ids`
- [x] `get_stakes`
- [x] `get_timelocked_stakes_by_ids`
- [x] `get_timelocked_stakes`
- [x] `get_committee_info`
- [x] `get_latest_iota_system_state`
- [x] `get_reference_gas_price`
- [x] `get_validators_apy`

### `IndexerApi` (5/5)

- [x] `get_owned_objects`
- [x] `query_transaction_blocks`
- [x] `query_events`
- [x] `get_dynamic_fields`
- [x] `get_dynamic_field_object`

### `MoveUtils` (5/5)

- [x] `get_move_function_arg_types`
- [x] `get_normalized_move_modules_by_package`
- [x] `get_normalized_move_module`
- [x] `get_normalized_move_struct`
- [x] `get_normalized_move_function`

### `ReadApi` (14/14)

- [x] `get_transaction_block`
- [x] `multi_get_transaction_blocks`
- [x] `get_object`
- [x] `multi_get_objects`
- [x] `try_get_past_object`
- [x] `try_multi_get_past_objects`
- [x] `try_get_object_before_version`
- [x] `get_checkpoint`
- [x] `get_checkpoints`
- [x] `get_events`
- [x] `get_total_transaction_blocks`
- [x] `get_latest_checkpoint_sequence_number`
- [x] `get_protocol_config`
- [x] `get_chain_identifier`

### `TransactionBuilder` (15/15)

- [x] `transfer_object`
- [x] `transfer_iota`
- [x] `pay`
- [x] `pay_iota`
- [x] `pay_all_iota`
- [x] `move_call`
- [x] `publish`
- [x] `split_coin`
- [x] `split_coin_equal`
- [x] `merge_coin`
- [x] `batch_transaction`
- [x] `request_add_stake`
- [x] `request_withdraw_stake`
- [x] `request_add_timelocked_stake`
- [x] `request_withdraw_timelocked_stake`

### `WriteApi` (2/3)

- [x] `execute_transaction_block`
- [x] `dev_inspect_transaction_block`
- [x] `dry_run_transaction_block`
