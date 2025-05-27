To generate checkpoint snapshots, follow these steps:

1. `cargo run --release --all-features --bin iota start --force-regenesis --with-faucet --with-graphql`
2. Run `cargo run --bin generate-checkpoint-snapshots`
3. Replace data in tests according to generated checkpoints.
