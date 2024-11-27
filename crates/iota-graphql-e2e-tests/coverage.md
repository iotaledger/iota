# Coverage

The e2e tests focus on validating the behavior and consistency of GraphQL queries against indexer DB.

Following `Query` entrypoints are used in the tests:










- [x] transaction_blocks (returns a page of `TransactionBlock`)
- [x] events (returns a page of `Event`s)
- [x] objects (returns a page of `Object`s)
- [x] packages (returns a page of `Package`s)
- [x] protocol_config (returns `ProtocolConfigs`)
- [x] coin_metadata (returns `CoinMetadata`)
- [] verify_zklogin_signature (returns `ZkLoginVerifyResult`)



For the `Mutation` type, the coverage is as follows:

- [x] execute_transaction_block

Please note that `dry_run_transaction_block` and `execute_transaction_block` are not covered directly in the `iota-graphql-e2e-tests` tests, but in the `iota-graphql-rpc` tests.
