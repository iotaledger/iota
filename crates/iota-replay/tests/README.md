The sandbox snapshots were originally generated based on some Sui mainnet transactions.
However, we can no longer support them due to significant code changes (rebate, renaming, balance, etc.).
We need to generate new ones using the SandboxPersist command (for this, we require a non-system transactions to replay).

[make_sandbox_snapshot.rs](../examples/make_sandbox_snapshot.rs) generates a snapshot based on a mock transaction (for instance package publishing). This should be sufficient to make the tests pass, at least until we have specific transactions we want to replay on the testnet/mainnet.

The sandbox snapshots are generated according to specific transactions we want to replay based on the testnet/mainnet.
Currently, they only replay locally.

To make a snapshot(local example):

1. iota-test-validator - `cargo run`
2. `iota client faucet`
3. iota-replay/examples/move/tx_instance - `iota client publish --gas-budget 1000000000`
4. Save tx digest
5. iota-replay - `cargo run --example make_sandbox_snapshot <your tx digest>`
