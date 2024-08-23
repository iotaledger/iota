# `iota-json-rpc-tests`

This crate wraps a suite of end-to-end tests of the `iota-json-rpc` library.

It uses the `iota-cluster` crate to spawn a test cluster of nodes, that runs either in the simulation testing framework set forth by `iota-simulator` or in `tokio` runtime, depending on whether the rust config variable `msim` is set.

The `iota-json-rpc` library exposes an rpc-server builder, and implements the
`iota-json-rpc-api` traits for `iota-node`.

As such the tests are limited to routing tests of a server instance, and on the
methods exposed by the `iota-node` inner rpc services.

## Coverage of `iota-json-rpc-api` methods (22/64)

The coverage is low, but it should be noted that serving RPC requests is not the
primary responsibility of nodes.

This is typically delegated to the RPC service exposed by `iota-indexer`
instances.

That is, expect for the `WriteApi` methods that serve requests relayed by `iota-indexer`

### `CoinReadApi` (4/6)

- [x] `get_coins`
- [ ] `get_all_coins`
- [x] `get_balance`
- [ ] `get_all_balances`
- [x] `get_coin_metadata`
- [x] `get_total_supply`

### `ExtendedApi` (0/4)

- [ ] `get_epochs`
- [ ] `get_current_epoch`
- [ ] `query_objects`
- [ ] `get_total_transactions`

### `GovernanceReadApi` (5/8)

- [x] `get_stakes_by_ids`
- [x] `get_stakes`
- [x] `get_timelocked_stakes_by_ids`
- [x] `get_timelocked_stakes`
- [ ] `get_committee_info`
- [x] `get_latest_iota_system_state`
- [ ] `get_reference_gas_price`
- [ ] `get_validators_apy`

### `IndexerApi` (2/9)

- [x] `get_owned_objects`
- [ ] `query_transaction_blocks`
- [ ] `query_events`
- [ ] `subscribe_event`
- [x] `subscribe_transaction`
- [ ] `get_dynamic_fields`
- [ ] `get_dynamic_field_object`
- [ ] `resolve_name_service_address`
- [ ] `resolve_name_service_names`

### `MoveUtils` (0/5)

- [ ] `get_move_function_arg_types`
- [ ] `get_normalized_move_modules_by_package`
- [ ] `get_normalized_move_module`
- [ ] `get_normalized_move_struct`
- [ ] `get_normalized_move_function`

### `ReadApi` (2/14)

- [ ] `get_transaction_block`
- [ ] `multi_get_transaction_blocks`
- [x] `get_object`
- [x] `multi_get_objects`
- [ ] `try_get_past_object`
- [ ] `try_multi_get_past_objects`
- [ ] `get_loaded_child_objects`
- [ ] `get_checkpoint`
- [ ] `get_checkpoints`
- [ ] `get_events`
- [ ] `get_total_transaction_blocks`
- [ ] `get_latest_checkpoint_sequence_number`
- [ ] `get_protocol_config`
- [ ] `get_chain_identifier`

### `TransactionBuilder` (7/15)

- [x] `transfer_object`
- [ ] `transfer_iota`
- [ ] `pay`
- [ ] `pay_iota`
- [ ] `pay_all_iota`
- [x] `move_call`
- [x] `publish`
- [ ] `split_coin`
- [ ] `split_coin_equal`
- [ ] `merge_coin`
- [ ] `batch_transaction`
- [x] `request_add_stake`
- [x] `request_withdraw_stake`
- [x] `request_add_timelocked_stake`
- [x] `request_withdraw_timelocked_stake`

### `WriteApi` (2/3)

- [x] `execute_transaction_block`
- [ ] `dev_inspect_transaction_block`
- [x] `dry_run_transaction_block`
