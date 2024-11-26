The sandbox snapshots are generated according to specific transactions we want to replay.

To make a snapshot(local example):

1. iota-test-validator - `cargo run`
2. `iota client faucet`
3. iota-replay/examples/move/tx_instance - `iota client publish --gas-budget 1000000000`
4. Save tx digest
5. iota-replay - `cargo run --example make_sandbox_snapshot <your tx digest>`
