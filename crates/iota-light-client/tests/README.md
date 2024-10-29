To generate checkpoint snapshots, follow these steps:

1. `cargo run --release --bin iota start --force-regenesis --with-faucet`
2. Clear checkpoints.yaml file - make `checkpoints: []`
3. Run `cargo run --bin generate_chk_snapshots`
4. Replace data in tests according to generated checkpoints.
