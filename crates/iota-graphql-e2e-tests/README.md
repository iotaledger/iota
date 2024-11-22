End-to-end tests for GraphQL service, built on top of the transactional test
runner.

# Testing through the transactional test runner

This crate specifically tests GraphQL queries in an e2e manner against the Indexer database.
The tests are executed through the [transactional test runner](../iota-transactional-test-runner), mediated by the [test.rs](tests/tests.rs) module.

Each test is defined in a `.move` file, which contains various statements that are executed in a transactional manner.
In addition to the `.move` file, there is a corresponding `.exp` file that contains the expected output of the transactional test.

# Local Set-up

These tests require a running instance of the `postgres` service, with a
database set-up.

It is recommended that the database server is started in a docker container.

## Using `docker compose`

```sh
$ POSTGRES_USER=postgres POSTGRES_DB=postgres POSTGRES_PASSWORD=postgrespw POSTGRES_INITDB_ARGS="-U postgres" docker compose -f docker/pg-services-local/docker-compose.yaml up -d postgres
```

## Using `docker`

```sh
docker run -d --name postgres \
 -e POSTGRES_PASSWORD=postgrespw \
 -e POSTGRES_INITDB_ARGS="-U postgres" \
 -p 5432:5432 \
 postgres:15 \
 -c max_connections=1000
```

# Running Locally

When running the tests locally, they must be run with the `pg_integration`
feature enabled:

```sh
$ cargo nextest run --features pg_integration
```

# Test coverage

The e2e tests focus on validating the behavior and consistency of GraphQL queries against indexer DB.

Following `Query` entrypoints are used in the tests:

- [x] chain_identifier (returns `String`)
- [x] available_range (returns `AvailableRange`)
- [x] service_config (returns `ServiceConfig`)
- [] dry_run_transaction_block (returns `DryRunResult`)
- [x] owner (returns `Owner`)
- [x] object (returns `Object`)
- [x] package (returns `MovePackage`)
- [x] address (returns `Address`)
- [x] type_ (returns `MoveType`)
- [x] epoch (returns `Epoch`)
- [x] checkpoint (returns `Checkpoint`)
- [x] transaction_block (returns `TransactionBlock`)
- [x] coins (returns a page of `Coin`)
- [x] checkpoints (returns a page of `Checkpoint`)
- [x] transaction_blocks (returns a page of `TransactionBlock`)
- [x] events (returns a page of `Event`s)
- [x] objects (returns a page of `Object`s)
- [x] packages (returns a page of `Package`s)
- [x] protocol_config (returns `ProtocolConfigs`)
- [x] coin_metadata (returns `CoinMetadata`)
- [] verify_zklogin_signature (returns `ZkLoginVerifyResult`)

Coverage for the possible dynamic queries:

`AvailableRange`
- [x] first
- [x] last

`ServiceConfig`
- [] is_enabled
- [x] available_versions
- [] enabled_features
- [] max_query_depth
- [] max_query_nodes
- [] max_output_nodes
- [] max_db_query_cost
- [] default_page_size
- [] max_page_size
- [] mutation_timeout_ms
- [] request_timeout_ms
- [] max_query_payload_size
- [] max_type_argument_depth
- [] max_type_argument_width
- [] max_type_nodes
- [] max_move_value_depth
- [] max_transaction_ids
- [] max_scan_limit

`Owner`
- [x] address
- [x] objects (handled by `OwnerImpl`)
  - `ObjectFilter`
    - [x] `type_`
    - [] `owner`
    - [] `object_ids`
    - [] `object_keys`
- [x] balance
  - [x] `ExactTypeFilter`
- [] balances
- [x] coins
  - [x] `ExactTypeFilter`
- [] staked_iotas
- [] as_address
- [x] as_object
- [] dynamic_field
- [x] dynamic_object_field
- [x] dynamic_fields

`DryRunResult`
- [] error
- [] results
- [] transaction

`Object`
- [x] address
- [] objects
  - `ObjectFilter`
    - [] `type_`
    - [] `owner`
    - [] `object_ids`
    - [] `object_keys`
- [x] balance
  - [x] `ExactTypeFilter`
- [] balances
- [x] coins
  - [x] `ExactTypeFilter`
- [x] staked_iotas
- [x] version
- [x] status
- [x] digest
- [x] owner
- [] previous_transaction_block
- [] storage_rebate
- [] received_transaction_blocks
- [] bcs
- [] display
- [] dynamic_field
- [] dynamic_object_field
- [] dynamic_fields
- [x] as_move_object
- [x] as_move_package

`MovePackage`
- [] address (handled by `Object`)
- [] objects (handled by `Object`)
  - `ObjectFilter`
    - [] `type_`
    - [] `owner`
    - [] `object_ids`
    - [] `object_keys`
- [] balance (handled by `Object`)
  - `ExactTypeFilter`
- [] balances (handled by `Object`)
- [] coins (handled by `Object`)
   - `ExactTypeFilter`
- [] staked_iotas (handled by `Object`)
- [] version (handled by `Object`)
- [] status (handled by `Object`)
- [] digest (handled by `Object`)
- [] owner (handled by `Object`)
- [] previous_transaction_block (handled by `Object`)
- [] storage_rebate (handled by `Object`)
- [] received_transaction_blocks (handled by `Object`)
- [] bcs (handled by `Object`)
- [x] package_at_version
- [x] package_versions
  - `MovePackageVersionFilter`
    - [x] after_version
    - [x] before_version
- [x] latest_package
- [x] module
- [x] modules
- [] linkage
- [] type_origins
- [] module_bcs

`Address`
- [x] address
- [x] objects
  - `ObjectFilter`
    - [x] `type_`
    - [] `owner`
    - [] `object_ids`
    - [] `object_keys`
- [] balance
  - [] `ExactTypeFilter`
- [] balances
- [x] coins
  - [x] `ExactTypeFilter`
- [x] staked_iotas
- [] transaction_blocks
  - [] `TransactionBlockFilter`
    - [] `function`
    - [] `kind`
    - [] `after_checkpoint`
    - [] `at_checkpoint`
    - [] `before_checkpoint`
    - [] `sign_address`
    - [] `recv_address`
    - [] `input_object`
    - [] `changed_object`
    - [] `transaction_ids`

`MoveType`
- [] repr
- [] signature
- [] layout
- [] abilities

`Epoch`
- [x] epoch_id
- [x] reference_gas_price
- [x] validator_set
- [] start_timestamp
- [] end_timestamp
- [] total_checkpoints
- [] total_transactions
- [] total_gas_fees
- [] total_stake_rewards
- [] fund_size
- [] net_inflow
- [] fund_inflow
- [] fund_outflow
- [] protocol_configs
- [] system_state_summary
- [] live_object_set_digest
- [x] checkpoints (returns a page of `Checkpoint`)
- [x] transaction_blocks

`Checkpoint`
- [x] digest
- [x] sequence_number
- [] timestamp
- [] validator_signatures
- [] previous_checkpoint_digest
- [] network_total_transactions
- [] rolling_gas_summary
- [] epoch (handled by `Epoch`)
- [x] transaction_blocks (returns a page of `TransactionBlock`)

`TransactionBlock`
- [x] digest
- [] sender
- [] gas_input
- [x] kind
- [] signatures
- [x] effects
- [] expiration
- [] bcs

`ProtocolConfigs`
- [x] protocol_version
- [] feature_flags
- [] configs
- [x] config
- [x] feature_flag

`CoinMetadata`
- [] address (handled by `MoveObject`)
- [] objects (handled by `MoveObject`)
- [] balance (handled by `MoveObject`)
- [] balances (handled by `MoveObject`)
- [] coins (handled by `MoveObject`)
- [] staked_iotas (handled by `MoveObject`)
- [] version (handled by `MoveObject`)
- [] status (handled by `MoveObject`)
- [] digest (handled by `MoveObject`)
- [] owner (handled by `MoveObject`)
- [] previous_transaction_block (handled by `MoveObject`)
- [] storage_rebate (handled by `MoveObject`)
- [] received_transaction_blocks (handled by `MoveObject`)
- [] bcs (handled by `MoveObject`)
- [] contents (handled by `MoveObject`)
- [] display (handled by `MoveObject`)
- [] dynamic_field (handled by `MoveObject`)
- [] dynamic_object_field (handled by `MoveObject`)
- [] dynamic_fields (handled by `MoveObject`)
- [x] decimals
- [x] name
- [x] symbol
- [x] description
- [x] icon_url
- [x] supply

`Event`
  - [x] sending_module
  - [x] sender
  - [x] timestamp
  - [x] move_value (flattened to `MoveValue`)

For the `Mutation` type, the coverage is as follows:

- [x] execute_transaction_block

Please note that `dry_run_transaction_block` and `execute_transaction_block` are not covered directly in the `iota-graphql-e2e-tests` tests, but in the `iota-graphql-rpc` tests.

# Snapshot Stability

Tests are pinned to an existing protocol version that has already been used on a
production network. The protocol version controls the protocol config and also
the version of the framework that gets used by tests. By using a version that
has already been used in a production setting, we are guaranteeing that it will
not be changed by later modifications to the protocol or framework (this would
be a bug).

When adding a new test, **remember to set the `--protocol-version`** for that
test to ensure stability.
